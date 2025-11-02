// Home View
const { invoke } = window.__TAURI__.core;
import { showStatus } from '../utils.js';

export async function updateHomeView() {
  try {
    const response = await invoke('get_sensor_data');

    if (response.success && response.data) {
      const cpuTemp = Object.entries(response.data.temps).find(
        ([key]) => key.toLowerCase().includes('cpu') || key.toLowerCase().includes('core')
      );
      if (cpuTemp) {
        const cpuTempEl = document.getElementById('home-cpu-temp');
        if (cpuTempEl) {
          cpuTempEl.textContent = cpuTemp[1];
        }
      }

      if (response.data.fans) {
        const fanSpeed = response.data.fans.Fan1 || response.data.fans.fan1;
        const fanStatus = response.data.fans.status;

        if (fanSpeed) {
          const fanSpeedEl = document.getElementById('home-fan-speed');
          if (fanSpeedEl) {
            fanSpeedEl.textContent = fanSpeed;
          }
        }

        if (fanStatus) {
          const fanModeEl = document.getElementById('home-fan-mode');
          if (fanModeEl) {
            fanModeEl.textContent = fanStatus;
          }
        }
      }
    }
  } catch (error) {
    console.error('[Home] Update failed:', error);
  }

  try {
    const monitorResponse = await invoke('get_system_monitor');
    if (monitorResponse.success && monitorResponse.data) {
      const memory = monitorResponse.data.memory;
      if (memory) {
        const usagePercent = memory.usage_percent.toFixed(1);
        const usedGB = (memory.used / 1024 / 1024).toFixed(1);

        const memUsageEl = document.getElementById('home-memory-usage');
        const memUsedEl = document.getElementById('home-memory-used');

        if (memUsageEl) {
          memUsageEl.textContent = usagePercent + '%';
        }
        if (memUsedEl) {
          memUsedEl.textContent = usedGB + ' GB used';
        }
      }

      const cpu = monitorResponse.data.cpu;
      if (cpu) {
        const cpuUsage = cpu.usage_percent.toFixed(1);
        const cpuUsageEl = document.getElementById('home-cpu-usage');
        if (cpuUsageEl) {
          cpuUsageEl.textContent = cpuUsage + '%';
        }
      }
    }
  } catch (error) {
    console.error('[Home] Monitor update failed:', error);
  }

  try {
    const batteryResponse = await invoke('get_battery_info');
    if (batteryResponse.success && batteryResponse.data && batteryResponse.data.length > 0) {
      const battery = batteryResponse.data[0];
      const battLevelEl = document.getElementById('home-battery-level');
      const battStatusEl = document.getElementById('home-battery-status');

      if (battLevelEl) {
        battLevelEl.textContent = battery.capacity + '%';
      }
      if (battStatusEl) {
        battStatusEl.textContent = battery.status;
      }
    }

    const thresholdResponse = await invoke('get_battery_thresholds');
    if (thresholdResponse.success && thresholdResponse.data) {
      const thresholdEl = document.getElementById('home-battery-threshold');
      if (thresholdEl) {
        thresholdEl.textContent = `${thresholdResponse.data.start}% - ${thresholdResponse.data.stop}%`;
      }
    }
  } catch (error) {
    console.error('[Home] Battery update failed:', error);
  }

  try {
    const profileResponse = await invoke('get_power_profile');
    if (profileResponse.success && profileResponse.data) {
      const profileEl = document.getElementById('home-power-profile');
      if (profileEl) {
        profileEl.textContent = profileResponse.data.current
          .split('-')
          .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
          .join(' ');
      }

      document.querySelectorAll('.home-setting-btn[data-profile]').forEach((btn) => {
        btn.classList.remove('active');
        if (btn.dataset.profile === profileResponse.data.current) {
          btn.classList.add('active');
        }
      });
    }
  } catch (error) {
    console.error('[Home] Power profile update failed:', error);
  }

  try {
    const cpuResponse = await invoke('get_cpu_info');
    if (cpuResponse.success && cpuResponse.data) {
      const govEl = document.getElementById('home-cpu-governor');
      if (govEl) {
        govEl.textContent =
          cpuResponse.data.governor.charAt(0).toUpperCase() + cpuResponse.data.governor.slice(1);
      }

      document.querySelectorAll('.home-setting-btn[data-governor]').forEach((btn) => {
        btn.classList.remove('active');
        if (btn.dataset.governor === cpuResponse.data.governor) {
          btn.classList.add('active');
        }
      });
    }
  } catch (error) {
    console.error('[Home] CPU info update failed:', error);
  }

  try {
    const turboResponse = await invoke('get_turbo_boost_status');
    if (turboResponse.success) {
      const enabled = turboResponse.data;
      const statusEl = document.getElementById('home-turbo-status');
      if (statusEl) {
        statusEl.textContent = enabled ? 'Enabled' : 'Disabled';
      }
      const toggle = document.getElementById('home-turbo-toggle');
      if (toggle) {
        toggle.checked = enabled;
      }
    }
  } catch (error) {
    console.error('[Home] Turbo boost update failed:', error);
  }
}

export function setupHomeActions() {
  const profileBtns = document.querySelectorAll('.home-setting-btn[data-profile]');
  profileBtns.forEach((btn) => {
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

  const governorBtns = document.querySelectorAll('.home-setting-btn[data-governor]');
  governorBtns.forEach((btn) => {
    btn.addEventListener('click', async () => {
      const governor = btn.dataset.governor;
      governorBtns.forEach((b) => (b.disabled = true));

      try {
        showStatus(`Setting CPU governor to ${governor}...`, 'info');
        const response = await invoke('set_cpu_governor', { governor });

        if (response.success) {
          showStatus(`✓ CPU governor set to ${governor}`, 'success');
          await new Promise((resolve) => setTimeout(resolve, 500));
          await updateHomeView();
        } else {
          showStatus(`Error: ${response.error || 'Failed to set governor'}`, 'error');
        }
      } catch (error) {
        showStatus(`Error: ${error}`, 'error');
      } finally {
        governorBtns.forEach((b) => (b.disabled = false));
      }
    });
  });

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
          e.target.checked = !enabled;
        }
      } catch (error) {
        showStatus(`Error: ${error}`, 'error');
        e.target.checked = !enabled;
      }
    });
  }
}
