//! Reading fans and temperatures from sysfs hwmon directly.
//!
//! This replaces shelling out to `sensors` and parsing its text output, which
//! had three problems:
//!
//!   * It needed lm-sensors installed, so temperatures silently vanished on a
//!     machine without it.
//!   * It hardcoded the chip name `thinkpad-isa-0000`.
//!   * It double-counted the first fan. `/proc/acpi/ibm/fan` contributed a
//!     "Fan1" entry and the `sensors` output contributed "fan1", so the UI
//!     listed the same fan twice under two names.
//!
//! The structural point, and the reason this exists: **tachometers and control
//! channels are not one-to-one**. A dual-fan ThinkPad — P1, P15, X1 Extreme —
//! reports two tachometers through hwmon but exposes a single PWM channel,
//! because thinkpad_acpi writes the same level to both fans. Code that assumes
//! one fan per channel is wrong on every one of those machines.
//!
//! `/proc/acpi/ibm/fan` cannot see the second fan at all: it has a `speed:`
//! field and no `speed2:`. hwmon is the only way to read it.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::hardware_root;

const HWMON_ROOT: &str = "/sys/class/hwmon";

/// One tachometer: something that reports an RPM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tachometer {
    /// Chip name plus index, e.g. "thinkpad fan1".
    pub label: String,
    pub rpm: u32,
    /// The chip this reading came from, so the UI can group by device.
    pub chip: String,
}

/// One temperature reading.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Temperature {
    pub label: String,
    pub celsius: f32,
    pub chip: String,
}

/// A control channel — a PWM output. May drive more than one tachometer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ControlChannel {
    pub label: String,
    pub chip: String,
    /// Raw pwmN value, 0-255, when readable.
    pub pwm: Option<u8>,
    /// pwmN_enable: 0 = full speed (NOT off), 1 = manual, 2 = automatic.
    pub enable: Option<u8>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HwmonReadings {
    pub tachometers: Vec<Tachometer>,
    pub temperatures: Vec<Temperature>,
    pub channels: Vec<ControlChannel>,
}

fn read_trimmed(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

fn read_u32(path: &Path) -> Option<u32> {
    read_trimmed(path)?.parse().ok()
}

/// Every hwmon chip directory, resolved through the hardware root so a captured
/// profile can stand in for real hardware.
fn hwmon_dirs() -> Vec<PathBuf> {
    let root = hardware_root::resolve(HWMON_ROOT);
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("name").exists())
        .collect();
    // Stable order so the UI does not reshuffle between polls.
    dirs.sort();
    dirs
}

/// Human label for a sensor: prefer the chip's own `*_label` file, fall back to
/// the attribute name. `sensors` does the same thing, and it is why a ThinkPad
/// reports "CPU" rather than "temp1".
fn sensor_label(dir: &Path, prefix: &str, index: u32, chip: &str) -> String {
    read_trimmed(&dir.join(format!("{}{}_label", prefix, index)))
        .unwrap_or_else(|| format!("{} {}{}", chip, prefix, index))
}

/// Read every fan, temperature and PWM channel this machine exposes.
pub fn read_all() -> HwmonReadings {
    let mut out = HwmonReadings::default();

    for dir in hwmon_dirs() {
        let chip = read_trimmed(&dir.join("name")).unwrap_or_else(|| "unknown".to_string());

        // Attribute indices are 1-based and not necessarily contiguous, so probe
        // a fixed range rather than stopping at the first gap.
        for i in 1..=8u32 {
            if let Some(rpm) = read_u32(&dir.join(format!("fan{}_input", i))) {
                // A stopped fan reports 0, which is legitimate. A missing sensor
                // has no file at all, which is why this is keyed on the read
                // succeeding rather than on the value.
                out.tachometers.push(Tachometer {
                    label: sensor_label(&dir, "fan", i, &chip),
                    rpm,
                    chip: chip.clone(),
                });
            }

            if let Some(millidegrees) = read_u32(&dir.join(format!("temp{}_input", i))) {
                let celsius = millidegrees as f32 / 1000.0;
                // Sanity bound: some chips expose placeholder sensors that read
                // absurd values when nothing is connected.
                if celsius > 0.0 && celsius < 150.0 {
                    out.temperatures.push(Temperature {
                        label: sensor_label(&dir, "temp", i, &chip),
                        celsius,
                        chip: chip.clone(),
                    });
                }
            }

            let pwm_path = dir.join(format!("pwm{}", i));
            if pwm_path.exists() {
                out.channels.push(ControlChannel {
                    label: format!("{} pwm{}", chip, i),
                    chip: chip.clone(),
                    pwm: read_u32(&pwm_path).map(|v| v.min(255) as u8),
                    enable: read_u32(&dir.join(format!("pwm{}_enable", i)))
                        .map(|v| v.min(255) as u8),
                });
            }
        }
    }

    out
}

