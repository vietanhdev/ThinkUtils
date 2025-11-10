use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const BAT0_PATH: &str = "/sys/class/power_supply/BAT0";
const BAT1_PATH: &str = "/sys/class/power_supply/BAT1";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BatteryInfo {
    pub name: String,
    pub status: String,
    pub capacity: u8,
    pub health: u8,
    pub cycles: u32,
    pub voltage: f32,
    pub current: f32,
    pub power: f32,
    pub energy_now: f32,
    pub energy_full: f32,
    pub energy_design: f32,
    pub technology: String,
    pub manufacturer: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatteryThresholds {
    pub start: u8,
    pub stop: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[tauri::command]
pub fn get_battery_info() -> ApiResponse<Vec<BatteryInfo>> {
    let mut batteries = Vec::new();

    for (index, bat_path) in [BAT0_PATH, BAT1_PATH].iter().enumerate() {
        if Path::new(bat_path).exists() {
            match read_battery_info(bat_path, index) {
                Ok(info) => batteries.push(info),
                Err(e) => println!("[Battery] Error reading {}: {}", bat_path, e),
            }
        }
    }

    if batteries.is_empty() {
        return ApiResponse {
            success: false,
            data: None,
            error: Some("No batteries found".to_string()),
        };
    }

    ApiResponse {
        success: true,
        data: Some(batteries),
        error: None,
    }
}

fn read_battery_info(path: &str, index: usize) -> Result<BatteryInfo, String> {
    let read_file = |file: &str| -> Result<String, String> {
        fs::read_to_string(format!("{}/{}", path, file))
            .map(|s| s.trim().to_string())
            .map_err(|e| e.to_string())
    };

    let read_u32 = |file: &str| -> u32 {
        read_file(file)
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    };

    let read_f32 = |file: &str| -> f32 {
        read_file(file)
            .ok()
            .and_then(|s| s.parse::<f32>().ok())
            .map(|v| v / 1_000_000.0) // Convert from µV/µA to V/A
            .unwrap_or(0.0)
    };

    let capacity = read_u32("capacity") as u8;
    let energy_now = read_f32("energy_now");
    let energy_full = read_f32("energy_full");
    let energy_design = read_f32("energy_full_design");

    let health = if energy_design > 0.0 {
        ((energy_full / energy_design) * 100.0) as u8
    } else {
        100
    };

    let voltage = read_f32("voltage_now");
    let current = read_f32("current_now");
    let power = voltage * current;

    Ok(BatteryInfo {
        name: format!("BAT{}", index),
        status: read_file("status").unwrap_or_else(|_| "Unknown".to_string()),
        capacity,
        health,
        cycles: read_u32("cycle_count"),
        voltage,
        current,
        power,
        energy_now,
        energy_full,
        energy_design,
        technology: read_file("technology").unwrap_or_else(|_| "Unknown".to_string()),
        manufacturer: read_file("manufacturer").unwrap_or_else(|_| "Unknown".to_string()),
    })
}

#[tauri::command]
pub fn get_battery_thresholds() -> ApiResponse<BatteryThresholds> {
    let start_path = format!("{}/charge_control_start_threshold", BAT0_PATH);
    let stop_path = format!("{}/charge_control_end_threshold", BAT0_PATH);

    let start = fs::read_to_string(&start_path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    let stop = fs::read_to_string(&stop_path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(100);

    ApiResponse {
        success: true,
        data: Some(BatteryThresholds { start, stop }),
        error: None,
    }
}

#[tauri::command]
pub async fn set_battery_thresholds(start: u8, stop: u8) -> ApiResponse<String> {
    if start >= stop {
        return ApiResponse {
            success: false,
            data: None,
            error: Some("Start threshold must be less than stop threshold".to_string()),
        };
    }

    if start > 100 || stop > 100 {
        return ApiResponse {
            success: false,
            data: None,
            error: Some("Thresholds must be between 0 and 100".to_string()),
        };
    }

    let start_path = format!("{}/charge_control_start_threshold", BAT0_PATH);
    let stop_path = format!("{}/charge_control_end_threshold", BAT0_PATH);

    // Try direct write first
    if fs::write(&start_path, start.to_string()).is_ok()
        && fs::write(&stop_path, stop.to_string()).is_ok() {
        return ApiResponse {
            success: true,
            data: Some(format!("Thresholds set: {}%-{}%", start, stop)),
            error: None,
        };
    }

    // Need elevated permissions
    let temp_script = format!("/tmp/battery_thresholds_{}.sh", std::process::id());
    let script_content = format!(
        "#!/bin/bash\nset -e\necho {} > {}\necho {} > {}\nexit 0\n",
        start, start_path, stop, stop_path
    );

    if let Err(e) = fs::write(&temp_script, script_content) {
        return ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to create script: {}", e)),
        };
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        let _ = fs::set_permissions(&temp_script, perms);
    }

    match tokio::process::Command::new("pkexec")
        .arg("bash")
        .arg(&temp_script)
        .output()
        .await
    {
        Ok(output) => {
            let _ = fs::remove_file(&temp_script);

            if output.status.success() {
                ApiResponse {
                    success: true,
                    data: Some(format!("Thresholds set: {}%-{}%", start, stop)),
                    error: None,
                }
            } else {
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some("Permission denied".to_string()),
                }
            }
        }
        Err(e) => {
            let _ = fs::remove_file(&temp_script);
            ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to execute: {}", e)),
            }
        }
    }
}

#[tauri::command]
pub fn get_power_consumption() -> ApiResponse<f32> {
    let power_path = format!("{}/power_now", BAT0_PATH);

    let power = fs::read_to_string(&power_path)
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .map(|w| w / 1_000_000.0) // Convert µW to W
        .unwrap_or(0.0);

    ApiResponse {
        success: true,
        data: Some(power),
        error: None,
    }
}
