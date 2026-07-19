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

/// Whether the new start threshold must be written before the new stop threshold.
///
/// The firmware rejects any write that would leave start >= stop, even
/// transiently, so the order depends on where the new stop lands relative to the
/// start already on the hardware.
///
/// Writing stop first leaves the intermediate state (current_start, new_stop).
/// That is only safe while current_start < new_stop. The boundary case matters:
/// at new_stop == current_start the intermediate state is (X, X), which is
/// start == stop and is rejected — hence >=, not >.
fn write_start_first(current_start: u8, new_stop: u8) -> bool {
    new_stop <= current_start
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

    // get_battery_thresholds() has no failure path — it substitutes defaults on a
    // failed read — so there is nothing to match on. Note the substituted default
    // for start is 0, which makes write_start_first() return false and yields the
    // stop-first order. That is the safe fallback: it is the order that was in use
    // before this ordering logic existed.
    let current = get_battery_thresholds().data.unwrap_or(BatteryThresholds {
        start: 0,
        stop: 100,
    });

    let (first_path, first_value, second_path, second_value) =
        if write_start_first(current.start, stop) {
            (start_path, start.to_string(), stop_path, stop.to_string())
        } else {
            (stop_path, stop.to_string(), start_path, start.to_string())
        };

    // Try direct write first
    if fs::write(&first_path, &first_value).is_ok()
        && fs::write(&second_path, &second_value).is_ok()
    {
        return ApiResponse {
            success: true,
            data: Some(format!("Thresholds set: {}%-{}%", start, stop)),
            error: None,
        };
    }

    // Need elevated permissions. Writes stay in the order chosen above.
    let temp_script = format!("/tmp/battery_thresholds_{}.sh", std::process::id());
    let script_content = format!(
        "#!/bin/bash\nset -e\necho {} > {}\necho {} > {}\nexit 0\n",
        first_value, first_path, second_value, second_path
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Model the firmware constraint: a write is rejected if it would leave
    /// start >= stop, even for an instant. Replays the two writes in the chosen
    /// order and reports whether every intermediate state stayed legal.
    fn ordering_is_safe(current: (u8, u8), target: (u8, u8)) -> bool {
        let (cur_start, cur_stop) = current;
        let (new_start, new_stop) = target;

        let states = if write_start_first(cur_start, new_stop) {
            // write start, then stop
            [(new_start, cur_stop), (new_start, new_stop)]
        } else {
            // write stop, then start
            [(cur_start, new_stop), (new_start, new_stop)]
        };

        states.iter().all(|(s, e)| s < e)
    }

    /// The case the original ordering logic missed. With `new_stop < current_start`
    /// this picks stop-first, producing the intermediate state (60, 60) — start
    /// equal to stop, which the firmware rejects with the same "Invalid argument"
    /// the ordering was introduced to prevent.
    #[test]
    fn boundary_new_stop_equals_current_start() {
        assert!(
            write_start_first(60, 60),
            "new_stop == current_start must write start first"
        );
        assert!(ordering_is_safe((60, 80), (40, 60)));
    }

    #[test]
    fn lowering_thresholds_writes_start_first() {
        assert!(write_start_first(60, 50));
        assert!(ordering_is_safe((60, 80), (30, 50)));
    }

    #[test]
    fn raising_thresholds_writes_stop_first() {
        assert!(!write_start_first(40, 90));
        assert!(ordering_is_safe((40, 60), (70, 90)));
    }

    /// Exhaustive sweep over every legal current/target pair. Any ordering rule
    /// that admits an illegal intermediate state fails here.
    #[test]
    fn no_transition_passes_through_an_illegal_state() {
        for cur_start in 0..=100u8 {
            for cur_stop in (cur_start + 1)..=100u8 {
                for new_start in 0..=100u8 {
                    for new_stop in (new_start + 1)..=100u8 {
                        assert!(
                            ordering_is_safe((cur_start, cur_stop), (new_start, new_stop)),
                            "illegal intermediate state going from ({}, {}) to ({}, {})",
                            cur_start,
                            cur_stop,
                            new_start,
                            new_stop
                        );
                    }
                }
            }
        }
    }

    /// A failed sysfs read substitutes start = 0, which must fall back to the
    /// stop-first order that predates this logic rather than doing something novel.
    #[test]
    fn unreadable_current_start_falls_back_to_stop_first() {
        assert!(!write_start_first(0, 80));
    }
}
