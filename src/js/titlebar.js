// Titlebar Window Controls
const { invoke } = window.__TAURI__.core;

export function setupTitlebar() {
  console.log('[Titlebar] Setting up window controls');

  const titlebar = document.querySelector('.titlebar');
  const minimizeBtn = document.getElementById('minimize-btn');
  const maximizeBtn = document.getElementById('maximize-btn');
  const closeBtn = document.getElementById('close-btn');

  // Load and display machine model in titlebar
  loadMachineModel();

  if (!minimizeBtn || !maximizeBtn || !closeBtn) {
    console.error('[Titlebar] Buttons not found');
    return;
  }

  if (titlebar) {
    titlebar.addEventListener('mousedown', async (e) => {
      if (e.target.closest('.titlebar-button')) {
        return;
      }
      try {
        await invoke('start_drag');
      } catch (e) {
        console.error('[Titlebar] Drag failed:', e);
      }
    });

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
    try {
      await invoke('minimize_window');
    } catch (e) {
      console.error('[Titlebar] Minimize failed:', e);
    }
  });

  maximizeBtn.addEventListener('click', async () => {
    try {
      await invoke('toggle_maximize');
    } catch (e) {
      console.error('[Titlebar] Maximize failed:', e);
    }
  });

  closeBtn.addEventListener('click', async () => {
    try {
      await invoke('close_window');
    } catch (e) {
      console.error('[Titlebar] Close failed:', e);
    }
  });
}

async function loadMachineModel() {
  try {
    const response = await invoke('get_system_info');
    if (response.success && response.data) {
      const titleElement = document.querySelector('.titlebar-title');
      if (titleElement) {
        titleElement.textContent = `ThinkUtils - ${response.data.model}`;
      }
    }
  } catch (e) {
    console.error('[Titlebar] Failed to load machine model:', e);
  }
}
