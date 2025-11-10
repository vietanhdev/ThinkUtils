use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct CpuInfo {
    pub governor: String,
    pub min_freq: u32,
    pub max_freq: u32,
    pub current_freq: u32,
    pub available_governors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PowerProfile {
    pub current: String,
    pub available: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[tauri::command]
pub fn get_cpu_info() -> ApiResponse<CpuInfo> {
    let cpu0_path = "/sys/devices/system/cpu/cpu0/cpufreq";

    let read_file = |file: &str| -> String {
        fs::read_to_string(format!("{}/{}", cpu0_path, file))
            .unwrap_or_default()
            .trim()
            .to_string()
    };

    let read_freq = |file: &str| -> u32 {
        read_file(file)
            .parse::<u32>()
            .unwrap_or(0) / 1000 // Convert kHz to MHz
    };

    let governor = read_file("scaling_governor");
    let available_governors = read_file("scaling_available_governors")
        .split_whitespace()
        .map(String::from)
        .collect();

    ApiResponse {
        success: true,
        data: Some(CpuInfo {
            governor,
            min_freq: read_freq("scaling_min_freq"),
            max_freq: read_freq("scaling_max_freq"),
            current_freq: read_freq("scaling_cur_freq"),
            available_governors,
        }),
        error: None,
    }
}

#[tauri::command]
pub async fn set_cpu_governor(governor: String) -> ApiResponse<String> {
    println!("[Performance] Setting CPU governor to: {}", governor);

    // Get number of CPUs
    let cpu_count = fs::read_dir("/sys/devices/system/cpu")
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with("cpu")
                        && e.file_name().to_string_lossy()[3..].chars().all(|c| c.is_numeric())
                })
                .count()
        })
        .unwrap_or(1);

    println!("[Performance] Found {} CPU cores", cpu_count);

    // Create script to set governor for all CPUs
    let temp_script = format!("/tmp/set_governor_{}.sh", std::process::id());
    let mut script_content = String::from("#!/bin/bash\nset -e\n");

    for i in 0..cpu_count {
        script_content.push_str(&format!(
            "echo {} > /sys/devices/system/cpu/cpu{}/cpufreq/scaling_governor\n",
            governor, i
        ));
    }
    script_content.push_str("exit 0\n");

    println!("[Performance] Script content:\n{}", script_content);

    if let Err(e) = fs::write(&temp_script, &script_content) {
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

    println!("[Performance] Executing pkexec...");

    match tokio::process::Command::new("pkexec")
        .arg("bash")
        .arg(&temp_script)
        .output()
        .await
    {
        Ok(output) => {
            let _ = fs::remove_file(&temp_script);

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            println!("[Performance] pkexec stdout: {}", stdout);
            println!("[Performance] pkexec stderr: {}", stderr);
            println!("[Performance] pkexec status: {}", output.status);

            if output.status.success() {
                println!("[Performance] Successfully set governor to: {}", governor);
                ApiResponse {
                    success: true,
                    data: Some(format!("CPU governor set to: {}", governor)),
                    error: None,
                }
            } else {
                let error_msg = if !stderr.is_empty() {
                    stderr.to_string()
                } else {
                    "Permission denied or operation failed".to_string()
                };
                println!("[Performance] Failed to set governor: {}", error_msg);
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(error_msg),
                }
            }
        }
        Err(e) => {
            let _ = fs::remove_file(&temp_script);
            println!("[Performance] Failed to execute pkexec: {}", e);
            ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to execute pkexec: {}", e)),
            }
        }
    }
}

#[tauri::command]
pub fn get_power_profile() -> ApiResponse<PowerProfile> {
    // Try power-profiles-daemon first
    match Command::new("powerprofilesctl").arg("get").output() {
        Ok(output) if output.status.success() => {
            let current = String::from_utf8_lossy(&output.stdout).trim().to_string();

            // Get available profiles
            let available = match Command::new("powerprofilesctl").arg("list").output() {
                Ok(list_output) if list_output.status.success() => {
                    String::from_utf8_lossy(&list_output.stdout)
                        .lines()
                        .filter(|line| line.contains("*") || line.trim().starts_with("power-saver")
                                    || line.trim().starts_with("balanced")
                                    || line.trim().starts_with("performance"))
                        .map(|line| {
                            line.trim()
                                .trim_start_matches("* ")
                                .split(':')
                                .next()
                                .unwrap_or("")
                                .trim()
                                .to_string()
                        })
                        .filter(|s| !s.is_empty())
                        .collect()
                }
                _ => vec!["power-saver".to_string(), "balanced".to_string(), "performance".to_string()],
            };

            return ApiResponse {
                success: true,
                data: Some(PowerProfile { current, available }),
                error: None,
            };
        }
        _ => {}
    }

    // Fallback to TLP if available
    match Command::new("tlp-stat").arg("-s").output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let current = if stdout.contains("AC") {
                "performance".to_string()
            } else {
                "power-saver".to_string()
            };

            return ApiResponse {
                success: true,
                data: Some(PowerProfile {
                    current,
                    available: vec!["power-saver".to_string(), "performance".to_string()],
                }),
                error: None,
            };
        }
        _ => {}
    }

    ApiResponse {
        success: false,
        data: None,
        error: Some("No power management tool found (install power-profiles-daemon or TLP)".to_string()),
    }
}

