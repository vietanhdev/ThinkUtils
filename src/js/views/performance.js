// Performance View
const { invoke } = window.__TAURI__.core;
import { showStatus } from '../utils.js';

export async function loadPerformanceInfo() {
  try {
    const cpuResponse = await invoke('get_cpu_info');
    if (cpuResponse.success && cpuResponse.data) {
      displayCpuInfo(cpuResponse.data);
    }

    const profileResponse = await invoke('get_power_profile');
    if (profileResponse.success && profileResponse.data) {
      displayPowerProfiles(profileResponse.data);
    }

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

  const container = document.getElementById('governor-buttons');
  container.innerHTML = '';

  info.available_governors.forEach((gov) => {
    const btn = document.createElement('button');
    btn.className = `option-btn ${gov === info.governor ? 'active' : ''}`;
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

  profileData.available.forEach((profile) => {
    const btn = document.createElement('button');
    btn.className = `option-btn ${profile === profileData.current ? 'active' : ''}`;
    btn.textContent = profile
      .split('-')
      .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
      .join(' ');
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
