use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use regex::Regex;

const PROC_FAN: &str = "/proc/acpi/ibm/fan";

#[derive(Debug, Serialize, Deserialize)]
pub struct SensorData {
    pub temps: HashMap<String, String>,
    pub fans: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[tauri::command]
pub fn get_sensor_data() -> ApiResponse<SensorData> {
    let mut temps = HashMap::new();
    let mut fans = HashMap::new();

    // Get fan info from /proc/acpi/ibm/fan
    match fs::read_to_string(PROC_FAN) {
        Ok(content) => {
            for line in content.lines() {
                if let Some((key, value)) = line.split_once(':') {
                    let key = key.trim();
                    let value = value.trim();

                    match key {
                        "status" => {
                            fans.insert("status".to_string(), value.to_string());
                        }
                        "level" => {
                            fans.insert("level".to_string(), value.to_string());
                        }
                        "speed" => {
                            fans.insert("Fan1".to_string(), format!("{} RPM", value));
                        }
                        _ => {}
                    }
                }
            }
        }
        Err(e) => {
            return ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to read {}: {}", PROC_FAN, e)),
            };
        }
    }

    // Get temperature data from sensors command
    match Command::new("sensors").arg("thinkpad-isa-0000").output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let temp_re = Regex::new(r"^(.+?):\s+\+?([0-9.]+°C)").unwrap();

                for line in stdout.lines() {
                    if let Some(caps) = temp_re.captures(line) {
                        let label = caps.get(1).unwrap().as_str().trim();
                        let value = caps.get(2).unwrap().as_str();
                        let label_lower = label.to_lowercase();
                        if label_lower.contains("cpu") || label_lower.contains("gpu") {
                            temps.insert(label.to_string(), value.to_string());
                        }
                    }

                    if line.starts_with("fan") && line.contains("RPM") {
                        if let Some((label, rest)) = line.split_once(':') {
                            let label = label.trim();
                            if let Some(rpm_match) = Regex::new(r"(\d+\s*RPM)").unwrap().find(rest) {
                                fans.insert(label.to_string(), rpm_match.as_str().to_string());
                            }
                        }
                    }
                }
            } else {
                temps.insert("Info".to_string(), "Install lm-sensors for temperature data".to_string());
            }
        }
        Err(_) => {
            temps.insert("Info".to_string(), "Install lm-sensors: sudo apt install lm-sensors".to_string());
        }
    }

    ApiResponse {
        success: true,
        data: Some(SensorData { temps, fans }),
        error: None,
    }
}

#[tauri::command]
pub async fn set_fan_speed(speed: String) -> ApiResponse<String> {
    println!("[Fan] Setting speed to: {}", speed);
    let command_str = format!("level {}", speed);

    match fs::write(PROC_FAN, &command_str) {
        Ok(_) => {
            println!("[Fan] ✓ Speed set successfully");
            return ApiResponse {
                success: true,
                data: Some(format!("Fan speed set to: {}", speed)),
                error: None,
            };
        }
        Err(_) => {
            println!("[Fan] Need elevated permissions");

            let temp_script = format!("/tmp/thinkfan_set_speed_{}.sh", std::process::id());
            let script_content = format!(
                "#!/bin/bash\nset -e\necho '{}' > {}\nexit 0\n",
                command_str, PROC_FAN
            );

            if let Err(e) = fs::write(&temp_script, script_content) {
                return ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Failed to create temp script: {}", e)),
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
                        println!("[Fan] ✓ Speed set via pkexec");
                        return ApiResponse {
                            success: true,
                            data: Some(format!("Fan speed set to: {}", speed)),
                            error: None,
                        };
                    } else {
                        return ApiResponse {
                            success: false,
                            data: None,
                            error: Some("Permission denied. Click 'Grant Permissions' to avoid repeated password prompts.".to_string()),
                        };
                    }
                }
                Err(e) => {
                    let _ = fs::remove_file(&temp_script);
                    return ApiResponse {
                        success: false,
                        data: None,
                        error: Some(format!("Failed to execute pkexec: {}", e)),
                    };
                }
            }
        }
    }
}

#[tauri::command]
pub fn check_permissions() -> ApiResponse<bool> {
    let has_permission = fs::metadata(PROC_FAN)
        .and_then(|_| fs::OpenOptions::new().write(true).open(PROC_FAN))
        .is_ok();

    ApiResponse {
        success: true,
        data: Some(has_permission),
        error: None,
    }
}

#[tauri::command]
pub async fn update_permissions() -> ApiResponse<String> {
    println!("[Permissions] Updating permissions for {}", PROC_FAN);

    let username = std::env::var("USER").unwrap_or_else(|_| "root".to_string());

    let result = tokio::process::Command::new("pkexec")
        .arg("chown")
        .arg(&username)
        .arg(PROC_FAN)
        .output()
        .await;

    match result {
        Ok(output) => {
            if output.status.success() {
                ApiResponse {
                    success: true,
                    data: Some("Permissions updated successfully".to_string()),
                    error: None,
                }
            } else {
                let chmod_result = tokio::process::Command::new("pkexec")
                    .arg("chmod")
                    .arg("666")
                    .arg(PROC_FAN)
                    .output()
                    .await;

                match chmod_result {
                    Ok(chmod_output) if chmod_output.status.success() => {
                        ApiResponse {
                            success: true,
                            data: Some("Permissions updated via chmod".to_string()),
                            error: None,
                        }
                    }
                    _ => {
                        ApiResponse {
                            success: false,
                            data: None,
                            error: Some("Failed to update permissions".to_string()),
                        }
                    }
                }
            }
        }
        Err(e) => ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to run pkexec: {}", e)),
        },
    }
}
