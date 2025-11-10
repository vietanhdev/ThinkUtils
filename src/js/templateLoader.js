// Template Loader - Load and inject HTML templates
const TEMPLATES = {
  titlebar: '/templates/titlebar.html',
  sidebar: '/templates/sidebar.html',
  dialogs: '/templates/dialogs.html',
  // Views
  homeView: '/templates/views/home.html',
  fanView: '/templates/views/fan.html',
  batteryView: '/templates/views/battery.html',
  performanceView: '/templates/views/performance.html',
  monitorView: '/templates/views/monitor.html',
  systemView: '/templates/views/system.html',
  syncView: '/templates/views/sync.html',
  securityView: '/templates/views/security.html'
};

/**
 * Load all HTML templates
 * @returns {Promise<Object>} Object containing all loaded templates
 */
export async function loadTemplates() {
  console.log('[TemplateLoader] Loading templates...');

  const templates = {};

  try {
    const promises = Object.entries(TEMPLATES).map(async ([name, path]) => {
      const response = await fetch(path);
      if (!response.ok) {
        throw new Error(`Failed to load template: ${path}`);
      }
      const html = await response.text();
      templates[name] = html;
      console.log(`[TemplateLoader] ✓ Loaded ${name}`);
    });

    await Promise.all(promises);
    console.log('[TemplateLoader] All templates loaded');
    return templates;
  } catch (error) {
    console.error('[TemplateLoader] Error loading templates:', error);
    throw error;
  }
}

/**
 * Inject templates into the DOM
 * @param {Object} templates - Object containing template HTML strings
 */
export function injectTemplates(templates) {
  console.log('[TemplateLoader] Injecting templates...');

  // Inject titlebar
  if (templates.titlebar) {
    const titlebarContainer = document.getElementById('titlebar-container');
    if (titlebarContainer) {
      titlebarContainer.innerHTML = templates.titlebar;
      console.log('[TemplateLoader] ✓ Injected titlebar');
    }
  }

  // Inject sidebar
  if (templates.sidebar) {
    const sidebarContainer = document.getElementById('sidebar-container');
    if (sidebarContainer) {
      sidebarContainer.innerHTML = templates.sidebar;
      console.log('[TemplateLoader] ✓ Injected sidebar');
    }
  }

  // Inject dialogs
  if (templates.dialogs) {
    const dialogsContainer = document.getElementById('dialogs-container');
    if (dialogsContainer) {
      dialogsContainer.innerHTML = templates.dialogs;
      console.log('[TemplateLoader] ✓ Injected dialogs');
    }
  }

  // Inject views
  const viewsContainer = document.getElementById('views-container');
  if (viewsContainer) {
    const viewTemplates = [
      { key: 'homeView', id: 'home-view' },
      { key: 'fanView', id: 'fan-view' },
      { key: 'batteryView', id: 'battery-view' },
      { key: 'performanceView', id: 'performance-view' },
      { key: 'monitorView', id: 'monitor-view' },
      { key: 'systemView', id: 'system-view' },
      { key: 'syncView', id: 'sync-view' },
      { key: 'securityView', id: 'security-view' }
    ];

    viewTemplates.forEach(({ key, id }) => {
      if (templates[key]) {
        const viewDiv = document.createElement('div');
        viewDiv.className = 'content-view';
        viewDiv.id = id;
        viewDiv.style.display = id === 'home-view' ? 'block' : 'none';
        viewDiv.innerHTML = templates[key];
        viewsContainer.appendChild(viewDiv);
      }
    });
    console.log('[TemplateLoader] ✓ Injected views');
  }

  console.log('[TemplateLoader] All templates injected');
}

/**
 * Check if the app is using modular templates
 * @returns {boolean}
 */
export function isModularMode() {
  return document.getElementById('titlebar-container') !== null;
}
