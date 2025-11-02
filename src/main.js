console.log('[ThinkUtils] Script loaded');

const { invoke } = window.__TAURI__.core;

// State
let currentFanMode = 'auto';
let updateInterval = null;
let fanControlInProgress = false;
let lastFanSpeedSet = null;

// DOM Elements
const elements = {};
let currentView = 'home';

// Initialize
window.addEventListener("DOMContentLoaded", () => {
  console.log('[ThinkUtils] Initializing...');
  initializeElements();
  setupEventListeners();
  setupFeatureNavigation();
  setupHomeActions();
  setupTitlebar();
  checkInitialPermissions();
  startAutoUpdate();
  console.log('[ThinkUtils] Ready');
});

function initializeElements() {
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

function setupEventListeners() {
  // Slider
  elements.slider.addEventListener('input', (e) => {
    elements.sliderValue.textContent = e.target.value;
  });

  elements.slider.addEventListener('change', (e) => {
    setFanMode('manual', e.target.value);
  });

  // Mode buttons
  elements.btnAuto.addEventListener('click', () => setFanMode('auto'));
  elements.btnManual.addEventListener('click', () => setFanMode('manual', elements.slider.value));
  elements.btnFull.addEventListener('click', () => setFanMode('full'));

  // About
  elements.aboutLink.addEventListener('click', (e) => {
    e.preventDefault();
    e.stopPropagation();
    showAbout();
  });
  
  // Close About
  if (elements.closeAbout) {
    elements.closeAbout.addEventListener('click', closeAbout);
  }

  // Grant permissions
  elements.btnGrantPermissions.addEventListener('click', async () => {
    await tryUpdatePermissions();
  });
  
  // Sync
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
  
  // Battery
  if (elements.thresholdStart) {
    elements.thresholdStart.addEventListener('input', (e) => {
      elements.thresholdStartValue.textContent = e.target.value + '%';
    });
  }
  if (elements.thresholdStop) {
    elements.thresholdStop.addEventListener('input', (e) => {
      elements.thresholdStopValue.textContent = e.target.value + '%';
    });
  }
  if (elements.btnApplyThresholds) {
    elements.btnApplyThresholds.addEventListener('click', applyBatteryThresholds);
  }
}

function setupFeatureNavigation() {
  const menuItems = document.querySelectorAll('.menu-item');
  
  menuItems.forEach(item => {
    item.addEventListener('click', (e) => {
      e.preventDefault();
      e.stopPropagation();
      
      if (item.classList.contains('disabled')) {
        console.log('[Navigation] Feature disabled:', item.dataset.feature);
        return;
      }
      
      const feature = item.dataset.feature;
      console.log('[Navigation] Switching to:', feature);
      
      // Update active state
      menuItems.forEach(i => i.classList.remove('active'));
      item.classList.add('active');
      
      // Switch view
      switchView(feature);
    });
  });
}

function setupTitlebar() {
  console.log('[Titlebar] Setting up window controls');
  
  const titlebar = document.querySelector('.titlebar');
  const minimizeBtn = document.getElementById('minimize-btn');
  const maximizeBtn = document.getElementById('maximize-btn');
  const closeBtn = document.getElementById('close-btn');
  
  if (!minimizeBtn || !maximizeBtn || !closeBtn) {
    console.error('[Titlebar] Buttons not found');
    return;
  }
  
  // Window dragging
  if (titlebar) {
    let isDragging = false;
    
    titlebar.addEventListener('mousedown', async (e) => {
      // Don't drag if clicking on buttons
      if (e.target.closest('.titlebar-button')) {
        return;
      }
      
      isDragging = true;
      try {
        await invoke('start_drag');
      } catch (e) {
        console.error('[Titlebar] Drag failed:', e);
      }
    });
    
    // Double click to maximize/restore
    titlebar.addEventListener('dblclick', async (e) => {
      if (e.target.closest('.titlebar-button')) {
        return;
      }
      
      try {
        await invoke('toggle_maximize');
      } catch (e) {
        console.error('[Titlebar] Double-click maximize failed:', e);
      }
    });
  }
  
  minimizeBtn.addEventListener('click', async () => {
    console.log('[Titlebar] Minimize clicked');
    try {
      await invoke('minimize_window');
    } catch (e) {
      console.error('[Titlebar] Minimize failed:', e);
    }
  });
  
  maximizeBtn.addEventListener('click', async () => {
    console.log('[Titlebar] Maximize clicked');
    try {
      await invoke('toggle_maximize');
    } catch (e) {
      console.error('[Titlebar] Maximize failed:', e);
    }
  });
  
  closeBtn.addEventListener('click', async () => {
    console.log('[Titlebar] Close clicked');
    try {
      await invoke('close_window');
    } catch (e) {
      console.error('[Titlebar] Close failed:', e);
    }
  });
}

function switchView(view) {
  currentView = view;
  
  // Hide all views
  if (elements.homeView) elements.homeView.style.display = 'none';
  if (elements.fanView) elements.fanView.style.display = 'none';
  if (elements.syncView) elements.syncView.style.display = 'none';
  if (elements.systemView) elements.systemView.style.display = 'none';
  if (elements.batteryView) elements.batteryView.style.display = 'none';
  if (elements.performanceView) elements.performanceView.style.display = 'none';
  if (elements.monitorView) elements.monitorView.style.display = 'none';
  
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
      if (elements.fanView) elements.fanView.style.display = 'grid';
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

// Home View Update
async function updateHomeView() {
  try {
    // Get sensor data for home view
    const response = await invoke('get_sensor_data');
    
    if (response.success && response.data) {
      // Update CPU temperature
      const cpuTemp = Object.entries(response.data.temps).find(([key]) => 
        key.toLowerCase().includes('cpu') || key.toLowerCase().includes('core')
      );
      if (cpuTemp) {
        document.getElementById('home-cpu-temp').textContent = cpuTemp[1];
      }
      
      // Update Fan data
      if (response.data.fans) {
        const fanSpeed = response.data.fans.Fan1 || response.data.fans.fan1;
        const fanLevel = response.data.fans.level;
        const fanStatus = response.data.fans.status;
        
        if (fanSpeed) {
          document.getElementById('home-fan-speed').textContent = fanSpeed;
          const speedValue = parseInt(fanSpeed);
          document.getElementById('home-fan-bar').style.width = Math.min((speedValue / 6500) * 100, 100) + '%';
        }
        
        if (fanLevel) {
          document.getElementById('home-fan-level').textContent = `Level ${fanLevel}`;
        }
        
        if (fanStatus) {
          document.getElementById('home-fan-mode').textContent = fanStatus;
        }
      }
    }
  } catch (error) {
    console.error('[Home] Update failed:', error);
  }
  
  // Update memory info
  try {
    const monitorResponse = await invoke('get_system_monitor');
    if (monitorResponse.success && monitorResponse.data) {
      const memory = monitorResponse.data.memory;
      if (memory) {
        const usagePercent = memory.usage_percent.toFixed(1);
        const totalGB = (memory.total / 1024 / 1024).toFixed(1);
        const usedGB = (memory.used / 1024 / 1024).toFixed(1);
        
        document.getElementById('home-memory-usage').textContent = usagePercent + '%';
        document.getElementById('home-memory-total').textContent = totalGB + ' GB';
        document.getElementById('home-memory-used').textContent = usedGB + ' GB used';
        document.getElementById('home-memory-bar').style.width = usagePercent + '%';
      }
      
      const cpu = monitorResponse.data.cpu;
      if (cpu) {
        const cpuUsage = cpu.usage_percent.toFixed(1);
        document.getElementById('home-cpu-usage').textContent = cpuUsage + '%';
        // Update CPU bar based on usage
        document.getElementById('home-cpu-bar').style.width = cpuUsage + '%';
      }
    }
  } catch (error) {
    console.error('[Home] Monitor update failed:', error);
  }
  
  // Update battery info
  try {
    const batteryResponse = await invoke('get_battery_info');
    if (batteryResponse.success && batteryResponse.data && batteryResponse.data.length > 0) {
      const battery = batteryResponse.data[0];
      document.getElementById('home-battery-level').textContent = battery.capacity + '%';
      document.getElementById('home-battery-status').textContent = battery.status;
      document.getElementById('home-battery-health').textContent = `Health: ${battery.health}%`;
      document.getElementById('home-battery-bar').style.width = battery.capacity + '%';
    }
    
    // Update battery threshold display
    const thresholdResponse = await invoke('get_battery_thresholds');
    if (thresholdResponse.success && thresholdResponse.data) {
      document.getElementById('home-battery-threshold').textContent = 
        `${thresholdResponse.data.start}% - ${thresholdResponse.data.stop}%`;
    }
  } catch (error) {
    console.error('[Home] Battery update failed:', error);
  }
  
  // Update system info
  try {
    const systemResponse = await invoke('get_system_info');
    if (systemResponse.success && systemResponse.data) {
      const info = systemResponse.data;
      document.getElementById('home-system-model').textContent = info.model;
      document.getElementById('home-system-cpu').textContent = info.cpu;
      document.getElementById('home-system-memory').textContent = info.memory;
      document.getElementById('home-system-os').textContent = info.os;
    }
  } catch (error) {
    console.error('[Home] System info update failed:', error);
  }
  
  // Update power profile
  try {
    const profileResponse = await invoke('get_power_profile');
    if (profileResponse.success && profileResponse.data) {
      document.getElementById('home-power-profile').textContent = 
        profileResponse.data.current.split('-').map(w => w.charAt(0).toUpperCase() + w.slice(1)).join(' ');
      
      // Update active button
      document.querySelectorAll('.home-setting-btn[data-profile]').forEach(btn => {
        btn.classList.remove('active');
        if (btn.dataset.profile === profileResponse.data.current) {
          btn.classList.add('active');
        }
      });
    }
  } catch (error) {
    console.error('[Home] Power profile update failed:', error);
  }
  
  // Update CPU governor and frequency
  try {
    const cpuResponse = await invoke('get_cpu_info');
    if (cpuResponse.success && cpuResponse.data) {
      document.getElementById('home-cpu-governor').textContent = 
        cpuResponse.data.governor.charAt(0).toUpperCase() + cpuResponse.data.governor.slice(1);
      
      // Update CPU frequency display
      if (cpuResponse.data.current_freq) {
        const freqGHz = (cpuResponse.data.current_freq / 1000).toFixed(2);
        document.getElementById('home-cpu-freq').textContent = `${freqGHz} GHz`;
      }
      
      // Update active button
      document.querySelectorAll('.home-setting-btn[data-governor]').forEach(btn => {
        btn.classList.remove('active');
        if (btn.dataset.governor === cpuResponse.data.governor) {
          btn.classList.add('active');
        }
      });
    }
  } catch (error) {
    console.error('[Home] CPU info update failed:', error);
  }
  
  // Update turbo boost status
  try {
    const turboResponse = await invoke('get_turbo_boost_status');
    if (turboResponse.success) {
      const enabled = turboResponse.data;
      document.getElementById('home-turbo-status').textContent = enabled ? 'Enabled' : 'Disabled';
      const toggle = document.getElementById('home-turbo-toggle');
      if (toggle) {
        toggle.checked = enabled;
      }
    }
  } catch (error) {
    console.error('[Home] Turbo boost update failed:', error);
  }
}

// Setup home quick action buttons
function setupHomeActions() {
  const actionCards = document.querySelectorAll('.home-action-card');
  actionCards.forEach(card => {
    card.addEventListener('click', () => {
      const feature = card.dataset.feature;
      if (feature) {
        // Find the menu item and trigger click
        const menuItem = document.querySelector(`.menu-item[data-feature="${feature}"]`);
        if (menuItem) {
          menuItem.click();
        }
      }
    });
  });
  
  // Setup power profile buttons
  const profileBtns = document.querySelectorAll('.home-setting-btn[data-profile]');
  profileBtns.forEach(btn => {
    btn.addEventListener('click', async () => {
      const profile = btn.dataset.profile;
      try {
        showStatus(`Setting power profile to ${profile}...`, 'info');
        const response = await invoke('set_power_profile', { profile });
        if (response.success) {
          showStatus(`✓ Power profile: ${profile}`, 'success');
          updateHomeView();
        } else {
          showStatus(`Error: ${response.error}`, 'error');
        }
      } catch (error) {
        showStatus(`Error: ${error}`, 'error');
      }
    });
  });
  
  // Setup CPU governor buttons
  const governorBtns = document.querySelectorAll('.home-setting-btn[data-governor]');
  governorBtns.forEach(btn => {
    btn.addEventListener('click', async () => {
      const governor = btn.dataset.governor;
      try {
        showStatus(`Setting CPU governor to ${governor}...`, 'info');
        const response = await invoke('set_cpu_governor', { governor });
        if (response.success) {
          showStatus(`✓ CPU governor: ${governor}`, 'success');
          updateHomeView();
        } else {
          showStatus(`Error: ${response.error}`, 'error');
        }
      } catch (error) {
        showStatus(`Error: ${error}`, 'error');
      }
    });
  });
  
  // Setup turbo boost toggle
  const turboToggle = document.getElementById('home-turbo-toggle');
  if (turboToggle) {
    turboToggle.addEventListener('change', async (e) => {
      const enabled = e.target.checked;
      try {
        showStatus(`${enabled ? 'Enabling' : 'Disabling'} turbo boost...`, 'info');
        const response = await invoke('set_turbo_boost', { enabled });
        if (response.success) {
          showStatus(`✓ Turbo boost ${enabled ? 'enabled' : 'disabled'}`, 'success');
          updateHomeView();
        } else {
          showStatus(`Error: ${response.error}`, 'error');
          e.target.checked = !enabled; // Revert on error
        }
      } catch (error) {
        showStatus(`Error: ${error}`, 'error');
        e.target.checked = !enabled; // Revert on error
      }
    });
  }
}

async function checkInitialPermissions() {
  try {
    const response = await invoke('check_permissions');
    if (!response.success || !response.data) {
      elements.permissionHelper.style.display = 'flex';
    } else {
      elements.permissionHelper.style.display = 'none';
    }
  } catch (error) {
    console.error('[Permissions] Check failed:', error);
  }
}

async function updateSensorData() {
  try {
    const response = await invoke('get_sensor_data');
    
    if (response.success && response.data) {
      updateTemperatureDisplay(response.data.temps);
      updateFanDisplay(response.data.fans);
    }
  } catch (error) {
    console.error('[Sensors] Update failed:', error);
  }
}

function updateTemperatureDisplay(temps) {
  elements.tempMetrics.innerHTML = '';
  
  if (Object.keys(temps).length === 0) {
    elements.tempMetrics.innerHTML = '<div class="metric-row"><span class="metric-label">No data</span></div>';
    return;
  }

  Object.entries(temps).sort((a, b) => a[0].localeCompare(b[0])).forEach(([label, value]) => {
    const row = document.createElement('div');
    row.className = 'metric-row';
    row.innerHTML = `
      <span class="metric-label">${label}</span>
      <span class="metric-value">${value}</span>
    `;
    elements.tempMetrics.appendChild(row);
  });
}

function updateFanDisplay(fans) {
  elements.fanMetrics.innerHTML = '';
  
  if (Object.keys(fans).length === 0) {
    elements.fanMetrics.innerHTML = '<div class="metric-row"><span class="metric-label">No data</span></div>';
    return;
  }

  // Update UI from fan level
  if (fans.level && !fanControlInProgress) {
    updateUIFromFanLevel(fans.level);
  }

  // Display fan metrics
  const sortOrder = ['Fan1', 'status', 'level'];
  Object.entries(fans)
    .sort((a, b) => {
      const indexA = sortOrder.indexOf(a[0]);
      const indexB = sortOrder.indexOf(b[0]);
      if (indexA !== -1 && indexB !== -1) return indexA - indexB;
      if (indexA !== -1) return -1;
      if (indexB !== -1) return 1;
      return a[0].localeCompare(b[0]);
    })
    .forEach(([label, value]) => {
      const row = document.createElement('div');
      row.className = label === 'Fan1' ? 'metric-row highlight' : 'metric-row';
      row.innerHTML = `
        <span class="metric-label">${label}</span>
        <span class="metric-value">${value}</span>
      `;
      elements.fanMetrics.appendChild(row);
    });
}

function updateUIFromFanLevel(level) {
  const levelStr = level.toString().toLowerCase();
  
  // Update buttons
  [elements.btnAuto, elements.btnManual, elements.btnFull].forEach(btn => {
    btn.classList.remove('active');
  });
  
  if (levelStr === 'auto') {
    currentFanMode = 'auto';
    elements.btnAuto.classList.add('active');
    elements.currentMode.textContent = 'AUTO';
    elements.sliderSection.style.display = 'none';
  } else if (levelStr === 'full-speed' || levelStr === 'disengaged') {
    currentFanMode = 'full';
    elements.btnFull.classList.add('active');
    elements.currentMode.textContent = 'MAX';
    elements.sliderSection.style.display = 'none';
  } else if (!isNaN(parseInt(levelStr))) {
    currentFanMode = 'manual';
    const numLevel = parseInt(levelStr);
    elements.slider.value = numLevel;
    elements.sliderValue.textContent = numLevel;
    elements.btnManual.classList.add('active');
    elements.currentMode.textContent = `LEVEL ${numLevel}`;
    elements.sliderSection.style.display = 'block';
  }
}

async function setFanMode(mode, level = null) {
  if (fanControlInProgress) {
    showStatus('Please wait...', 'info');
    return;
  }
  
  currentFanMode = mode;
  
  // Update UI
  [elements.btnAuto, elements.btnManual, elements.btnFull].forEach(btn => {
    btn.classList.remove('active');
  });
  
  let speedValue;
  let statusText;
  
  switch (mode) {
    case 'auto':
      elements.btnAuto.classList.add('active');
      elements.currentMode.textContent = 'AUTO';
      elements.sliderSection.style.display = 'none';
      speedValue = 'auto';
      statusText = 'Fan mode: Auto';
      break;
    
    case 'manual':
      elements.btnManual.classList.add('active');
      speedValue = level || elements.slider.value;
      elements.currentMode.textContent = `LEVEL ${speedValue}`;
      elements.sliderSection.style.display = 'block';
      statusText = `Fan level: ${speedValue}`;
      break;
    
    case 'full':
      elements.btnFull.classList.add('active');
      elements.currentMode.textContent = 'MAX';
      elements.sliderSection.style.display = 'none';
      speedValue = 'full-speed';
      statusText = 'Fan mode: Maximum';
      break;
  }
  
  if (lastFanSpeedSet === speedValue) {
    showStatus(statusText, 'success');
    return;
  }
  
  fanControlInProgress = true;
  lastFanSpeedSet = speedValue;
  
  showStatus('Setting fan speed...', 'info');
  
  try {
    const response = await invoke('set_fan_speed', { speed: speedValue });
    
    if (response.success) {
      showStatus(statusText, 'success');
      await new Promise(resolve => setTimeout(resolve, 500));
      await updateSensorData();
    } else {
      showStatus(`Error: ${response.error}`, 'error');
      lastFanSpeedSet = null;
      
      if (response.error && response.error.includes('Permission')) {
        elements.permissionHelper.style.display = 'flex';
      }
    }
  } catch (error) {
    showStatus(`Error: ${error}`, 'error');
    lastFanSpeedSet = null;
  } finally {
    fanControlInProgress = false;
  }
}

async function tryUpdatePermissions() {
  try {
    showStatus('Requesting permissions...', 'info');
    
    const response = await invoke('update_permissions');
    
    if (response.success) {
      showStatus('✓ Permissions granted!', 'success');
      elements.permissionHelper.style.display = 'none';
      await checkInitialPermissions();
    } else {
      showStatus(`Failed: ${response.error}`, 'error');
    }
  } catch (error) {
    showStatus(`Error: ${error}`, 'error');
  }
}

function showStatus(message, type = 'info') {
  // Try to find the fan status banner first, fallback to regular status message
  const statusEl = elements.fanStatusBanner || elements.statusMessage;
  
  if (statusEl) {
    statusEl.textContent = message;
    statusEl.className = statusEl.classList.contains('fan-status-banner') 
      ? `fan-status-banner ${type}` 
      : `status-banner ${type}`;
    statusEl.style.display = 'block';
    
    const timeout = type === 'error' ? 10000 : 5000;
    
    setTimeout(() => {
      if (statusEl.textContent === message) {
        statusEl.style.display = 'none';
      }
    }, timeout);
  }
}

function startAutoUpdate() {
  updateSensorData();
  updateInterval = setInterval(updateSensorData, 1000);
  
  // Update home view periodically if it's active
  setInterval(() => {
    if (currentView === 'home') {
      updateHomeView();
    }
  }, 2000);
}

// Sync Functions
async function checkSyncStatus() {
  try {
    const response = await invoke('google_auth_status');
    
    if (response.success && response.data && response.data.is_logged_in) {
      // User is logged in
      elements.syncLogin.style.display = 'none';
      elements.syncDashboard.style.display = 'block';
      
      // Update UI with user info
      document.getElementById('user-email').textContent = response.data.user_email || 'Unknown';
      document.getElementById('last-sync').textContent = response.data.last_sync 
        ? `Last synced: ${response.data.last_sync}` 
        : 'Last synced: Never';
      
      // Display synced settings
      updateSyncedSettingsDisplay(response.data.settings);
    } else {
      // User is not logged in
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
      // Open auth URL in browser using Tauri command
      console.log('[OAuth] Opening URL:', response.data.auth_url);
      
      try {
        await invoke('open_url', { url: response.data.auth_url });
      } catch (openError) {
        console.error('[OAuth] Failed to open URL:', openError);
        // Fallback: show URL to user
        const userAction = confirm(
          'Unable to open browser automatically.\n\n' +
          'Click OK to copy the login URL to clipboard.'
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
      
      // Poll for login completion
      let attempts = 0;
      const maxAttempts = 60; // 60 seconds
      
      const checkInterval = setInterval(async () => {
        attempts++;
        
        const statusResponse = await invoke('google_auth_status');
        
        if (statusResponse.success && statusResponse.data && statusResponse.data.is_logged_in) {
          clearInterval(checkInterval);
          
          elements.syncLogin.style.display = 'none';
          elements.syncDashboard.style.display = 'block';
          
          document.getElementById('user-email').textContent = statusResponse.data.user_email;
          document.getElementById('last-sync').textContent = `Last synced: ${statusResponse.data.last_sync}`;
          
          updateSyncedSettingsDisplay(statusResponse.data.settings);
          
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
    
    const settings = {
      fan_mode: currentFanMode,
      fan_level: parseInt(elements.slider.value),
      auto_start: false,
      minimize_to_tray: true,
      theme: 'system',
      battery_start_threshold: elements.thresholdStart ? parseInt(elements.thresholdStart.value) : 40,
      battery_stop_threshold: elements.thresholdStop ? parseInt(elements.thresholdStop.value) : 80,
    };
    
    const response = await invoke('sync_to_cloud', { settings });
    
    if (response.success) {
      // Refresh status to get updated last_sync time
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
      // Apply downloaded settings
      const settings = response.data;
      
      // Apply fan settings
      if (settings.fan_mode) {
        await setFanMode(settings.fan_mode, settings.fan_level);
      }
      
      // Apply battery thresholds
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
  document.getElementById('synced-fan-mode').textContent = settings.fan_mode || 'auto';
  document.getElementById('synced-theme').textContent = settings.theme || 'system';
  document.getElementById('synced-autostart').textContent = settings.auto_start ? 'Enabled' : 'Disabled';
}

// System Info Functions
async function loadSystemInfo() {
  try {
    const response = await invoke('get_system_info');
    
    if (response.success && response.data) {
      const info = response.data;
      document.getElementById('system-model').textContent = info.model;
      document.getElementById('system-cpu').textContent = info.cpu;
      document.getElementById('system-memory').textContent = info.memory;
      document.getElementById('system-os').textContent = info.os;
      document.getElementById('system-kernel').textContent = info.kernel;
      document.getElementById('system-hostname').textContent = info.hostname;
    }
  } catch (error) {
    console.error('[System] Info load failed:', error);
    document.querySelectorAll('.info-content p').forEach(p => {
      p.textContent = 'Error loading';
    });
  }
}

// Battery Functions
async function loadBatteryInfo() {
  try {
    const response = await invoke('get_battery_info');
    
    if (response.success && response.data) {
      displayBatteries(response.data);
    }
    
    // Load thresholds
    const thresholdResponse = await invoke('get_battery_thresholds');
    if (thresholdResponse.success && thresholdResponse.data) {
      elements.thresholdStart.value = thresholdResponse.data.start;
      elements.thresholdStop.value = thresholdResponse.data.stop;
      elements.thresholdStartValue.textContent = thresholdResponse.data.start + '%';
      elements.thresholdStopValue.textContent = thresholdResponse.data.stop + '%';
    }
  } catch (error) {
    console.error('[Battery] Load failed:', error);
  }
}

function displayBatteries(batteries) {
  const container = document.getElementById('battery-cards');
  container.innerHTML = '';
  
  batteries.forEach(battery => {
    const card = document.createElement('div');
    card.className = 'battery-card';
    card.innerHTML = `
      <div class="battery-header">
        <span class="battery-name">${battery.name}</span>
        <span class="battery-status">${battery.status}</span>
      </div>
      <div class="battery-capacity">${battery.capacity}%</div>
      <div class="battery-details">
        <div class="battery-detail">
          <span class="battery-detail-label">Health</span>
          <span class="battery-detail-value">${battery.health}%</span>
        </div>
        <div class="battery-detail">
          <span class="battery-detail-label">Cycles</span>
          <span class="battery-detail-value">${battery.cycles}</span>
        </div>
        <div class="battery-detail">
          <span class="battery-detail-label">Power</span>
          <span class="battery-detail-value">${battery.power.toFixed(1)}W</span>
        </div>
        <div class="battery-detail">
          <span class="battery-detail-label">Technology</span>
          <span class="battery-detail-value">${battery.technology}</span>
        </div>
      </div>
    `;
    container.appendChild(card);
  });
}

async function applyBatteryThresholds() {
  const start = parseInt(elements.thresholdStart.value);
  const stop = parseInt(elements.thresholdStop.value);
  
  try {
    showStatus('Setting battery thresholds...', 'info');
    
    const response = await invoke('set_battery_thresholds', { start, stop });
    
    if (response.success) {
      showStatus(`✓ Thresholds set: ${start}%-${stop}%`, 'success');
    } else {
      showStatus(`Error: ${response.error}`, 'error');
    }
  } catch (error) {
    showStatus(`Error: ${error}`, 'error');
  }
}

// Performance Functions
async function loadPerformanceInfo() {
  try {
    // Load CPU info
    const cpuResponse = await invoke('get_cpu_info');
    if (cpuResponse.success && cpuResponse.data) {
      displayCpuInfo(cpuResponse.data);
    }
    
    // Load power profile
    const profileResponse = await invoke('get_power_profile');
    if (profileResponse.success && profileResponse.data) {
      displayPowerProfiles(profileResponse.data);
    }
    
    // Load turbo boost status
    const turboResponse = await invoke('get_turbo_boost_status');
    if (turboResponse.success) {
      displayTurboStatus(turboResponse.data);
    }
  } catch (error) {
    console.error('[Performance] Load failed:', error);
  }
}

function displayCpuInfo(info) {
  document.getElementById('cpu-governor').textContent = info.governor;
  document.getElementById('cpu-freq').textContent = `${info.current_freq} MHz`;
  document.getElementById('cpu-freq-range').textContent = `${info.min_freq} - ${info.max_freq} MHz`;
  
  // Display governor buttons
  const container = document.getElementById('governor-buttons');
  container.innerHTML = '';
  
  info.available_governors.forEach(gov => {
    const btn = document.createElement('button');
    btn.className = `governor-btn ${gov === info.governor ? 'active' : ''}`;
    btn.textContent = gov.charAt(0).toUpperCase() + gov.slice(1);
    btn.onclick = () => setCpuGovernor(gov);
    container.appendChild(btn);
  });
}

async function setCpuGovernor(governor) {
  try {
    showStatus(`Setting CPU governor to ${governor}...`, 'info');
    
    const response = await invoke('set_cpu_governor', { governor });
    
    if (response.success) {
      showStatus(`✓ CPU governor: ${governor}`, 'success');
      await loadPerformanceInfo();
    } else {
      showStatus(`Error: ${response.error}`, 'error');
    }
  } catch (error) {
    showStatus(`Error: ${error}`, 'error');
  }
}

function displayPowerProfiles(profileData) {
  const container = document.getElementById('profile-buttons');
  container.innerHTML = '';
  
  profileData.available.forEach(profile => {
    const btn = document.createElement('button');
    btn.className = `profile-btn ${profile === profileData.current ? 'active' : ''}`;
    btn.textContent = profile.split('-').map(w => w.charAt(0).toUpperCase() + w.slice(1)).join(' ');
    btn.onclick = () => setPowerProfile(profile);
    container.appendChild(btn);
  });
}

async function setPowerProfile(profile) {
  try {
    showStatus(`Setting power profile to ${profile}...`, 'info');
    
    const response = await invoke('set_power_profile', { profile });
    
    if (response.success) {
      showStatus(`✓ Power profile: ${profile}`, 'success');
      await loadPerformanceInfo();
    } else {
      showStatus(`Error: ${response.error}`, 'error');
    }
  } catch (error) {
    showStatus(`Error: ${error}`, 'error');
  }
}

function displayTurboStatus(enabled) {
  const statusText = document.getElementById('turbo-status-text');
  const toggle = document.getElementById('turbo-toggle');
  
  if (statusText) {
    statusText.textContent = enabled ? 'Enabled' : 'Disabled';
    statusText.style.color = enabled ? 'var(--red-primary)' : 'var(--text-secondary)';
  }
  
  if (toggle) {
    toggle.checked = enabled;
    
    // Remove existing listener to avoid duplicates
    toggle.removeEventListener('change', handleTurboToggle);
    toggle.addEventListener('change', handleTurboToggle);
  }
}

function handleTurboToggle(e) {
  setTurboBoost(e.target.checked);
}

async function setTurboBoost(enabled) {
  try {
    showStatus(`${enabled ? 'Enabling' : 'Disabling'} turbo boost...`, 'info');
    
    const response = await invoke('set_turbo_boost', { enabled });
    
    if (response.success) {
      showStatus(`✓ Turbo boost ${enabled ? 'enabled' : 'disabled'}`, 'success');
      await loadPerformanceInfo();
    } else {
      showStatus(`Error: ${response.error}`, 'error');
    }
  } catch (error) {
    showStatus(`Error: ${error}`, 'error');
  }
}

// Monitor Functions
let monitorInterval = null;

async function startMonitoring() {
  await updateMonitorData();
  
  if (monitorInterval) {
    clearInterval(monitorInterval);
  }
  
  monitorInterval = setInterval(async () => {
    if (currentView === 'monitor') {
      await updateMonitorData();
    }
  }, 2000);
}

async function updateMonitorData() {
  try {
    const response = await invoke('get_system_monitor');
    
    if (response.success && response.data) {
      displayCpuMonitor(response.data.cpu);
      displayMemoryMonitor(response.data.memory);
      displayDiskMonitor(response.data.disk);
      displayNetworkMonitor(response.data.network);
      displayProcessMonitor(response.data.processes);
    }
  } catch (error) {
    console.error('[Monitor] Update failed:', error);
  }
}

function displayCpuMonitor(cpu) {
  const totalUsage = cpu.usage_percent.toFixed(1);
  document.getElementById('cpu-usage-total').textContent = totalUsage + '%';
  document.getElementById('cpu-usage-bar').style.width = totalUsage + '%';
  
  const loadAvg = cpu.load_avg;
  document.getElementById('load-avg').textContent = 
    `${loadAvg.one_min.toFixed(2)} / ${loadAvg.five_min.toFixed(2)} / ${loadAvg.fifteen_min.toFixed(2)}`;
  
  const coresContainer = document.getElementById('cpu-cores');
  coresContainer.innerHTML = '';
  
  cpu.cores.forEach(core => {
    const coreDiv = document.createElement('div');
    coreDiv.className = 'cpu-core-item';
    coreDiv.innerHTML = `
      <div class="cpu-core-header">
        <span class="cpu-core-label">Core ${core.core_id}</span>
        <span class="cpu-core-value">${core.usage_percent.toFixed(1)}%</span>
      </div>
      <div class="cpu-core-bar">
        <div class="cpu-core-bar-fill" style="width: ${core.usage_percent}%"></div>
      </div>
      <div class="cpu-core-freq">${(core.frequency / 1000).toFixed(0)} MHz</div>
    `;
    coresContainer.appendChild(coreDiv);
  });
}

function displayMemoryMonitor(memory) {
  const usagePercent = memory.usage_percent.toFixed(1);
  document.getElementById('memory-usage-total').textContent = usagePercent + '%';
  document.getElementById('memory-usage-bar').style.width = usagePercent + '%';
  
  const usedGB = (memory.used / 1024 / 1024).toFixed(1);
  const availableGB = (memory.available / 1024 / 1024).toFixed(1);
  const totalGB = (memory.total / 1024 / 1024).toFixed(1);
  
  document.getElementById('memory-used').textContent = usedGB + ' GB';
  document.getElementById('memory-available').textContent = availableGB + ' GB';
  document.getElementById('memory-total').textContent = totalGB + ' GB';
  
  const swapUsedMB = (memory.swap_used / 1024).toFixed(0);
  const swapTotalMB = (memory.swap_total / 1024).toFixed(0);
  const swapPercent = memory.swap_total > 0 
    ? ((memory.swap_used / memory.swap_total) * 100).toFixed(1)
    : 0;
  
  document.getElementById('swap-usage').textContent = `${swapUsedMB} MB / ${swapTotalMB} MB`;
  document.getElementById('swap-usage-bar').style.width = swapPercent + '%';
}

function displayDiskMonitor(disks) {
  const container = document.getElementById('disk-list');
  container.innerHTML = '';
  
  disks.forEach(disk => {
    const totalGB = (disk.total / 1024 / 1024 / 1024).toFixed(1);
    const usedGB = (disk.used / 1024 / 1024 / 1024).toFixed(1);
    const availableGB = (disk.available / 1024 / 1024 / 1024).toFixed(1);
    
    const diskDiv = document.createElement('div');
    diskDiv.className = 'disk-item';
    diskDiv.innerHTML = `
      <div class="disk-header">
        <span class="disk-name">${disk.mount_point}</span>
        <span class="disk-usage">${disk.usage_percent.toFixed(1)}%</span>
      </div>
      <div class="disk-progress">
        <div class="disk-progress-bar" style="width: ${disk.usage_percent}%"></div>
      </div>
      <div class="disk-details">
        <span class="disk-device">${disk.device}</span>
        <span class="disk-size">${usedGB} GB / ${totalGB} GB</span>
      </div>
    `;
    container.appendChild(diskDiv);
  });
}

function displayNetworkMonitor(interfaces) {
  const container = document.getElementById('network-list');
  container.innerHTML = '';
  
  interfaces.forEach(iface => {
    const rxMB = (iface.rx_bytes / 1024 / 1024).toFixed(2);
    const txMB = (iface.tx_bytes / 1024 / 1024).toFixed(2);
    
    const ifaceDiv = document.createElement('div');
    ifaceDiv.className = 'network-item';
    ifaceDiv.innerHTML = `
      <div class="network-header">
        <span class="network-name">${iface.interface}</span>
      </div>
      <div class="network-stats">
        <div class="network-stat">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="17 11 12 6 7 11"/>
            <polyline points="17 18 12 13 7 18"/>
          </svg>
          <span>${rxMB} MB</span>
        </div>
        <div class="network-stat">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="7 13 12 18 17 13"/>
            <polyline points="7 6 12 11 17 6"/>
          </svg>
          <span>${txMB} MB</span>
        </div>
      </div>
      <div class="network-packets">
        <span>RX: ${iface.rx_packets.toLocaleString()} packets</span>
        <span>TX: ${iface.tx_packets.toLocaleString()} packets</span>
      </div>
    `;
    container.appendChild(ifaceDiv);
  });
}

function displayProcessMonitor(processes) {
  const container = document.getElementById('process-list');
  container.innerHTML = '';
  
  processes.forEach(proc => {
    const procDiv = document.createElement('div');
    procDiv.className = 'process-row';
    procDiv.innerHTML = `
      <span class="process-col-pid">${proc.pid}</span>
      <span class="process-col-name">${proc.name}</span>
      <span class="process-col-cpu">${proc.cpu_percent.toFixed(1)}%</span>
      <span class="process-col-mem">${proc.memory_mb.toFixed(0)} MB</span>
      <span class="process-col-status">${proc.status}</span>
    `;
    container.appendChild(procDiv);
  });
}

window.addEventListener('beforeunload', () => {
  if (updateInterval) {
    clearInterval(updateInterval);
  }
  if (monitorInterval) {
    clearInterval(monitorInterval);
  }
});

// ===================================
// About Dialog
// ===================================

function showAbout() {
  console.log('[About] Opening dialog');
  const dialog = document.getElementById('about-dialog');
  if (dialog) {
    dialog.style.display = 'flex';
    console.log('[About] Dialog display set to flex');
    
    // Close on overlay click
    if (!dialog.hasAttribute('data-listener')) {
      dialog.setAttribute('data-listener', 'true');
      dialog.addEventListener('click', (e) => {
        if (e.target === dialog) {
          console.log('[About] Overlay clicked');
          closeAbout();
        }
      });
    }
    
    // Close on Escape key
    const escapeHandler = (e) => {
      if (e.key === 'Escape') {
        console.log('[About] Escape pressed');
        closeAbout();
        document.removeEventListener('keydown', escapeHandler);
      }
    };
    document.addEventListener('keydown', escapeHandler);
    
    // Setup link handlers
    setupAboutLinks();
  } else {
    console.error('[About] Dialog element not found');
  }
}

function closeAbout() {
  console.log('[About] Closing dialog');
  const dialog = document.getElementById('about-dialog');
  if (dialog) {
    dialog.style.display = 'none';
    console.log('[About] Dialog closed');
  }
}

function setupAboutLinks() {
  const githubLink = document.getElementById('link-github');
  const docsLink = document.getElementById('link-docs');
  
  if (githubLink && !githubLink.hasAttribute('data-listener')) {
    githubLink.setAttribute('data-listener', 'true');
    githubLink.addEventListener('click', (e) => {
      e.preventDefault();
      // Don't redirect - just prevent default
    });
  }
  
  if (docsLink && !docsLink.hasAttribute('data-listener')) {
    docsLink.setAttribute('data-listener', 'true');
    docsLink.addEventListener('click', (e) => {
      e.preventDefault();
      // Don't redirect - just prevent default
    });
  }
}

async function openExternal(url) {
  try {
    await invoke('open_url', { url });
  } catch (error) {
    console.error('Failed to open URL:', error);
  }
}
