use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::fan_control::{
    helper_is_packaged, helper_path, polkit_rule, HELPER_SCRIPT, HELPER_SELF_INSTALL_PATH,
    POLKIT_RULE_PATH,
};

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

// Files that need write permissions and are the same on every machine.
//
// The battery thresholds are NOT here: their attribute names vary, and this list
// previously named the legacy thinkpad_acpi pair while battery.rs wrote the
// standard kernel pair. Both exist on a ThinkPad and report the same value, but
// they are separate sysfs files, so granting one never affected the other --
// "Grant Permissions" silently never fixed battery thresholds. They come from
// battery::threshold_paths() now, which is the single source of truth.
//
// thinkpad_hwmon/pwm1 is also gone: that path does not exist. The real attribute
// lives under .../thinkpad_hwmon/hwmon/hwmonN/pwm1, and the exists() guard below
// meant the wrong path was silently skipped rather than reported.
const REQUIRED_FILES: &[&str] = &[
    "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor",
    // Turbo lives at a different path per vendor: intel_pstate exposes no_turbo,
    // everything else (amd_pstate, acpi-cpufreq) exposes cpufreq/boost. Only the
    // Intel one was listed, so on an AMD ThinkPad "Grant Permissions" reported
    // success while leaving the boost control unwritable. Both are listed and
    // non-existent paths are skipped, so each machine gets whichever it has.
    "/sys/devices/system/cpu/intel_pstate/no_turbo",
    "/sys/devices/system/cpu/cpufreq/boost",
];

/// Every sysfs file the app wants writable, resolved for this machine.
fn required_files() -> Vec<String> {
    let mut files: Vec<String> = REQUIRED_FILES.iter().map(|s| s.to_string()).collect();
    if let Some((start, stop)) = crate::battery::threshold_paths() {
        files.push(start);
        files.push(stop);
    }
    // The thinkpad hwmon PWM lives under a numbered hwmon directory, so it has to
    // be discovered rather than hardcoded.
    if let Ok(entries) = fs::read_dir("/sys/devices/platform/thinkpad_hwmon/hwmon") {
        for entry in entries.flatten() {
            let pwm = entry.path().join("pwm1");
            if pwm.exists() {
                files.push(pwm.to_string_lossy().to_string());
            }
        }
    }
    files
}

#[tauri::command]
pub async fn check_permissions_status() -> ApiResponse<PermissionStatus> {
    let mut missing_files = Vec::new();

    for file_path in required_files() {
        if Path::new(&file_path).exists() {
            // Check if we can write to it
            match fs::OpenOptions::new().write(true).open(&file_path) {
                Ok(_) => {
                    // We have permission
                }
                Err(_) => {
                    missing_files.push(file_path.clone());
                }
            }
        }
    }

    // Also check if fan control helper + polkit rule are installed
    // Only check the helper — /etc/polkit-1/rules.d/ is root-only so
    // Path::exists() on the polkit rule always fails for normal users.
    if helper_path().is_none() {
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
    for file_path in required_files() {
        if Path::new(&file_path).exists() {
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

    // Install the fan helper and polkit rule -- but only when this app owns
    // them. On a distro-packaged install both files belong to dpkg/rpm/pacman,
    // and rewriting them would put the package database out of sync with the
    // filesystem and leave orphans behind on uninstall.
    if helper_is_packaged() {
        println!("[Permissions] Packaged helper detected; only adjusting sysfs permissions");
    } else {
        script_lines.push(format!(
            "mkdir -p \"$(dirname {})\"",
            HELPER_SELF_INSTALL_PATH
        ));
        script_lines.push(format!("cat > {} << 'HELPEREOF'", HELPER_SELF_INSTALL_PATH));
        script_lines.push(HELPER_SCRIPT.trim().to_string());
        script_lines.push("HELPEREOF".to_string());
        script_lines.push(format!("chmod 755 {}", HELPER_SELF_INSTALL_PATH));

        script_lines.push("mkdir -p /etc/polkit-1/rules.d".to_string());
        script_lines.push(format!("cat > {} << 'RULEEOF'", POLKIT_RULE_PATH));
        script_lines.push(polkit_rule().trim().to_string());
        script_lines.push("RULEEOF".to_string());
        script_lines.push(
            "systemctl reload polkit 2>/dev/null || killall -HUP polkitd 2>/dev/null || true"
                .to_string(),
        );
    }

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
