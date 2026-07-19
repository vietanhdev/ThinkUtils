// View registry — one place that knows what a view is called, where its element
// lives, and what to start and stop when it becomes visible.
//
// Before this, navigation.js held a 9-branch hide block, a separate titles map,
// and a 9-case show switch. The titles map and the per-view templates had
// already drifted: the MCP subtitle differed between them.
//
// It also had no concept of hiding. Nothing was ever torn down, so every timer
// was either global-forever or had to re-check `currentView` on each tick. The
// fan sensor poll did neither and ran every second for the life of the app,
// on a battery utility.

import { elements } from '../dom.js';
import { updateHomeView } from './home.js';
import { checkSyncStatus } from './sync.js';
import { loadSystemInfo } from './system.js';
import { loadBatteryInfo } from './battery.js';
import { loadPerformanceInfo } from './performance.js';
import { startMonitoring, stopMonitoring } from './monitor.js';
import { loadSecurityStatus } from './security.js';
import { loadMcpStatus, setupMcpView } from './mcp.js';
import { startAutoUpdate, stopAutoUpdate } from './fan.js';

/**
 * Every view, in sidebar order.
 *
 * `title` and `subtitle` are the single source of truth — the page header reads
 * them, and view templates must not repeat them.
 *
 * `display` matters: most views are `block`, but the fan view is a grid and
 * would collapse if shown as a block.
 *
 * `onShow` runs when the view becomes visible; `onHide` when it is replaced.
 * A view that starts a timer must stop it in `onHide`.
 */
export const VIEWS = [
  {
    id: 'home',
    title: 'Home',
    subtitle: 'Quick settings and overview',
    element: 'homeView',
    display: 'block',
    onShow: updateHomeView
  },
  {
    id: 'fan',
    title: 'Fan Control',
    subtitle: 'Manage cooling and fan speeds',
    element: 'fanView',
    display: 'grid',
    // Polls sensors every second. It used to start once at app launch and never
    // stop, so it kept polling /proc while the user sat on any other view.
    onShow: startAutoUpdate,
    onHide: stopAutoUpdate
  },
  {
    id: 'battery',
    title: 'Battery',
    subtitle: 'Monitor and optimize battery health',
    element: 'batteryView',
    display: 'block',
    onShow: loadBatteryInfo
  },
  {
    id: 'performance',
    title: 'Performance',
    subtitle: 'Optimize CPU and power settings',
    element: 'performanceView',
    display: 'block',
    onShow: loadPerformanceInfo
  },
  {
    id: 'monitor',
    title: 'System Monitor',
    subtitle: 'Real-time resource monitoring',
    element: 'monitorView',
    display: 'block',
    onShow: startMonitoring,
    onHide: stopMonitoring
  },
  {
    id: 'system',
    title: 'System Info',
    subtitle: 'Your ThinkPad details',
    element: 'systemView',
    display: 'block',
    onShow: loadSystemInfo
  },
  {
    id: 'security',
    title: 'Security',
    subtitle: 'Antivirus protection and security settings',
    element: 'securityView',
    display: 'block',
    onShow: loadSecurityStatus
  },
  {
    id: 'mcp',
    title: 'AI Integration',
    subtitle: 'Connect AI assistants to your ThinkPad via MCP',
    element: 'mcpView',
    display: 'block',
    onShow: () => {
      setupMcpView();
      loadMcpStatus();
    }
  },
  {
    id: 'sync',
    title: 'Cloud Sync',
    subtitle: 'Sync settings across devices',
    element: 'syncView',
    display: 'block',
    onShow: checkSyncStatus
  }
];

export function getView(id) {
  return VIEWS.find((v) => v.id === id) ?? null;
}

/** The DOM element for a view, or null when templates failed to inject. */
export function viewElement(view) {
  return elements[view.element] ?? null;
}
