// Sync View
const { invoke } = window.__TAURI__.core;
import { elements } from '../dom.js';
import { showStatus } from '../utils.js';
import { getState } from '../state.js';

export function setupSyncHandlers() {
  if (elements.btnGoogleLogin) {
    elements.btnGoogleLogin.addEventListener('click', handleGoogleLogin);
  }
  if (elements.btnGoogleLogout) {
    elements.btnGoogleLogout.addEventListener('click', handleGoogleLogout);
  }
  if (elements.btnSyncNow) {
    elements.btnSyncNow.addEventListener('click', handleSyncNow);
  }
  if (elements.btnDownloadSettings) {
    elements.btnDownloadSettings.addEventListener('click', handleDownloadSettings);
  }
}

export async function checkSyncStatus() {
  try {
    const response = await invoke('google_auth_status');

    if (response.success && response.data && response.data.is_logged_in) {
      elements.syncLogin.style.display = 'none';
      elements.syncDashboard.style.display = 'flex';

      const emailEl = document.getElementById('user-email');
      const syncEl = document.getElementById('last-sync');

      if (emailEl) {
        emailEl.textContent = response.data.user_email || 'Unknown';
      }
      if (syncEl) {
        syncEl.textContent = response.data.last_sync
          ? `Last synced: ${response.data.last_sync}`
          : 'Last synced: Never';
      }

      updateSyncedSettingsDisplay(response.data.settings || {});
    } else {
      elements.syncLogin.style.display = 'block';
      elements.syncDashboard.style.display = 'none';
    }
  } catch (error) {
    console.error('[Sync] Status check failed:', error);
  }
}

async function handleGoogleLogin() {
  try {
    showStatus('Opening Google login...', 'info');
    const response = await invoke('google_auth_init');

    if (response.success && response.data) {
      try {
        await invoke('open_url', { url: response.data.auth_url });
      } catch (openError) {
        const userAction = confirm(
          'Unable to open browser automatically.\n\nClick OK to copy the login URL to clipboard.'
        );
        if (userAction) {
          try {
            await navigator.clipboard.writeText(response.data.auth_url);
            showStatus('URL copied! Paste in your browser.', 'info');
          } catch (e) {
            prompt('Copy this URL and open in your browser:', response.data.auth_url);
          }
        }
        return;
      }

      showStatus('Complete login in your browser...', 'info');

      let attempts = 0;
      const maxAttempts = 60;

      const checkInterval = setInterval(async () => {
        attempts++;
        const statusResponse = await invoke('google_auth_status');

        if (statusResponse.success && statusResponse.data && statusResponse.data.is_logged_in) {
          clearInterval(checkInterval);
          elements.syncLogin.style.display = 'none';
          elements.syncDashboard.style.display = 'flex';

          const emailEl = document.getElementById('user-email');
          const syncEl = document.getElementById('last-sync');

          if (emailEl) {
            emailEl.textContent = statusResponse.data.user_email;
          }
          if (syncEl) {
            syncEl.textContent = `Last synced: ${statusResponse.data.last_sync}`;
          }

          updateSyncedSettingsDisplay(statusResponse.data.settings || {});
          showStatus('✓ Logged in successfully', 'success');
        } else if (attempts >= maxAttempts) {
          clearInterval(checkInterval);
          showStatus('Login timeout. Please try again.', 'error');
        }
      }, 1000);
    } else {
      showStatus(`Error: ${response.error}`, 'error');
    }
  } catch (error) {
    showStatus(`Error: ${error}`, 'error');
  }
}

async function handleGoogleLogout() {
  try {
    const response = await invoke('google_logout');
    if (response.success) {
      elements.syncLogin.style.display = 'block';
      elements.syncDashboard.style.display = 'none';
      showStatus('✓ Logged out', 'success');
    } else {
      showStatus(`Error: ${response.error}`, 'error');
    }
  } catch (error) {
    showStatus(`Error: ${error}`, 'error');
  }
}

async function handleSyncNow() {
  try {
    showStatus('Syncing settings...', 'info');

    // Import fan curve functions
    const { getCurvePoints } = await import('../fanCurve.js');

    const settings = {
      fan_mode: getState('currentFanMode'),
      fan_level: parseInt(elements.slider.value),
      fan_curve: getCurvePoints(),
      auto_start: false,
      minimize_to_tray: true,
      theme: 'system',
      battery_start_threshold: elements.thresholdStart
        ? parseInt(elements.thresholdStart.value)
        : 40,
      battery_stop_threshold: elements.thresholdStop ? parseInt(elements.thresholdStop.value) : 80
    };

    const response = await invoke('sync_to_cloud', { settings });
    if (response.success) {
      await checkSyncStatus();
      showStatus('✓ Settings synced', 'success');
    } else {
      showStatus(`Error: ${response.error}`, 'error');
    }
  } catch (error) {
    showStatus(`Error: ${error}`, 'error');
  }
}

async function handleDownloadSettings() {
  try {
    showStatus('Downloading settings...', 'info');
    const response = await invoke('sync_from_cloud');

    if (response.success && response.data) {
      const settings = response.data;

      // Restore fan curve if available
      if (settings.fan_curve) {
        const { setCurvePoints } = await import('../fanCurve.js');
        setCurvePoints(settings.fan_curve);
        localStorage.setItem('fanCurve', JSON.stringify(settings.fan_curve));
      }

      if (settings.fan_mode) {
        const { setFanMode } = await import('./fan.js');
        await setFanMode(settings.fan_mode, settings.fan_level);
      }

      if (elements.thresholdStart && settings.battery_start_threshold) {
        elements.thresholdStart.value = settings.battery_start_threshold;
        elements.thresholdStartValue.textContent = settings.battery_start_threshold + '%';
      }
      if (elements.thresholdStop && settings.battery_stop_threshold) {
        elements.thresholdStop.value = settings.battery_stop_threshold;
        elements.thresholdStopValue.textContent = settings.battery_stop_threshold + '%';
      }

      updateSyncedSettingsDisplay(settings);
      showStatus('✓ Settings applied from sync', 'success');
    } else {
      showStatus(`Error: ${response.error}`, 'error');
    }
  } catch (error) {
    showStatus(`Error: ${error}`, 'error');
  }
}

function updateSyncedSettingsDisplay(settings) {
  const fanModeEl = document.getElementById('synced-fan-mode');
  const themeEl = document.getElementById('synced-theme');
  const autostartEl = document.getElementById('synced-autostart');

  if (fanModeEl) {
    fanModeEl.textContent = settings.fan_mode || 'auto';
  }
  if (themeEl) {
    themeEl.textContent = settings.theme || 'system';
  }
  if (autostartEl) {
    autostartEl.textContent = settings.auto_start ? 'Enabled' : 'Disabled';
  }
}
