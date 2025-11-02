// Main Application Entry Point
console.log('[ThinkUtils] Script loaded');

import { initializeElements } from './dom.js';
import { setupTitlebar } from './titlebar.js';
import { setupFeatureNavigation } from './navigation.js';
import { setupFanControl, checkInitialPermissions, startAutoUpdate } from './views/fan.js';
import { setupHomeActions, updateHomeView } from './views/home.js';
import { setupSyncHandlers } from './views/sync.js';
import { setupBatteryHandlers } from './views/battery.js';
import { setupAboutDialog } from './about.js';
import { state } from './state.js';

// Check if we're using modular HTML (template loading)
const isModularHTML = document.getElementById('titlebar-container') !== null;

async function initializeApp() {
  console.log('[ThinkUtils] Initializing...');

  // If using modular HTML, load templates first
  if (isModularHTML) {
    const { loadTemplates, injectTemplates } = await import('./templateLoader.js');
    const templates = await loadTemplates();
    injectTemplates(templates);
  }

  initializeElements();
  setupTitlebar();
  setupFeatureNavigation();
  setupFanControl();
  setupHomeActions();
  setupSyncHandlers();
  setupBatteryHandlers();
  setupAboutDialog();
  checkInitialPermissions();
  startAutoUpdate();

  // Update home view periodically
  setInterval(() => {
    if (state.currentView === 'home') {
      updateHomeView();
    }
  }, 2000);

  console.log('[ThinkUtils] Ready');
}

window.addEventListener('DOMContentLoaded', initializeApp);

window.addEventListener('beforeunload', () => {
  if (state.updateInterval) {
    clearInterval(state.updateInterval);
  }
  if (state.monitorInterval) {
    clearInterval(state.monitorInterval);
  }
});
