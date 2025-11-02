// Fan Control View
const { invoke } = window.__TAURI__.core;
import { elements } from '../dom.js';
import { setState, getState } from '../state.js';
import { showStatus } from '../utils.js';

export function setupFanControl() {
  elements.slider.addEventListener('input', (e) => {
    elements.sliderValue.textContent = e.target.value;
  });

  elements.slider.addEventListener('change', (e) => {
    setFanMode('manual', e.target.value);
  });

  elements.btnAuto.addEventListener('click', () => setFanMode('auto'));
  elements.btnManual.addEventListener('click', () => setFanMode('manual', elements.slider.value));
  elements.btnFull.addEventListener('click', () => setFanMode('full'));

  elements.btnGrantPermissions.addEventListener('click', async () => {
    await tryUpdatePermissions();
  });
}

export async function checkInitialPermissions() {
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

export async function updateSensorData() {
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
    elements.tempMetrics.innerHTML =
      '<div class="metric-row"><span class="metric-label">No data</span></div>';
    return;
  }

  Object.entries(temps)
    .sort((a, b) => a[0].localeCompare(b[0]))
    .forEach(([label, value]) => {
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
    elements.fanMetrics.innerHTML =
      '<div class="metric-row"><span class="metric-label">No data</span></div>';
    return;
  }

  if (fans.level && !getState('fanControlInProgress')) {
    updateUIFromFanLevel(fans.level);
  }

  const sortOrder = ['Fan1', 'status', 'level'];
  Object.entries(fans)
    .sort((a, b) => {
      const indexA = sortOrder.indexOf(a[0]);
      const indexB = sortOrder.indexOf(b[0]);
      if (indexA !== -1 && indexB !== -1) {
        return indexA - indexB;
      }
      if (indexA !== -1) {
        return -1;
      }
      if (indexB !== -1) {
        return 1;
      }
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

  [elements.btnAuto, elements.btnManual, elements.btnFull].forEach((btn) => {
    btn.classList.remove('active');
  });

  if (levelStr === 'auto') {
    setState('currentFanMode', 'auto');
    elements.btnAuto.classList.add('active');
    elements.currentMode.textContent = 'AUTO';
    elements.sliderSection.style.display = 'none';
  } else if (levelStr === 'full-speed' || levelStr === 'disengaged') {
    setState('currentFanMode', 'full');
    elements.btnFull.classList.add('active');
    elements.currentMode.textContent = 'MAX';
    elements.sliderSection.style.display = 'none';
  } else if (!isNaN(parseInt(levelStr))) {
    setState('currentFanMode', 'manual');
    const numLevel = parseInt(levelStr);
    elements.slider.value = numLevel;
    elements.sliderValue.textContent = numLevel;
    elements.btnManual.classList.add('active');
    elements.currentMode.textContent = `LEVEL ${numLevel}`;
    elements.sliderSection.style.display = 'block';
  }
}

async function setFanMode(mode, level = null) {
  if (getState('fanControlInProgress')) {
    showStatus('Please wait...', 'info');
    return;
  }

  setState('currentFanMode', mode);

  [elements.btnAuto, elements.btnManual, elements.btnFull].forEach((btn) => {
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

  if (getState('lastFanSpeedSet') === speedValue) {
    showStatus(statusText, 'success');
    return;
  }

  setState('fanControlInProgress', true);
  setState('lastFanSpeedSet', speedValue);

  showStatus('Setting fan speed...', 'info');

  try {
    const response = await invoke('set_fan_speed', { speed: speedValue });

    if (response.success) {
      showStatus(statusText, 'success');
      await new Promise((resolve) => setTimeout(resolve, 500));
      await updateSensorData();
    } else {
      showStatus(`Error: ${response.error}`, 'error');
      setState('lastFanSpeedSet', null);

      if (response.error && response.error.includes('Permission')) {
        elements.permissionHelper.style.display = 'flex';
      }
    }
  } catch (error) {
    showStatus(`Error: ${error}`, 'error');
    setState('lastFanSpeedSet', null);
  } finally {
    setState('fanControlInProgress', false);
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

export function startAutoUpdate() {
  updateSensorData();
  const interval = setInterval(updateSensorData, 1000);
  setState('updateInterval', interval);
}
