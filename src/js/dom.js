// DOM Element References
export const elements = {};

export function initializeElements() {
  // Page header
  elements.pageTitle = document.getElementById('page-title');
  elements.pageSubtitle = document.getElementById('page-subtitle');

  // Fan control
  elements.tempMetrics = document.getElementById('temp-metrics');
  elements.fanMetrics = document.getElementById('fan-metrics');
  elements.slider = document.getElementById('fan-slider');
  elements.sliderValue = document.getElementById('slider-value');
  elements.sliderSection = document.getElementById('slider-section');
  elements.btnAuto = document.getElementById('btn-auto');
  elements.btnManual = document.getElementById('btn-manual');
  elements.btnFull = document.getElementById('btn-full');
  elements.currentMode = document.getElementById('current-mode');
  elements.statusMessage = document.getElementById('status-message');
  elements.fanStatusBanner = document.querySelector('.fan-status-banner');
  elements.aboutLink = document.getElementById('about-link');
  elements.aboutDialog = document.getElementById('about-dialog');
  elements.closeAbout = document.getElementById('close-about');
  elements.permissionHelper = document.getElementById('permission-helper');
  elements.btnGrantPermissions = document.getElementById('btn-grant-permissions');

  // Views
  elements.homeView = document.getElementById('home-view');
  elements.fanView = document.getElementById('fan-view');
  elements.syncView = document.getElementById('sync-view');
  elements.systemView = document.getElementById('system-view');
  elements.batteryView = document.getElementById('battery-view');
  elements.performanceView = document.getElementById('performance-view');
  elements.monitorView = document.getElementById('monitor-view');

  // Sync
  elements.btnGoogleLogin = document.getElementById('btn-google-login');
  elements.btnGoogleLogout = document.getElementById('btn-google-logout');
  elements.btnSyncNow = document.getElementById('btn-sync-now');
  elements.btnDownloadSettings = document.getElementById('btn-download-settings');
  elements.syncLogin = document.getElementById('sync-login');
  elements.syncDashboard = document.getElementById('sync-dashboard');

  // Battery
  elements.thresholdStart = document.getElementById('threshold-start');
  elements.thresholdStop = document.getElementById('threshold-stop');
  elements.thresholdStartValue = document.getElementById('threshold-start-value');
  elements.thresholdStopValue = document.getElementById('threshold-stop-value');
  elements.btnApplyThresholds = document.getElementById('btn-apply-thresholds');
}
