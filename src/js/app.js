// Main Application Entry Point
console.log('[ThinkUtils] Script loaded');

// Report uncaught frontend errors to the backend so they reach the process log.
//
// This is registered before any other import runs, because an exception thrown
// during module evaluation is exactly the case that would otherwise vanish. A
// view dying on an absent sysfs path leaves the sidebar painted and the process
// alive, so nothing outside the browser console would ever notice.
const reportError = (message) => {
  try {
    window.__TAURI__?.core?.invoke('report_frontend_error', {
      msg: String(message).slice(0, 500)
    });
  } catch {
    // Reporting must never itself throw and take down init.
  }
};

window.addEventListener('error', (e) => {
  reportError(`${e.message} @ ${e.filename}:${e.lineno}`);
});
window.addEventListener('unhandledrejection', (e) => {
  reportError(`unhandled rejection: ${e.reason}`);
});

import { initializeElements } from './dom.js';
import { setupTitlebar } from './titlebar.js';
import { setupFeatureNavigation } from './navigation.js';
import { setupFanControl, checkInitialPermissions } from './views/fan.js';
import { setupHomeActions, updateHomeView } from './views/home.js';
import { setupSyncHandlers } from './views/sync.js';
import { setupBatteryHandlers } from './views/battery.js';
import { setupSecurityHandlers } from './views/security.js';
import { setupAboutDialog } from './about.js';
import { openDialog, closeDialog } from './dialog.js';
import { state, setState } from './state.js';
import { initializeSettings } from './settingsManager.js';
import { isModularMode, loadTemplates, injectTemplates } from './templateLoader.js';

async function checkAndSetupPermissions() {
  console.log('[Permissions] Checking permission status...');
  try {
    // If the fan helper + polkit rule are already installed (from a previous setup),
    // skip the startup dialog. Sysfs permissions reset on reboot but the polkit rule
    // persists, so features can use pkexec for sysfs operations when needed.
    const fanPerms = await window.__TAURI__.core.invoke('check_permissions');
    if (fanPerms.success && fanPerms.data) {
      console.log('[Permissions] ✓ Fan helper already installed, skipping startup dialog');
      return true;
    }

    // First time: check if setup is needed
    const response = await window.__TAURI__.core.invoke('check_permissions_status');
    if (response.success && response.data) {
      if (!response.data.has_permissions) {
        console.log('[Permissions] First time setup needed, showing dialog');
        showPermissionDialog();
        return false;
      }
    }
  } catch (error) {
    console.error('[Permissions] Error checking permissions:', error);
  }
  return true;
}

function showPermissionDialog() {
  // Was a bare style.display toggle with no Escape handler, which made this
  // dialog impossible to dismiss from the keyboard.
  openDialog('permission-dialog');
}

function hidePermissionDialog() {
  closeDialog('permission-dialog');
}

async function setupPermissions() {
  console.log('[Permissions] Setting up permissions...');
  try {
    const response = await window.__TAURI__.core.invoke('setup_permissions');
    if (response.success) {
      console.log('[Permissions] ✓ Setup successful');
      hidePermissionDialog();
      await checkInitialPermissions();
      alert(
        'Permissions configured successfully!\n\nPlease restart ThinkUtils for all changes to take effect.'
      );
      return true;
    } else {
      console.error('[Permissions] ✗ Setup failed:', response.error);
      alert('Failed to setup permissions: ' + response.error);
      return false;
    }
  } catch (error) {
    console.error('[Permissions] ✗ Setup error:', error);
    alert('Error setting up permissions: ' + error);
    return false;
  }
}

function setupPermissionDialog() {
  const setupBtn = document.getElementById('setup-permissions');
  const skipBtn = document.getElementById('skip-permissions');

  if (setupBtn) {
    setupBtn.addEventListener('click', async () => {
      await setupPermissions();
    });
  }

  if (skipBtn) {
    skipBtn.addEventListener('click', () => {
      hidePermissionDialog();
    });
  }
}

