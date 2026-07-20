// Performance View
const { invoke } = window.__TAURI__.core;
import { setPowerProfile, setCpuGovernor, setTurboBoost, bindOnce } from '../hardwareControls.js';

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
    btn.onclick = () => setCpuGovernor(gov, loadPerformanceInfo, Array.from(container.children));
    container.appendChild(btn);
  });
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
    btn.onclick = () => setPowerProfile(profile, loadPerformanceInfo);
    container.appendChild(btn);
  });
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
    // Rebound on every render, so bindOnce rather than add/remove of a named
    // handler -- the Home copy of this leaked a listener per render.
    bindOnce(toggle, 'change', (e) =>
      setTurboBoost(e.target.checked, loadPerformanceInfo, e.target)
    );
  }
}
