use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurvePoint {
    pub temp: i32,
    pub level: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanCurveConfig {
    pub enabled: bool,
    pub points: Vec<CurvePoint>,
}

impl Default for FanCurveConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            points: vec![
                CurvePoint { temp: 40, level: 0 },
                CurvePoint { temp: 50, level: 1 },
                CurvePoint { temp: 60, level: 3 },
                CurvePoint { temp: 70, level: 5 },
                CurvePoint { temp: 80, level: 7 },
            ],
        }
    }
}

pub type FanCurveState = Arc<Mutex<FanCurveConfig>>;

const STORE_FILE: &str = "settings.json";
const CURVE_KEY: &str = "fan_curve";

/// Save fan curve config to persistent storage
fn save_config_to_store(app: &AppHandle, config: &FanCurveConfig) -> Result<(), String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("Failed to get store: {}", e))?;

    let config_json = serde_json::to_value(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    store.set(CURVE_KEY, config_json);
    store.save()
        .map_err(|e| format!("Failed to save store: {}", e))?;

    println!("[Fan Curve] Configuration saved to store");
    Ok(())
}

/// Load fan curve config from persistent storage
pub fn load_config_from_store(app: &AppHandle) -> FanCurveConfig {
    match app.store(STORE_FILE) {
        Ok(store) => {
            if let Some(config_value) = store.get(CURVE_KEY) {
                match serde_json::from_value::<FanCurveConfig>(config_value.clone()) {
                    Ok(config) => {
                        println!("[Fan Curve] Configuration loaded from store");
                        return config;
                    }
                    Err(e) => {
                        eprintln!("[Fan Curve] Failed to deserialize config: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("[Fan Curve] Failed to get store: {}", e);
        }
    }

    println!("[Fan Curve] Using default configuration");
    FanCurveConfig::default()
}

#[tauri::command]
pub async fn set_fan_curve(
    app: AppHandle,
    state: tauri::State<'_, FanCurveState>,
    points: Vec<CurvePoint>,
) -> Result<(), String> {
    let mut config = state.lock().map_err(|e| format!("Failed to lock state: {}", e))?;
    config.points = points.clone();

    // Save to persistent storage
    save_config_to_store(&app, &config)?;

    Ok(())
}

#[tauri::command]
pub async fn get_fan_curve(
    state: tauri::State<'_, FanCurveState>,
) -> Result<FanCurveConfig, String> {
    let config = state.lock().map_err(|e| format!("Failed to lock state: {}", e))?;
    Ok(config.clone())
}

#[tauri::command]
pub async fn enable_fan_curve(
    app: AppHandle,
    state: tauri::State<'_, FanCurveState>,
    enabled: bool,
) -> Result<(), String> {
    let mut config = state.lock().map_err(|e| format!("Failed to lock state: {}", e))?;
    config.enabled = enabled;

    // Save to persistent storage
    save_config_to_store(&app, &config)?;

    Ok(())
}

/// Calculate fan level based on temperature and curve points
fn calculate_fan_level(temp: i32, points: &[CurvePoint]) -> i32 {
    if points.is_empty() {
        return 0;
    }

    // Sort points by temperature
    let mut sorted_points = points.to_vec();
    sorted_points.sort_by_key(|p| p.temp);

    // If temp is below first point, use first level
    if temp <= sorted_points[0].temp {
        return sorted_points[0].level;
    }

    // If temp is above last point, use last level
    if temp >= sorted_points[sorted_points.len() - 1].temp {
        return sorted_points[sorted_points.len() - 1].level;
    }

    // Find the two points to interpolate between
    for i in 0..sorted_points.len() - 1 {
        if temp >= sorted_points[i].temp && temp <= sorted_points[i + 1].temp {
            // Linear interpolation
            let t = (temp - sorted_points[i].temp) as f64
                / (sorted_points[i + 1].temp - sorted_points[i].temp) as f64;
            let level = sorted_points[i].level as f64
                + t * (sorted_points[i + 1].level - sorted_points[i].level) as f64;
            return level.round() as i32;
        }
    }

    sorted_points[sorted_points.len() - 1].level
}

/// Get CPU temperature from sensors
fn get_cpu_temperature() -> Result<i32, String> {
    use std::fs;
    use std::path::Path;

    // Try to read from common thermal zones
    let thermal_zones = [
        "/sys/class/thermal/thermal_zone0/temp",
        "/sys/class/thermal/thermal_zone1/temp",
        "/sys/class/thermal/thermal_zone2/temp",
    ];

    for zone in &thermal_zones {
        if Path::new(zone).exists() {
            if let Ok(content) = fs::read_to_string(zone) {
                if let Ok(temp_millidegrees) = content.trim().parse::<i32>() {
                    let temp = temp_millidegrees / 1000;
                    // Sanity check: temperature should be between 0 and 120°C
                    if temp > 0 && temp < 120 {
                        return Ok(temp);
                    }
                }
            }
        }
    }

    // Try hwmon sensors
    let hwmon_path = "/sys/class/hwmon";
    if let Ok(entries) = fs::read_dir(hwmon_path) {
        for entry in entries.flatten() {
            let path = entry.path();

            // Look for CPU or package temperature
            for i in 1..10 {
                let temp_label_path = path.join(format!("temp{}_label", i));
                let temp_input_path = path.join(format!("temp{}_input", i));

                if temp_label_path.exists() && temp_input_path.exists() {
                    if let Ok(label) = fs::read_to_string(&temp_label_path) {
                        let label_lower = label.to_lowercase();
                        if label_lower.contains("cpu") || label_lower.contains("package") || label_lower.contains("core") {
                            if let Ok(content) = fs::read_to_string(&temp_input_path) {
                                if let Ok(temp_millidegrees) = content.trim().parse::<i32>() {
                                    let temp = temp_millidegrees / 1000;
                                    if temp > 0 && temp < 120 {
                                        return Ok(temp);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Err("Could not read CPU temperature".to_string())
}

/// Set fan speed using the fan_control module
fn set_fan_speed_internal(level: i32) -> Result<(), String> {
    use std::fs;
    use std::io::Write;

    let fan_level_path = "/proc/acpi/ibm/fan";

    let command = if level >= 0 && level <= 7 {
        format!("level {}", level)
    } else {
        return Err("Invalid fan level".to_string());
    };

    let mut file = fs::OpenOptions::new()
        .write(true)
        .open(fan_level_path)
        .map_err(|e| format!("Failed to open fan control: {}", e))?;

    file.write_all(command.as_bytes())
        .map_err(|e| format!("Failed to write fan level: {}", e))?;

    Ok(())
}

/// Background task that monitors temperature and adjusts fan speed
pub async fn fan_curve_background_task(app: AppHandle) {
    let state = app.state::<FanCurveState>();
    let mut last_level: Option<i32> = None;
    let mut error_count = 0;
    const MAX_ERRORS: i32 = 5;

    loop {
        // Sleep for 2 seconds between checks
        sleep(Duration::from_secs(2)).await;

        // Check if curve mode is enabled
        let config = match state.lock() {
            Ok(cfg) => cfg.clone(),
            Err(e) => {
                eprintln!("[Fan Curve] Failed to lock state: {}", e);
                continue;
            }
        };

        if !config.enabled {
            last_level = None;
            continue;
        }

        // Get current CPU temperature
        let temp = match get_cpu_temperature() {
            Ok(t) => t,
            Err(e) => {
                error_count += 1;
                if error_count <= MAX_ERRORS {
                    eprintln!("[Fan Curve] Failed to read temperature: {}", e);
                }
                continue;
            }
        };

        error_count = 0; // Reset error count on success

        // Calculate target fan level
        let target_level = calculate_fan_level(temp, &config.points);

        // Only update if level changed (avoid unnecessary writes)
        if last_level != Some(target_level) {
            match set_fan_speed_internal(target_level) {
                Ok(_) => {
                    println!("[Fan Curve] Temp: {}°C -> Fan Level: {}", temp, target_level);
                    last_level = Some(target_level);

                    // Emit event to frontend for UI update
                    if let Err(e) = app.emit_to("main", "fan-curve-update", serde_json::json!({
                        "temperature": temp,
                        "fan_level": target_level,
                    })) {
                        eprintln!("[Fan Curve] Failed to emit event: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("[Fan Curve] Failed to set fan speed: {}", e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_fan_level() {
        let points = vec![
            CurvePoint { temp: 40, level: 0 },
            CurvePoint { temp: 50, level: 1 },
            CurvePoint { temp: 60, level: 3 },
            CurvePoint { temp: 70, level: 5 },
            CurvePoint { temp: 80, level: 7 },
        ];

        // Below first point
        assert_eq!(calculate_fan_level(35, &points), 0);

        // At points
        assert_eq!(calculate_fan_level(40, &points), 0);
        assert_eq!(calculate_fan_level(50, &points), 1);
        assert_eq!(calculate_fan_level(60, &points), 3);

        // Between points (interpolation)
        assert_eq!(calculate_fan_level(45, &points), 1); // Rounded from 0.5
        assert_eq!(calculate_fan_level(55, &points), 2); // Rounded from 2.0

        // Above last point
        assert_eq!(calculate_fan_level(85, &points), 7);
    }
}