async function initializeApp() {
  console.log('[ThinkUtils] Initializing...');

  let loadedTemplateCount = 0;

  // If using modular HTML, load templates first
  if (isModularMode()) {
    console.log('[ThinkUtils] Modular mode detected, loading templates...');
    try {
      const templates = await loadTemplates();
      injectTemplates(templates);
      loadedTemplateCount = Object.keys(templates).length;
    } catch (error) {
      console.error('[ThinkUtils] Failed to load templates:', error);
      // Continue anyway - app might still work with inline HTML
    }
  } else {
    console.log('[ThinkUtils] Using inline HTML mode');
  }

  initializeElements();

  // Each view's setup wires listeners onto cached elements, and every one of
  // them dereferences those elements without checking. If a single template
  // failed to load, the first such access threw and took the whole boot
  // sequence with it -- every later setup call, the permission check, and
  // settings loading never ran, while the sidebar (wired one line earlier)
  // still switched views. The app looked alive with every control inert, and
  // the template-failure handler above claimed to "continue anyway" while
  // doing nothing of the sort.
  //
  // Isolating each step makes that claim true: a broken view costs that view,
  // not the application. Failures are reported rather than swallowed, so this
  // degrades loudly instead of silently.
  const failures = [];
  const step = (name, fn) => {
    try {
      fn();
    } catch (error) {
      console.error(`[ThinkUtils] ${name} failed:`, error);
      failures.push(`${name}: ${error?.message ?? error}`);
      reportError(`init step ${name} failed: ${error?.message ?? error}`);
    }
  };

  step('titlebar', setupTitlebar);
  step('navigation', setupFeatureNavigation);
  step('fan', setupFanControl);
  step('home', setupHomeActions);
  step('sync', setupSyncHandlers);
  step('battery', setupBatteryHandlers);
  step('security', setupSecurityHandlers);
  step('about', setupAboutDialog);
  step('permissionDialog', setupPermissionDialog);

  // The fan sensor poll is NOT started here any more. It runs every second, and
  // starting it at launch meant it polled /proc for the life of the app no
  // matter which view was open. navigation.js starts it when the fan view is
  // shown and stops it when the view is left.

  // Check all permissions at startup (sysfs + fan helper + polkit rule).
  // One dialog handles everything. After setup, re-check fan permissions.
  await checkAndSetupPermissions();
  await checkInitialPermissions();

  // Load and apply all settings
  console.log('[ThinkUtils] Loading settings...');
  await initializeSettings();

  // Home refresh. Tracked in state so beforeunload can clear it -- this used to
  // be an untracked setInterval that the cleanup handler claimed to cover.
  const homeInterval = setInterval(() => {
    if (state.currentView === 'home') {
      updateHomeView();
    }
  }, 2000);
  setState('homeInterval', homeInterval);

  // Paint the starting view now rather than waiting for the first interval
  // tick. switchView() is only reached from a sidebar click, so the view the
  // app opens on never got its refresh and Home showed template placeholders
  // for its first two seconds.
  if (state.currentView === 'home') {
    step('initial home refresh', updateHomeView);
  }

  console.log('[ThinkUtils] Ready');

  // Signal that init actually completed. A headless test can see a fully painted
  // window from a frontend that died halfway through, so reaching this line is
  // the only proof the boot sequence finished.
  try {
    await window.__TAURI__?.core?.invoke('report_frontend_ready', {
      templates: loadedTemplateCount,
      views: document.querySelectorAll('#views-container > *').length,
      // Reaching this line no longer proves every view wired up, since a failed
      // step is now isolated rather than fatal. Report which ones failed so a
      // half-working boot is still visible to the launch test.
      failed: failures
    });
  } catch (error) {
    console.error('[ThinkUtils] Could not report ready state:', error);
  }
}

window.addEventListener('DOMContentLoaded', initializeApp);

// Clear every tracked timer. The previous version listed two of the three and
// read as though it were complete.
window.addEventListener('beforeunload', () => {
  for (const key of ['updateInterval', 'monitorInterval', 'homeInterval']) {
    if (state[key]) {
      clearInterval(state[key]);
      setState(key, null);
    }
  }
});
