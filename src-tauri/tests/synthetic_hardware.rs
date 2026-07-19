//! Hardware shapes nobody here owns, built from scratch.
//!
//! The captured profile covers exactly one machine: a dual-fan ThinkPad P1 with
//! `fan_control=1` already enabled. Every other shape the app has to survive is
//! untested by it — and those are the ones most likely to break, because nobody
//! is running the app on them during development.
//!
//! These build a sysfs tree in a temp dir and point `THINKUTILS_HARDWARE_ROOT`
//! at it, exercising the same code path a real machine takes. Real values come
//! from the kernel's documented formats: RPM as a plain integer, temperatures in
//! millidegrees, `pwmN_enable` as 0/1/2.
//!
//! Machines represented:
//!
//!   single-fan ThinkPad    the common case (T/X series)
//!   dual-fan ThinkPad      P/X1 Extreme — two tachometers, ONE control channel
//!   fan_control disabled   procfs present but no `commands:` lines
//!   non-ThinkPad laptop    battery and cpufreq, no ThinkPad fan interface
//!   no hardware at all     every container
//!   hostile values         negative, absurd and non-numeric sysfs contents

use std::fs;
use std::path::PathBuf;
use thinkutils_lib::hardware_root::HARDWARE_ROOT_ENV;
use thinkutils_lib::hwmon;

/// `THINKUTILS_HARDWARE_ROOT` is process-global, so every test that sets it must
/// serialise. Without this they interleave and fail in ways that look like logic
/// bugs rather than a race.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Machine {
    root: PathBuf,
}

