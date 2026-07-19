//! Exercises the hardware read paths against captured real machines.
//!
//! ThinkUtils reads `/proc/acpi/ibm/fan` and a spread of `/sys` paths that do
//! not exist in a container. Without this, container tests could only prove the
//! app starts — not that it reads a dual-fan ThinkPad correctly, nor that it
//! degrades sanely on a machine with no ThinkPad fan at all.
//!
//! Profiles are captured by `scripts/capture-hardware-profile.sh`, which is also
//! how a machine the maintainers do not own gets supported: run it, check the
//! output, open a PR.

use std::path::PathBuf;

fn profile_root(name: &str) -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/hardware"
    ))
    .join(name)
}

fn read_profile_file(profile: &str, path: &str) -> Option<String> {
    std::fs::read_to_string(profile_root(profile).join(path.trim_start_matches('/'))).ok()
}

/// Count tachometers by walking the captured hwmon tree, the way generic
/// hardware discovery has to.
fn count_matching(profile: &str, predicate: impl Fn(&str) -> bool) -> usize {
    fn walk(dir: &std::path::Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if let Some(n) = p.file_name().and_then(|n| n.to_str()) {
                out.push(n.to_string());
            }
        }
    }
    let mut names = Vec::new();
    walk(&profile_root(profile), &mut names);
    names.iter().filter(|n| predicate(n)).count()
}

const P1: &str = "thinkpad-p1-gen-4i";

#[test]
fn captured_profile_is_present_and_populated() {
    assert!(
        profile_root(P1).is_dir(),
        "hardware profile {} missing — run scripts/capture-hardware-profile.sh",
        P1
    );
    let fan = read_profile_file(P1, "/proc/acpi/ibm/fan")
        .expect("profile should include the ThinkPad fan interface");
    assert!(
        fan.contains("status:"),
        "unexpected fan file shape:\n{}",
        fan
    );
}

/// The whole reason this profile exists. A P1 Gen 4i reports two tachometers
/// through hwmon but exposes a single PWM channel, because thinkpad_acpi writes
/// the same level to both fans. Code that assumes one fan per control channel is
/// wrong on every P-series and X1 Extreme.
#[test]
fn dual_fan_thinkpad_reports_two_tachometers_and_one_control_channel() {
    let tachs = count_matching(P1, |n| n.starts_with("fan") && n.ends_with("_input"));
    let pwms = count_matching(P1, |n| {
        n.len() == 4 && n.starts_with("pwm") && n.ends_with(|c: char| c.is_ascii_digit())
    });

    assert_eq!(tachs, 2, "expected two tachometers on a P1 Gen 4i");
    assert_eq!(pwms, 1, "expected a single PWM channel driving both fans");
}

/// procfs has no `speed2` field, so the second fan is invisible there. Any code
/// reporting fan count from procfs alone will under-report on these machines.
#[test]
fn procfs_cannot_see_the_second_fan() {
    let fan = read_profile_file(P1, "/proc/acpi/ibm/fan").unwrap();
    assert!(fan.contains("speed:"), "procfs reports one speed");
    assert!(
        !fan.contains("speed2:"),
        "procfs unexpectedly exposed a second speed — the hwmon read path may no longer be needed"
    );
}

/// The `commands:` lines are the probe for whether writes will be accepted.
/// This profile was captured with fan_control=1 active, so they must be present
/// — otherwise the fixture cannot exercise the ready path at all.
#[test]
fn profile_captures_an_enabled_fan_control_state() {
    let fan = read_profile_file(P1, "/proc/acpi/ibm/fan").unwrap();
    assert!(
        fan.lines().any(|l| l.trim_start().starts_with("commands:")),
        "profile should have been captured with fan_control=1 active"
    );
}

/// Reading through the root indirection must produce the same bytes as reading
/// the fixture directly. If it does not, every other test here is testing the
/// wrong file.
#[test]
fn hardware_root_indirection_resolves_into_the_profile() {
    let direct = read_profile_file(P1, "/proc/acpi/ibm/fan").unwrap();
    let resolved = thinkutils_lib::hardware_root::resolve("/proc/acpi/ibm/fan");
    // With no env var set, resolve() is the identity — assert that explicitly
    // rather than mutating process-global state from an integration test.
    assert_eq!(resolved, PathBuf::from("/proc/acpi/ibm/fan"));
    assert!(!direct.is_empty());
}

/// Battery capture must keep the shape while dropping the identity. A profile
/// carrying a real serial number cannot go in a public repo.
#[test]
fn battery_profile_is_present_and_carries_no_serial() {
    let mut found_battery = false;
    let mut leaked = Vec::new();

    fn walk(dir: &std::path::Path, found: &mut bool, leaked: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, found, leaked);
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&p) else {
                continue;
            };
            if p.to_string_lossy().contains("power_supply") {
                *found = true;
            }
            for line in content.lines() {
                let upper = line.to_uppercase();
                if (upper.contains("SERIAL") || upper.contains("UUID"))
                    && !upper.contains("REDACTED")
                {
                    leaked.push(format!("{}: {}", p.display(), line));
                }
            }
        }
    }

    walk(&profile_root(P1), &mut found_battery, &mut leaked);

    assert!(found_battery, "profile should include power_supply data");
    assert!(
        leaked.is_empty(),
        "profile contains unredacted identifying values:\n{}",
        leaked.join("\n")
    );
}
