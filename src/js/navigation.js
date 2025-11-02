// Navigation and View Switching
import { elements } from './dom.js';
import { setState } from './state.js';
import { updateHomeView } from './views/home.js';
import { checkSyncStatus } from './views/sync.js';
import { loadSystemInfo } from './views/system.js';
import { loadBatteryInfo } from './views/battery.js';
import { loadPerformanceInfo } from './views/performance.js';
import { startMonitoring } from './views/monitor.js';

export function setupFeatureNavigation() {
  const menuItems = document.querySelectorAll('.menu-item');

  menuItems.forEach((item) => {
    item.addEventListener('click', (e) => {
      e.preventDefault();
      e.stopPropagation();

      if (item.classList.contains('disabled')) {
        console.log('[Navigation] Feature disabled:', item.dataset.feature);
        return;
      }

      const feature = item.dataset.feature;
      console.log('[Navigation] Switching to:', feature);

      menuItems.forEach((i) => i.classList.remove('active'));
      item.classList.add('active');

      switchView(feature);
    });
  });
}

export function switchView(view) {
  setState('currentView', view);

  // Hide all views
  if (elements.homeView) {
    elements.homeView.style.display = 'none';
  }
  if (elements.fanView) {
    elements.fanView.style.display = 'none';
  }
  if (elements.syncView) {
    elements.syncView.style.display = 'none';
  }
  if (elements.systemView) {
    elements.systemView.style.display = 'none';
  }
  if (elements.batteryView) {
    elements.batteryView.style.display = 'none';
  }
  if (elements.performanceView) {
    elements.performanceView.style.display = 'none';
  }
  if (elements.monitorView) {
    elements.monitorView.style.display = 'none';
  }

  // Update page title
  const titles = {
    home: { title: 'Home', subtitle: 'Quick settings and overview' },
    fan: { title: 'Fan Control', subtitle: 'Manage cooling and fan speeds' },
    battery: { title: 'Battery', subtitle: 'Monitor and optimize battery health' },
    performance: { title: 'Performance', subtitle: 'Optimize CPU and power settings' },
    monitor: { title: 'System Monitor', subtitle: 'Real-time resource monitoring' },
    system: { title: 'System Info', subtitle: 'Your ThinkPad details' },
    sync: { title: 'Cloud Sync', subtitle: 'Sync settings across devices' }
  };

  if (titles[view] && elements.pageTitle && elements.pageSubtitle) {
    elements.pageTitle.textContent = titles[view].title;
    elements.pageSubtitle.textContent = titles[view].subtitle;
  }

  // Show selected view
  switch (view) {
    case 'home':
      if (elements.homeView) {
        elements.homeView.style.display = 'block';
        updateHomeView();
      }
      break;
    case 'fan':
      if (elements.fanView) {
        elements.fanView.style.display = 'grid';
      }
      break;
    case 'sync':
      if (elements.syncView) {
        elements.syncView.style.display = 'block';
        checkSyncStatus();
      }
      break;
    case 'system':
      if (elements.systemView) {
        elements.systemView.style.display = 'block';
        loadSystemInfo();
      }
      break;
    case 'battery':
      if (elements.batteryView) {
        elements.batteryView.style.display = 'block';
        loadBatteryInfo();
      }
      break;
    case 'performance':
      if (elements.performanceView) {
        elements.performanceView.style.display = 'block';
        loadPerformanceInfo();
      }
      break;
    case 'monitor':
      if (elements.monitorView) {
        elements.monitorView.style.display = 'block';
        startMonitoring();
      }
      break;
  }
}