#[tauri::command]
pub async fn set_power_profile(profile: String) -> ApiResponse<String> {
    println!("[Performance] Setting power profile to: {}", profile);

    // Try power-profiles-daemon
    match tokio::process::Command::new("powerprofilesctl")
        .arg("set")
        .arg(&profile)
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            return ApiResponse {
                success: true,
                data: Some(format!("Power profile set to: {}", profile)),
                error: None,
            };
        }
        _ => {}
    }

    // Fallback to TLP
    let tlp_mode = match profile.as_str() {
        "power-saver" => "BAT",
        "performance" => "AC",
        _ => "BAT",
    };

    match tokio::process::Command::new("sudo")
        .arg("tlp")
        .arg(tlp_mode)
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            ApiResponse {
                success: true,
                data: Some(format!("TLP mode set to: {}", tlp_mode)),
                error: None,
            }
        }
        _ => {
            ApiResponse {
                success: false,
                data: None,
                error: Some("Failed to set power profile".to_string()),
            }
        }
    }
}

#[tauri::command]
pub fn get_turbo_boost_status() -> ApiResponse<bool> {
    let intel_pstate = "/sys/devices/system/cpu/intel_pstate/no_turbo";
    let cpufreq_boost = "/sys/devices/system/cpu/cpufreq/boost";

    // Check Intel P-state
    if let Ok(content) = fs::read_to_string(intel_pstate) {
        let no_turbo = content.trim() == "1";
        return ApiResponse {
            success: true,
            data: Some(!no_turbo), // Invert because file is "no_turbo"
            error: None,
        };
    }

    // Check cpufreq boost
    if let Ok(content) = fs::read_to_string(cpufreq_boost) {
        let boost = content.trim() == "1";
        return ApiResponse {
            success: true,
            data: Some(boost),
            error: None,
        };
    }

    ApiResponse {
        success: false,
        data: None,
        error: Some("Turbo boost control not available".to_string()),
    }
}

#[tauri::command]
pub async fn set_turbo_boost(enabled: bool) -> ApiResponse<String> {
    let intel_pstate = "/sys/devices/system/cpu/intel_pstate/no_turbo";
    let cpufreq_boost = "/sys/devices/system/cpu/cpufreq/boost";

    let value = if enabled { "0" } else { "1" }; // Inverted for no_turbo
    let boost_value = if enabled { "1" } else { "0" };

    // Try Intel P-state first
    if std::path::Path::new(intel_pstate).exists() {
        let temp_script = format!("/tmp/set_turbo_{}.sh", std::process::id());
        let script_content = format!("#!/bin/bash\nset -e\necho {} > {}\nexit 0\n", value, intel_pstate);

        if fs::write(&temp_script, script_content).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o755);
                let _ = fs::set_permissions(&temp_script, perms);
            }

            if let Ok(output) = tokio::process::Command::new("pkexec")
                .arg("bash")
                .arg(&temp_script)
                .output()
                .await
            {
                let _ = fs::remove_file(&temp_script);

                if output.status.success() {
                    return ApiResponse {
                        success: true,
                        data: Some(format!("Turbo boost {}", if enabled { "enabled" } else { "disabled" })),
                        error: None,
                    };
                }
            }
        }
    }

    // Try cpufreq boost
    if std::path::Path::new(cpufreq_boost).exists() {
        let temp_script = format!("/tmp/set_boost_{}.sh", std::process::id());
        let script_content = format!("#!/bin/bash\nset -e\necho {} > {}\nexit 0\n", boost_value, cpufreq_boost);

        if fs::write(&temp_script, script_content).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o755);
                let _ = fs::set_permissions(&temp_script, perms);
            }

            if let Ok(output) = tokio::process::Command::new("pkexec")
                .arg("bash")
                .arg(&temp_script)
                .output()
                .await
            {
                let _ = fs::remove_file(&temp_script);

                if output.status.success() {
                    return ApiResponse {
                        success: true,
                        data: Some(format!("Turbo boost {}", if enabled { "enabled" } else { "disabled" })),
                        error: None,
                    };
                }
            }
        }
    }

    ApiResponse {
        success: false,
        data: None,
        error: Some("Failed to set turbo boost".to_string()),
    }
}
