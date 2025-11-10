use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurvePoint {
    pub temp: i32,
    pub level: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    // Fan Control
    pub fan_mode: String,
    pub fan_level: i32,
    pub fan_curve_enabled: bool,
    pub fan_curve_points: Vec<CurvePoint>,

    // Battery
    pub battery_start_threshold: i32,
    pub battery_stop_threshold: i32,

    // Performance
    pub cpu_governor: String,
    pub turbo_boost_enabled: bool,
    pub power_profile: String,

    // App Settings
    pub auto_start: bool,
    pub minimize_to_tray: bool,
    pub theme: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            // Fan Control defaults
            fan_mode: "auto".to_string(),
            fan_level: 0,
            fan_curve_enabled: false,
            fan_curve_points: vec![
                CurvePoint { temp: 40, level: 0 },
                CurvePoint { temp: 50, level: 1 },
                CurvePoint { temp: 60, level: 3 },
                CurvePoint { temp: 70, level: 5 },
                CurvePoint { temp: 80, level: 7 },
            ],

            // Battery defaults
            battery_start_threshold: 40,
            battery_stop_threshold: 80,

            // Performance defaults
            cpu_governor: "powersave".to_string(),
            turbo_boost_enabled: true,
            power_profile: "balanced".to_string(),

            // App defaults
            auto_start: false,
            minimize_to_tray: true,
            theme: "system".to_string(),
        }
    }
}

const STORE_FILE: &str = "settings.json";
const SETTINGS_KEY: &str = "app_settings";

/// Save all app settings to persistent storage
#[tauri::command]
pub async fn save_app_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("Failed to get store: {}", e))?;

    let settings_json = serde_json::to_value(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    store.set(SETTINGS_KEY, settings_json);
    store.save()
        .map_err(|e| format!("Failed to save store: {}", e))?;

    println!("[Settings] All settings saved to store");
    Ok(())
}

/// Load all app settings from persistent storage
#[tauri::command]
pub async fn load_app_settings(app: AppHandle) -> Result<AppSettings, String> {
    match app.store(STORE_FILE) {
        Ok(store) => {
            if let Some(settings_value) = store.get(SETTINGS_KEY) {
                match serde_json::from_value::<AppSettings>(settings_value.clone()) {
                    Ok(settings) => {
                        println!("[Settings] Settings loaded from store");
                        return Ok(settings);
                    }
                    Err(e) => {
                        eprintln!("[Settings] Failed to deserialize settings: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("[Settings] Failed to get store: {}", e);
        }
    }

    println!("[Settings] Using default settings");
    Ok(AppSettings::default())
}

/// Update specific setting field
#[tauri::command]
pub async fn update_setting(
    app: AppHandle,
    key: String,
    value: serde_json::Value,
) -> Result<(), String> {
    // Load current settings
    let mut settings = load_app_settings(app.clone()).await?;

    // Update the specific field
    match key.as_str() {
        "fan_mode" => {
            settings.fan_mode = value.as_str()
                .ok_or("Invalid fan_mode value")?
                .to_string();
        }
        "fan_level" => {
            settings.fan_level = value.as_i64()
                .ok_or("Invalid fan_level value")? as i32;
        }
        "fan_curve_enabled" => {
            settings.fan_curve_enabled = value.as_bool()
                .ok_or("Invalid fan_curve_enabled value")?;
        }
        "battery_start_threshold" => {
            settings.battery_start_threshold = value.as_i64()
                .ok_or("Invalid battery_start_threshold value")? as i32;
        }
        "battery_stop_threshold" => {
            settings.battery_stop_threshold = value.as_i64()
                .ok_or("Invalid battery_stop_threshold value")? as i32;
        }
        "cpu_governor" => {
            settings.cpu_governor = value.as_str()
                .ok_or("Invalid cpu_governor value")?
                .to_string();
        }
        "turbo_boost_enabled" => {
            settings.turbo_boost_enabled = value.as_bool()
                .ok_or("Invalid turbo_boost_enabled value")?;
        }
        "power_profile" => {
            settings.power_profile = value.as_str()
                .ok_or("Invalid power_profile value")?
                .to_string();
        }
        _ => return Err(format!("Unknown setting key: {}", key)),
    }

    // Save updated settings
    save_app_settings(app, settings).await
}

