// Settings Manager - Centralized settings loading and saving
const { invoke } = window.__TAURI__.core;

let currentSettings = null;

/**
 * Load all settings from backend on app startup
 */
export async function loadAllSettings() {
  try {
    console.log('[Settings] Loading all settings...');
    currentSettings = await invoke('load_app_settings');
    console.log('[Settings] Loaded:', currentSettings);
    return currentSettings;
  } catch (error) {
    console.error('[Settings] Failed to load settings:', error);
    return null;
  }
}

/**
 * Save all settings to backend
 */
export async function saveAllSettings(settings) {
  try {
    await invoke('save_app_settings', { settings });
    currentSettings = settings;
    console.log('[Settings] All settings saved');
    return true;
  } catch (error) {
    console.error('[Settings] Failed to save settings:', error);
    return false;
  }
}

/**
 * Update a specific setting
 */
export async function updateSetting(key, value) {
  try {
    await invoke('update_setting', { key, value });
    if (currentSettings) {
      currentSettings[key] = value;
    }
    console.log(`[Settings] Updated ${key}:`, value);
    return true;
  } catch (error) {
    console.error(`[Settings] Failed to update ${key}:`, error);
    return false;
  }
}

/**
 * Get current settings (from cache)
 */
export function getCurrentSettings() {
  return currentSettings;
}

/**
 * Apply fan control settings (only if permissions are available)
 */
export async function applyFanSettings(settings) {
  // Check if we have permissions before trying to set fan speed,
  // otherwise this would trigger a pkexec password prompt on startup.
  try {
    const perms = await invoke('check_permissions');
    if (!perms.success || !perms.data) {
      console.log('[Settings] Skipping fan settings - no permissions yet');
      return;
    }
  } catch {
    return;
  }

  const { setFanMode } = await import('./views/fan.js');

  if (settings.fan_curve_enabled) {
    await setFanMode('curve');
  } else if (settings.fan_mode) {
    await setFanMode(settings.fan_mode, settings.fan_level);
  }
}

/**
 * Apply battery settings
 */
export async function applyBatterySettings(settings) {
  const elements = (await import('./dom.js')).elements;

  if (elements.thresholdStart) {
    elements.thresholdStart.value = settings.battery_start_threshold;
    if (elements.thresholdStartValue) {
      elements.thresholdStartValue.textContent = settings.battery_start_threshold + '%';
    }
  }

  if (elements.thresholdStop) {
    elements.thresholdStop.value = settings.battery_stop_threshold;
    if (elements.thresholdStopValue) {
      elements.thresholdStopValue.textContent = settings.battery_stop_threshold + '%';
    }
  }
}

/**
 * Apply performance settings
 */
export async function applyPerformanceSettings(settings) {
  // CPU Governor
  if (settings.cpu_governor) {
    try {
      await invoke('set_cpu_governor', { governor: settings.cpu_governor });
    } catch (error) {
      console.error('[Settings] Failed to apply CPU governor:', error);
    }
  }

  // Turbo Boost
  if (settings.turbo_boost_enabled !== undefined) {
    try {
      await invoke('set_turbo_boost', { enabled: settings.turbo_boost_enabled });
    } catch (error) {
      console.error('[Settings] Failed to apply turbo boost:', error);
    }
  }

  // Power Profile
  if (settings.power_profile) {
    try {
      await invoke('set_power_profile', { profile: settings.power_profile });
    } catch (error) {
      console.error('[Settings] Failed to apply power profile:', error);
    }
  }
}

/**
 * Apply all settings to the system
 */
export async function applyAllSettings(settings) {
  if (!settings) {
    console.warn('[Settings] No settings to apply');
    return;
  }

  console.log('[Settings] Applying all settings...');

  // Only apply UI-level and fan settings on startup.
  // Performance settings (CPU governor, turbo boost, power profile) are NOT
  // auto-applied because they use pkexec which would prompt for a password
  // every time the app starts. Users apply these explicitly from the UI.
  await applyBatterySettings(settings);
  await applyFanSettings(settings);

  console.log('[Settings] All settings applied');
}

/**
 * Initialize settings on app startup
 */
export async function initializeSettings() {
  const settings = await loadAllSettings();
  if (settings) {
    await applyAllSettings(settings);
  }
  return settings;
}
