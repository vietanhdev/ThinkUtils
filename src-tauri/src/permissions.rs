use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::fan_control::{HELPER_PATH, HELPER_SCRIPT, POLKIT_RULE, POLKIT_RULE_PATH};

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PermissionStatus {
    pub has_permissions: bool,
    pub missing_files: Vec<String>,
}

// Files that need write permissions
const REQUIRED_FILES: &[&str] = &[
    "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor",
    "/sys/devices/system/cpu/intel_pstate/no_turbo",
    "/sys/devices/platform/thinkpad_hwmon/pwm1",
    "/sys/class/power_supply/BAT0/charge_start_threshold",
    "/sys/class/power_supply/BAT0/charge_stop_threshold",
];

#[tauri::command]
pub async fn check_permissions_status() -> ApiResponse<PermissionStatus> {
    let mut missing_files = Vec::new();

    for file_path in REQUIRED_FILES {
        if Path::new(file_path).exists() {
            // Check if we can write to it
            match fs::OpenOptions::new().write(true).open(file_path) {
                Ok(_) => {
                    // We have permission
                }
                Err(_) => {
                    missing_files.push(file_path.to_string());
                }
            }
        }
    }

    // Also check if fan control helper + polkit rule are installed
    // Only check the helper — /etc/polkit-1/rules.d/ is root-only so
    // Path::exists() on the polkit rule always fails for normal users.
    if !Path::new(HELPER_PATH).exists() {
        // Can we at least write to the fan file directly?
        let fan_path = "/proc/acpi/ibm/fan";
        if Path::new(fan_path).exists()
            && fs::OpenOptions::new().write(true).open(fan_path).is_err()
        {
            missing_files.push("Fan control helper (not installed)".to_string());
        }
    }

    let has_permissions = missing_files.is_empty();

    ApiResponse {
        success: true,
        data: Some(PermissionStatus {
            has_permissions,
            missing_files,
        }),
        error: None,
    }
}

#[tauri::command]
pub async fn setup_permissions() -> ApiResponse<String> {
    println!("[Permissions] Setting up file permissions...");

    let username = std::env::var("USER").unwrap_or_else(|_| "root".to_string());

    // Validate username to prevent command injection (only allow alphanumeric, dash, underscore)
    if !username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return ApiResponse {
            success: false,
            data: None,
            error: Some("Invalid username detected".to_string()),
        };
    }

    // Create a script that sets up all permissions
    let mut script_lines = vec![
        "#!/bin/bash".to_string(),
        "set -e".to_string(),
        "echo 'Setting up ThinkUtils permissions...'".to_string(),
    ];

    // Add chmod commands for each file that exists
    for file_path in REQUIRED_FILES {
        if Path::new(file_path).exists() {
            script_lines.push(format!("if [ -f {} ]; then", file_path));
            script_lines.push(format!("  chmod 666 {} 2>/dev/null || true", file_path));
            script_lines.push(format!(
                "  chown {}:root {} 2>/dev/null || true",
                username, file_path
            ));
            script_lines.push("fi".to_string());
        }
    }

    // Also set up the fan control file if it exists
    let fan_file = "/sys/devices/platform/thinkpad_hwmon/pwm1_enable";
    if Path::new(fan_file).exists() {
        script_lines.push(format!("if [ -f {} ]; then", fan_file));
        script_lines.push(format!("  chmod 666 {} 2>/dev/null || true", fan_file));
        script_lines.push(format!(
            "  chown {}:root {} 2>/dev/null || true",
            username, fan_file
        ));
        script_lines.push("fi".to_string());
    }

    // Also install the fan control helper + polkit rule so the user
    // doesn't have to click "Grant Permissions" again on the fan page.
    // Uses shared constants from fan_control module to avoid duplication.
    script_lines.push(format!("cat > {} << 'HELPEREOF'", HELPER_PATH));
    script_lines.push(HELPER_SCRIPT.trim().to_string());
    script_lines.push("HELPEREOF".to_string());
    script_lines.push(format!("chmod 755 {}", HELPER_PATH));

    script_lines.push("mkdir -p /etc/polkit-1/rules.d".to_string());
    script_lines.push(format!("cat > {} << 'RULEEOF'", POLKIT_RULE_PATH));
    script_lines.push(POLKIT_RULE.trim().to_string());
    script_lines.push("RULEEOF".to_string());
    script_lines.push(
        "systemctl reload polkit 2>/dev/null || killall -HUP polkitd 2>/dev/null || true"
            .to_string(),
    );

    script_lines.push("echo 'Permissions setup complete!'".to_string());
    script_lines.push("exit 0".to_string());

    let script_content = script_lines.join("\n");

    // Secure temp file creation (random name, O_EXCL to prevent symlink attacks)
    let random = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_script = format!("/tmp/thinkutils_perms_{}.sh", random);

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let mut file = match fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o700)
            .open(&temp_script)
        {
            Ok(f) => f,
            Err(e) => {
                return ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Failed to create setup script: {}", e)),
                };
            }
        };

        if let Err(e) = file.write_all(script_content.as_bytes()) {
            let _ = fs::remove_file(&temp_script);
            return ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to write setup script: {}", e)),
            };
        }
    }

    #[cfg(not(unix))]
    {
        if let Err(e) = fs::write(&temp_script, &script_content) {
            return ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to create setup script: {}", e)),
            };
        }
    }

    // Run with pkexec - this will prompt for password once
    match tokio::process::Command::new("pkexec")
        .arg("bash")
        .arg(&temp_script)
        .output()
        .await
    {
        Ok(output) => {
            let _ = fs::remove_file(&temp_script);

            if output.status.success() {
                println!("[Permissions] ✓ Permissions setup successful");
                ApiResponse {
                    success: true,
                    data: Some("Permissions configured successfully".to_string()),
                    error: None,
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("[Permissions] ✗ Setup failed: {}", stderr);
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Permission setup failed: {}", stderr)),
                }
            }
        }
        Err(e) => {
            let _ = fs::remove_file(&temp_script);
            println!("[Permissions] ✗ Failed to execute pkexec: {}", e);
            ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to setup permissions: {}", e)),
            }
        }
    }
}