/// Fans belonging to the ThinkPad's own controller.
///
/// Used to answer "how many fans does this machine have" without counting the
/// NVMe or wifi chips' sensors.
pub fn thinkpad_tachometers(readings: &HwmonReadings) -> Vec<&Tachometer> {
    readings
        .tachometers
        .iter()
        .filter(|t| t.chip == "thinkpad")
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs against the captured P1 Gen 4i profile, which is a dual-fan machine.
    /// The env var is process-global, so this serialises access and restores it.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_profile<T>(name: &str, f: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let previous = std::env::var(hardware_root::HARDWARE_ROOT_ENV).ok();

        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/hardware/");
        std::env::set_var(
            hardware_root::HARDWARE_ROOT_ENV,
            format!("{}{}", root, name),
        );

        let out = f();

        match previous {
            Some(p) => std::env::set_var(hardware_root::HARDWARE_ROOT_ENV, p),
            None => std::env::remove_var(hardware_root::HARDWARE_ROOT_ENV),
        }
        out
    }

    /// The whole reason this module exists. procfs reports one speed; hwmon
    /// reports both fans.
    #[test]
    fn reads_both_fans_of_a_dual_fan_thinkpad() {
        with_profile("thinkpad-p1-gen-4i", || {
            let readings = read_all();
            let fans = thinkpad_tachometers(&readings);
            assert_eq!(
                fans.len(),
                2,
                "expected two ThinkPad tachometers, got {:?}",
                fans
            );
        });
    }

    /// Two fans, one PWM: thinkpad_acpi writes the same level to both, so they
    /// cannot be driven independently. Anything presenting per-fan control on
    /// these machines would be lying.
    #[test]
    fn a_dual_fan_thinkpad_still_has_one_control_channel() {
        with_profile("thinkpad-p1-gen-4i", || {
            let readings = read_all();
            let channels: Vec<_> = readings
                .channels
                .iter()
                .filter(|c| c.chip == "thinkpad")
                .collect();
            assert_eq!(channels.len(), 1, "expected a single PWM channel");
        });
    }

    /// The old code took the first fan from procfs AND from `sensors`, listing
    /// it twice under two names.
    #[test]
    fn each_fan_appears_exactly_once() {
        with_profile("thinkpad-p1-gen-4i", || {
            let readings = read_all();
            let mut labels: Vec<&str> = readings
                .tachometers
                .iter()
                .map(|t| t.label.as_str())
                .collect();
            labels.sort_unstable();
            let before = labels.len();
            labels.dedup();
            assert_eq!(
                before,
                labels.len(),
                "duplicate tachometer labels: {:?}",
                labels
            );
        });
    }

    #[test]
    fn reads_temperatures_without_lm_sensors() {
        with_profile("thinkpad-p1-gen-4i", || {
            let readings = read_all();
            assert!(
                !readings.temperatures.is_empty(),
                "no temperatures read from the captured profile"
            );
            for t in &readings.temperatures {
                assert!(t.celsius > 0.0 && t.celsius < 150.0, "implausible: {:?}", t);
            }
        });
    }

    /// A machine with no hwmon at all must yield empty lists, not panic — that
    /// is every container, and most non-ThinkPad hardware for the fan parts.
    #[test]
    fn missing_hwmon_yields_no_readings() {
        with_profile("nonexistent-profile", || {
            let readings = read_all();
            assert!(readings.tachometers.is_empty());
            assert!(readings.channels.is_empty());
        });
    }
}

#[cfg(test)]
mod live_hardware {
    use super::*;

    /// Reads the real machine, not a fixture. Skipped where there is no
    /// thinkpad hwmon chip, so it is a no-op in CI and on other hardware.
    #[test]
    fn reads_this_machine_if_it_is_a_thinkpad() {
        let readings = read_all();
        let fans = thinkpad_tachometers(&readings);
        if fans.is_empty() {
            eprintln!("no thinkpad hwmon chip here - skipping live check");
            return;
        }

        println!("live: {} thinkpad tachometer(s)", fans.len());
        for f in &fans {
            println!("  {} = {} RPM", f.label, f.rpm);
            assert!(f.rpm < 20_000, "implausible RPM: {:?}", f);
        }

        let channels = readings
            .channels
            .iter()
            .filter(|c| c.chip == "thinkpad")
            .count();
        println!("live: {} thinkpad control channel(s)", channels);
        assert!(
            channels <= fans.len(),
            "more control channels than fans is not a shape this driver produces"
        );
    }
}
