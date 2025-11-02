// Monitor View
const { invoke } = window.__TAURI__.core;
import { setState, getState } from '../state.js';

export async function startMonitoring() {
  await updateMonitorData();

  const interval = getState('monitorInterval');
  if (interval) {
    clearInterval(interval);
  }

  const newInterval = setInterval(async () => {
    if (getState('currentView') === 'monitor') {
      await updateMonitorData();
    }
  }, 2000);

  setState('monitorInterval', newInterval);
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

  cpu.cores.forEach((core) => {
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
  const swapPercent =
    memory.swap_total > 0 ? ((memory.swap_used / memory.swap_total) * 100).toFixed(1) : 0;

  document.getElementById('swap-usage').textContent = `${swapUsedMB} MB / ${swapTotalMB} MB`;
  document.getElementById('swap-usage-bar').style.width = swapPercent + '%';
}

function displayDiskMonitor(disks) {
  const container = document.getElementById('disk-list');
  container.innerHTML = '';

  disks.forEach((disk) => {
    const totalGB = (disk.total / 1024 / 1024 / 1024).toFixed(1);
    const usedGB = (disk.used / 1024 / 1024 / 1024).toFixed(1);

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

  interfaces.forEach((iface) => {
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

  processes.forEach((proc) => {
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
