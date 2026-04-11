use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use regex::Regex;

const PROC_FAN: &str = "/proc/acpi/ibm/fan";
pub const HELPER_PATH: &str = "/usr/local/bin/thinkutils-fan-control";
pub const POLKIT_RULE_PATH: &str = "/etc/polkit-1/rules.d/50-thinkutils.rules";

/// Dedicated fan control helper script - validates input before writing to fan.
/// Installed at HELPER_PATH by setup_permissions().
pub const HELPER_SCRIPT: &str = r#"#!/bin/bash
set -e
FAN="/proc/acpi/ibm/fan"
case "$1" in
    "level auto"|"level full-speed"|"level 0"|"level 1"|"level 2"|"level 3"|"level 4"|"level 5"|"level 6"|"level 7")
        echo "$1" > "$FAN"
        ;;
    *)
        echo "Invalid command" >&2
        exit 1
        ;;
esac
"#;

/// Polkit rule that only allows the dedicated helper script without password.
/// Much tighter than allowing arbitrary bash execution.
pub const POLKIT_RULE: &str = r#"/* ThinkUtils: Allow passwordless fan control via dedicated helper only */
polkit.addRule(function(action, subject) {
    if (action.id == "org.freedesktop.policykit.exec") {
        var program = action.lookup("program");
        if (program == "/usr/local/bin/thinkutils-fan-control") {
            if (subject.isInGroup("wheel") || subject.isInGroup("sudo")) {
                return polkit.Result.YES;
            }
        }
    }
});
"#;

/// Valid fan speed values (whitelist)
const VALID_SPEEDS: &[&str] = &[
    "auto", "full-speed", "0", "1", "2", "3", "4", "5", "6", "7",
];

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

/// Create a temp script securely (O_EXCL prevents symlink attacks, random name, restricted perms)
#[cfg(unix)]
fn create_secure_temp_script(content: &str) -> Result<String, String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let random = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = format!("/tmp/thinkutils_{}.sh", random);

    let mut file = fs::OpenOptions::new()
        .create_new(true) // O_EXCL: fail if exists, don't follow symlinks
        .write(true)
        .mode(0o700) // Only owner can read/write/execute
        .open(&path)
        .map_err(|e| format!("Failed to create temp script: {}", e))?;

    file.write_all(content.as_bytes())
        .map_err(|e| {
            let _ = fs::remove_file(&path);
            format!("Failed to write temp script: {}", e)
        })?;

    Ok(path)
}

#[cfg(not(unix))]
fn create_secure_temp_script(content: &str) -> Result<String, String> {
    let random = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = format!("/tmp/thinkutils_{}.sh", random);
    fs::write(&path, content)
        .map_err(|e| format!("Failed to create temp script: {}", e))?;
    Ok(path)
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
    // Validate speed against whitelist
    if !VALID_SPEEDS.contains(&speed.as_str()) {
        return ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Invalid fan speed: {}", speed)),
        };
    }

    println!("[Fan] Setting speed to: {}", speed);
    let command_str = format!("level {}", speed);

    // 1. Try direct write (no elevation needed)
    if fs::write(PROC_FAN, &command_str).is_ok() {
        println!("[Fan] ✓ Speed set successfully");
        return ApiResponse {
            success: true,
            data: Some(format!("Fan speed set to: {}", speed)),
            error: None,
        };
    }

    println!("[Fan] Need elevated permissions");

    // 2. Use dedicated helper if installed (passwordless via polkit rule)
    if std::path::Path::new(HELPER_PATH).exists() {
        match tokio::process::Command::new("pkexec")
            .arg(HELPER_PATH)
            .arg(&command_str)
            .output()
            .await
        {
            Ok(output) if output.status.success() => {
                println!("[Fan] ✓ Speed set via helper");
                return ApiResponse {
                    success: true,
                    data: Some(format!("Fan speed set to: {}", speed)),
                    error: None,
                };
            }
            Ok(_) => {
                return ApiResponse {
                    success: false,
                    data: None,
                    error: Some("Permission denied. Click 'Grant Permissions' to enable passwordless fan control.".to_string()),
                };
            }
            Err(e) => {
                return ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Failed to execute helper: {}", e)),
                };
            }
        }
    }

    // 3. Fallback: secure temp script + pkexec (will prompt for password)
    let script_content = format!(
        "#!/bin/bash\nset -e\necho '{}' > {}\nexit 0\n",
        command_str, PROC_FAN
    );

    let temp_script = match create_secure_temp_script(&script_content) {
        Ok(path) => path,
        Err(e) => {
            return ApiResponse {
                success: false,
                data: None,
                error: Some(e),
            };
        }
    };

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
                ApiResponse {
                    success: true,
                    data: Some(format!("Fan speed set to: {}", speed)),
                    error: None,
                }
            } else {
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some("Permission denied. Click 'Grant Permissions' to avoid repeated password prompts.".to_string()),
                }
            }
        }
        Err(e) => {
            let _ = fs::remove_file(&temp_script);
            ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to execute pkexec: {}", e)),
            }
        }
    }
}

#[tauri::command]
pub fn check_permissions() -> ApiResponse<bool> {
    // Check if direct write is possible
    let direct_write = fs::OpenOptions::new().write(true).open(PROC_FAN).is_ok();

    // Check if the dedicated helper and polkit rule are both installed
    let helper_installed = std::path::Path::new(HELPER_PATH).exists()
        && std::path::Path::new(POLKIT_RULE_PATH).exists();

    let has_permission = direct_write || helper_installed;

    ApiResponse {
        success: true,
        data: Some(has_permission),
        error: None,
    }
}