impl Machine {
    fn new(name: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "thinkutils_synth_{}_{}_{}",
            name,
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&root).expect("create machine root");
        Self { root }
    }

    fn write(&self, rel: &str, contents: &str) -> &Self {
        let path = self.root.join(rel.trim_start_matches('/'));
        fs::create_dir_all(path.parent().expect("has parent")).expect("create dirs");
        fs::write(&path, contents).expect("write file");
        self
    }

    /// One hwmon chip. `fans` are RPM values, `pwm` is an optional (value,
    /// enable) pair, `temps` are (label, celsius).
    fn hwmon_chip(
        &self,
        index: u32,
        name: &str,
        fans: &[u32],
        pwm: Option<(u8, u8)>,
        temps: &[(&str, f32)],
    ) -> &Self {
        let dir = format!("/sys/class/hwmon/hwmon{}", index);
        self.write(&format!("{}/name", dir), name);

        for (i, rpm) in fans.iter().enumerate() {
            self.write(&format!("{}/fan{}_input", dir, i + 1), &rpm.to_string());
        }
        if let Some((value, enable)) = pwm {
            self.write(&format!("{}/pwm1", dir), &value.to_string());
            self.write(&format!("{}/pwm1_enable", dir), &enable.to_string());
        }
        for (i, (label, celsius)) in temps.iter().enumerate() {
            // The kernel reports millidegrees, not degrees.
            self.write(
                &format!("{}/temp{}_input", dir, i + 1),
                &((celsius * 1000.0) as i64).to_string(),
            );
            self.write(&format!("{}/temp{}_label", dir, i + 1), label);
        }
        self
    }

    /// The ThinkPad fan interface. `controllable` decides whether the
    /// `commands:` lines are present — their absence is exactly how the kernel
    /// signals that `fan_control=1` was not set and every write will be refused.
    fn thinkpad_fan(&self, speed: u32, level: &str, controllable: bool) -> &Self {
        let mut content = format!(
            "status:\t\tenabled\nspeed:\t\t{}\nlevel:\t\t{}\n",
            speed, level
        );
        if controllable {
            content.push_str(
                "commands:\tlevel <level> (<level> is 0-7, auto, disengaged, full-speed)\n\
                 commands:\tenable, disable\n\
                 commands:\twatchdog <timeout> (<timeout> is 0 (off), 1-120 (seconds))\n",
            );
        }
        self.write("/proc/acpi/ibm/fan", &content)
    }

    fn battery(&self, capacity: u8, start: u8, stop: u8) -> &Self {
        let dir = "/sys/class/power_supply/BAT0";
        self.write(&format!("{}/type", dir), "Battery");
        self.write(&format!("{}/capacity", dir), &capacity.to_string());
        self.write(&format!("{}/status", dir), "Discharging");
        self.write(
            &format!("{}/charge_control_start_threshold", dir),
            &start.to_string(),
        );
        self.write(
            &format!("{}/charge_control_end_threshold", dir),
            &stop.to_string(),
        );
        self
    }

    fn run<T>(&self, f: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let previous = std::env::var(HARDWARE_ROOT_ENV).ok();
        std::env::set_var(HARDWARE_ROOT_ENV, &self.root);

        let out = f();

        match previous {
            Some(p) => std::env::set_var(HARDWARE_ROOT_ENV, p),
            None => std::env::remove_var(HARDWARE_ROOT_ENV),
        }
        out
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn thinkpad_fan_count(readings: &hwmon::HwmonReadings) -> usize {
    hwmon::thinkpad_tachometers(readings).len()
}

// --- The common ThinkPad: one fan -----------------------------------------

#[test]
fn single_fan_thinkpad_reports_one_fan() {
    let m = Machine::new("single_fan");
    m.thinkpad_fan(2800, "auto", true)
        .hwmon_chip(0, "thinkpad", &[2800], Some((128, 2)), &[("CPU", 55.0)])
        .battery(72, 75, 80);

    m.run(|| {
        let r = hwmon::read_all();
        assert_eq!(thinkpad_fan_count(&r), 1);
        assert_eq!(
            r.channels.iter().filter(|c| c.chip == "thinkpad").count(),
            1
        );
    });
}

// --- P-series: two fans, one channel --------------------------------------

/// The shape that motivated reading hwmon at all. Two tachometers, one PWM,
/// because thinkpad_acpi writes the same level to both.
#[test]
fn dual_fan_thinkpad_has_two_fans_but_one_channel() {
    let m = Machine::new("dual_fan");
    m.thinkpad_fan(3100, "auto", true).hwmon_chip(
        0,
        "thinkpad",
        &[3100, 2900],
        Some((160, 2)),
        &[("CPU", 61.0), ("GPU", 58.0)],
    );

    m.run(|| {
        let r = hwmon::read_all();
        assert_eq!(thinkpad_fan_count(&r), 2, "both fans must be visible");
        assert_eq!(
            r.channels.iter().filter(|c| c.chip == "thinkpad").count(),
            1,
            "the driver exposes one channel for both fans"
        );
    });
}

/// A four-fan machine would still be read correctly. No ThinkPad has this today,
/// but the reader must not cap at two just because that is what we have tested
/// on real hardware.
#[test]
fn more_than_two_fans_are_all_reported() {
    let m = Machine::new("quad_fan");
    m.hwmon_chip(
        0,
        "thinkpad",
        &[1000, 2000, 3000, 4000],
        Some((200, 1)),
        &[],
    );

    m.run(|| {
        let r = hwmon::read_all();
        assert_eq!(thinkpad_fan_count(&r), 4);
        let rpms: Vec<u32> = hwmon::thinkpad_tachometers(&r)
            .iter()
            .map(|t| t.rpm)
            .collect();
        assert_eq!(rpms, vec![1000, 2000, 3000, 4000]);
    });
}

// --- Non-ThinkPad and absent hardware -------------------------------------

/// A Dell or HP laptop: battery and temperatures, no ThinkPad fan interface.
/// The app must still read what is there rather than treating the machine as
/// unsupported outright.
#[test]
fn non_thinkpad_laptop_still_reports_sensors() {
    let m = Machine::new("non_thinkpad");
    m.hwmon_chip(0, "coretemp", &[], None, &[("Package id 0", 47.0)])
        .hwmon_chip(1, "dell_smm", &[2400], None, &[("CPU", 47.0)])
        .battery(88, 0, 100);

    m.run(|| {
        let r = hwmon::read_all();
        assert_eq!(thinkpad_fan_count(&r), 0, "no thinkpad chip here");
        assert_eq!(r.tachometers.len(), 1, "the dell_smm fan is still readable");
        assert!(
            !r.temperatures.is_empty(),
            "temperatures must still be read"
        );
    });
}

/// Every container, and any desktop without hwmon. Must be empty, not a panic.
#[test]
fn machine_with_no_hwmon_reads_empty() {
    let m = Machine::new("bare");
    m.write("/etc/hostname", "container");

    m.run(|| {
        let r = hwmon::read_all();
        assert!(r.tachometers.is_empty());
        assert!(r.temperatures.is_empty());
        assert!(r.channels.is_empty());
    });
}

// --- Malformed and hostile sysfs contents ---------------------------------

/// sysfs reads can return empty strings, error text, or values from a sensor
/// that is not connected. None of it may panic, and implausible temperatures
/// must not reach the UI.
#[test]
fn implausible_and_unparsable_values_are_rejected() {
    let m = Machine::new("hostile");
    m.write("/sys/class/hwmon/hwmon0/name", "flaky")
        .write("/sys/class/hwmon/hwmon0/fan1_input", "")
        .write("/sys/class/hwmon/hwmon0/fan2_input", "not-a-number")
        .write("/sys/class/hwmon/hwmon0/fan3_input", "1500")
        // 250C: a disconnected sensor pegged high. Above the sanity bound.
        .write("/sys/class/hwmon/hwmon0/temp1_input", "250000")
        // -40C reads as a huge u32 after the kernel's signed formatting; either
        // way it must not be shown as a plausible reading.
        .write("/sys/class/hwmon/hwmon0/temp2_input", "-40000")
        .write("/sys/class/hwmon/hwmon0/temp3_input", "52000");

    m.run(|| {
        let r = hwmon::read_all();
        assert_eq!(r.tachometers.len(), 1, "only the parsable fan counts");
        assert_eq!(r.tachometers[0].rpm, 1500);
        assert_eq!(r.temperatures.len(), 1, "only the plausible temperature");
        assert!((r.temperatures[0].celsius - 52.0).abs() < 0.1);
    });
}

/// A stopped fan legitimately reports 0. That is a reading, not a missing
/// sensor, and dropping it would hide the most safety-relevant state there is.
#[test]
fn a_stopped_fan_is_reported_rather_than_dropped() {
    let m = Machine::new("stopped");
    m.hwmon_chip(0, "thinkpad", &[0], Some((0, 1)), &[]);

    m.run(|| {
        let r = hwmon::read_all();
        assert_eq!(thinkpad_fan_count(&r), 1, "a stopped fan is still a fan");
        assert_eq!(hwmon::thinkpad_tachometers(&r)[0].rpm, 0);
    });
}

/// pwm1_enable=0 means FULL SPEED on thinkpad_acpi, not off. Reading it as
/// "disabled" would invert the meaning of the most aggressive setting.
#[test]
fn pwm_enable_zero_is_captured_verbatim() {
    let m = Machine::new("pwm_zero");
    m.hwmon_chip(0, "thinkpad", &[7000], Some((255, 0)), &[]);

    m.run(|| {
        let r = hwmon::read_all();
        let channel = r
            .channels
            .iter()
            .find(|c| c.chip == "thinkpad")
            .expect("channel present");
        assert_eq!(channel.enable, Some(0));
        assert_eq!(channel.pwm, Some(255));
    });
}

// --- Chip discovery -------------------------------------------------------

/// A directory without a `name` file is not an hwmon chip and must be skipped
/// rather than read as one with an empty name.
#[test]
fn directories_without_a_name_file_are_skipped() {
    let m = Machine::new("nameless");
    fs::create_dir_all(m.root.join("sys/class/hwmon/hwmon0")).expect("mkdir");
    fs::write(m.root.join("sys/class/hwmon/hwmon0/fan1_input"), "1234").expect("write");
    m.hwmon_chip(1, "thinkpad", &[2222], None, &[]);

    m.run(|| {
        let r = hwmon::read_all();
        assert_eq!(r.tachometers.len(), 1, "only the chip with a name counts");
        assert_eq!(r.tachometers[0].rpm, 2222);
    });
}

/// Chips are read in a stable order so the UI does not reshuffle between polls.
#[test]
fn chip_order_is_stable_across_reads() {
    let m = Machine::new("order");
    m.hwmon_chip(0, "acpitz", &[], None, &[("zone", 40.0)])
        .hwmon_chip(1, "thinkpad", &[1111, 2222], None, &[])
        .hwmon_chip(2, "nvme", &[], None, &[("Composite", 38.0)]);

    m.run(|| {
        let first: Vec<String> = hwmon::read_all()
            .tachometers
            .iter()
            .map(|t| t.label.clone())
            .collect();
        for _ in 0..5 {
            let again: Vec<String> = hwmon::read_all()
                .tachometers
                .iter()
                .map(|t| t.label.clone())
                .collect();
            assert_eq!(first, again, "reading order must be deterministic");
        }
    });
}

/// The label file is what turns "temp1" into "CPU". Without it the reading is
/// still usable, just less legible — it must not be dropped.
#[test]
fn sensors_without_a_label_still_report() {
    let m = Machine::new("unlabelled");
    m.write("/sys/class/hwmon/hwmon0/name", "unknown_chip")
        .write("/sys/class/hwmon/hwmon0/temp1_input", "44000");

    m.run(|| {
        let r = hwmon::read_all();
        assert_eq!(r.temperatures.len(), 1);
        assert!(
            r.temperatures[0].label.contains("temp1"),
            "expected a fallback label, got {:?}",
            r.temperatures[0].label
        );
    });
}

// --- fan_control=1 missing: the most common broken state ------------------

/// A stock ThinkPad. The fan interface exists, so the app looks supported, but
/// thinkpad_acpi was loaded without fan_control=1 and will refuse every write.
/// The kernel signals this by omitting the `commands:` lines -- the app must key
/// off that rather than reporting a permissions problem the user cannot fix.
#[test]
fn fan_control_disabled_is_visible_in_procfs() {
    let m = Machine::new("no_fan_control");
    m.thinkpad_fan(2600, "auto", false)
        .hwmon_chip(0, "thinkpad", &[2600], Some((120, 2)), &[]);

    m.run(|| {
        let fan = fs::read_to_string(m.root.join("proc/acpi/ibm/fan")).expect("procfs present");
        assert!(
            !fan.lines().any(|l| l.trim_start().starts_with("commands:")),
            "this machine must present as fan_control=0"
        );
        // Reading still works; only writing would be refused.
        let r = hwmon::read_all();
        assert_eq!(thinkpad_fan_count(&r), 1, "fans stay readable regardless");
    });
}

/// The same machine with the parameter set. Same hardware, different capability
/// -- and the ONLY difference visible to the app is those three lines.
#[test]
fn fan_control_enabled_differs_only_by_the_commands_lines() {
    let off = Machine::new("fc_off");
    off.thinkpad_fan(2600, "auto", false);
    let on = Machine::new("fc_on");
    on.thinkpad_fan(2600, "auto", true);

    let read = |m: &Machine| fs::read_to_string(m.root.join("proc/acpi/ibm/fan")).unwrap();
    let (a, b) = (read(&off), read(&on));

    assert!(
        b.starts_with(&a),
        "the enabled form is the disabled form plus commands"
    );
    assert_eq!(
        b.lines()
            .filter(|l| l.trim_start().starts_with("commands:"))
            .count(),
        3
    );
    assert_eq!(
        a.lines()
            .filter(|l| l.trim_start().starts_with("commands:"))
            .count(),
        0
    );
}
