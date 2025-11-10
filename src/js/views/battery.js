// Battery View
const { invoke } = window.__TAURI__.core;
import { elements } from '../dom.js';
import { showStatus } from '../utils.js';

export function setupBatteryHandlers() {
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

export async function loadBatteryInfo() {
  try {
    const response = await invoke('get_battery_info');
    if (response.success && response.data) {
      displayBatteries(response.data);
    }

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

  batteries.forEach((battery) => {
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
