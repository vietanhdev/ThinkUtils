// System Info View
const { invoke } = window.__TAURI__.core;

export async function loadSystemInfo() {
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
    document.querySelectorAll('.info-content p').forEach((p) => {
      p.textContent = 'Error loading';
    });
  }
}
