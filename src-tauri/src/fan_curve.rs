use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;
use tokio::time::sleep;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurvePoint {
    pub temp: i32,
    pub level: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanCurveConfig {
    pub enabled: bool,
    pub points: Vec<CurvePoint>,
}

impl Default for FanCurveConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            points: vec![
                CurvePoint { temp: 40, level: 0 },
                CurvePoint { temp: 50, level: 1 },
                CurvePoint { temp: 60, level: 3 },
                CurvePoint { temp: 70, level: 5 },
                CurvePoint { temp: 80, level: 7 },
            ],
        }
    }
}

pub type FanCurveState = Arc<Mutex<FanCurveConfig>>;

const STORE_FILE: &str = "settings.json";
const CURVE_KEY: &str = "fan_curve";

/// Save fan curve config to persistent storage
fn save_config_to_store(app: &AppHandle, config: &FanCurveConfig) -> Result<(), String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("Failed to get store: {}", e))?;

    let config_json =
        serde_json::to_value(config).map_err(|e| format!("Failed to serialize config: {}", e))?;

    store.set(CURVE_KEY, config_json);
    store
        .save()
        .map_err(|e| format!("Failed to save store: {}", e))?;

    println!("[Fan Curve] Configuration saved to store");
    Ok(())
}

/// Load fan curve config from persistent storage
pub fn load_config_from_store(app: &AppHandle) -> FanCurveConfig {
    match app.store(STORE_FILE) {
        Ok(store) => {
            if let Some(config_value) = store.get(CURVE_KEY) {
                match serde_json::from_value::<FanCurveConfig>(config_value.clone()) {
                    Ok(config) => {
                        println!("[Fan Curve] Configuration loaded from store");
                        return config;
                    }
                    Err(e) => {
                        eprintln!("[Fan Curve] Failed to deserialize config: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("[Fan Curve] Failed to get store: {}", e);
        }
    }

    println!("[Fan Curve] Using default configuration");
    FanCurveConfig::default()
}

#[tauri::command]
pub async fn set_fan_curve(
    app: AppHandle,
    state: tauri::State<'_, FanCurveState>,
    points: Vec<CurvePoint>,
) -> Result<(), String> {
    let mut config = state
        .lock()
        .map_err(|e| format!("Failed to lock state: {}", e))?;
    config.points = points.clone();

    // Save to persistent storage
    save_config_to_store(&app, &config)?;

    Ok(())
}

#[tauri::command]
pub async fn get_fan_curve(
    state: tauri::State<'_, FanCurveState>,
) -> Result<FanCurveConfig, String> {
    let config = state
        .lock()
        .map_err(|e| format!("Failed to lock state: {}", e))?;
    Ok(config.clone())
}

#[tauri::command]
pub async fn enable_fan_curve(
    app: AppHandle,
    state: tauri::State<'_, FanCurveState>,
    enabled: bool,
) -> Result<(), String> {
    let mut config = state
        .lock()
        .map_err(|e| format!("Failed to lock state: {}", e))?;
    config.enabled = enabled;

    // Save to persistent storage
    save_config_to_store(&app, &config)?;

    Ok(())
}

/// Calculate fan level based on temperature and curve points
fn calculate_fan_level(temp: i32, points: &[CurvePoint]) -> i32 {
    if points.is_empty() {
        return 0;
    }

    // Sort points by temperature
    let mut sorted_points = points.to_vec();
    sorted_points.sort_by_key(|p| p.temp);

    // If temp is below first point, use first level
    if temp <= sorted_points[0].temp {
        return sorted_points[0].level;
    }

    // If temp is above last point, use last level
    if temp >= sorted_points[sorted_points.len() - 1].temp {
        return sorted_points[sorted_points.len() - 1].level;
    }

    // Find the two points to interpolate between
    for i in 0..sorted_points.len() - 1 {
        if temp >= sorted_points[i].temp && temp <= sorted_points[i + 1].temp {
            // Linear interpolation
            let t = (temp - sorted_points[i].temp) as f64
                / (sorted_points[i + 1].temp - sorted_points[i].temp) as f64;
            let level = sorted_points[i].level as f64
                + t * (sorted_points[i + 1].level - sorted_points[i].level) as f64;
            return level.round() as i32;
        }
    }

    sorted_points[sorted_points.len() - 1].level
}

/// Get CPU temperature from sensors
fn get_cpu_temperature() -> Result<i32, String> {
    use std::fs;
    use std::path::Path;

    // Try to read from common thermal zones
    let thermal_zones = [
        "/sys/class/thermal/thermal_zone0/temp",
        "/sys/class/thermal/thermal_zone1/temp",
        "/sys/class/thermal/thermal_zone2/temp",
    ];

    for zone in &thermal_zones {
        if Path::new(zone).exists() {
            if let Ok(content) = fs::read_to_string(zone) {
                if let Ok(temp_millidegrees) = content.trim().parse::<i32>() {
                    let temp = temp_millidegrees / 1000;
                    // Sanity check: temperature should be between 0 and 120°C
                    if temp > 0 && temp < 120 {
                        return Ok(temp);
                    }
                }
            }
        }
    }

    // Try hwmon sensors
    let hwmon_path = "/sys/class/hwmon";
    if let Ok(entries) = fs::read_dir(hwmon_path) {
        for entry in entries.flatten() {
            let path = entry.path();

            // Look for CPU or package temperature
            for i in 1..10 {
                let temp_label_path = path.join(format!("temp{}_label", i));
                let temp_input_path = path.join(format!("temp{}_input", i));

                if temp_label_path.exists() && temp_input_path.exists() {
                    if let Ok(label) = fs::read_to_string(&temp_label_path) {
                        let label_lower = label.to_lowercase();
                        if label_lower.contains("cpu")
                            || label_lower.contains("package")
                            || label_lower.contains("core")
                        {
                            if let Ok(content) = fs::read_to_string(&temp_input_path) {
                                if let Ok(temp_millidegrees) = content.trim().parse::<i32>() {
                                    let temp = temp_millidegrees / 1000;
                                    if temp > 0 && temp < 120 {
                                        return Ok(temp);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Err("Could not read CPU temperature".to_string())
}

/// How long the firmware watchdog waits before forcing the fan back to auto.
///
/// Shared with the helper script's whitelist, which only accepts this exact
/// value. thinkpad_acpi's watchdog is one-shot and only rearms when it receives
/// a fan command, so it is re-armed on a timer — see [`watchdog_due`] for why
/// re-arming on level change alone was not enough.
use crate::fan_control::FAN_WATCHDOG_SECS;

/// Hand the fan back to firmware control.
///
/// Called whenever this task stops steering the fan for any reason: curve
/// disabled, sensors unreadable, or app shutdown. Modelled on fancontrol, which
/// treats every read failure as a reason to restore rather than to hold the last
/// value — holding is what leaves a fan stopped under load.
async fn restore_fan_to_auto() {
    match write_fan_command("level auto").await {
        Ok(_) => println!("[Fan Curve] Fan returned to automatic control"),
        Err(e) => eprintln!("[Fan Curve] FAILED to restore fan to auto: {}", e),
    }
}

/// Arm the firmware watchdog so the fan reverts to auto if this process dies.
///
/// Best-effort: an install whose helper predates watchdog support will reject
/// the command, and a failure here must not stop the curve from running.
async fn arm_fan_watchdog() {
    let _ = write_fan_command(&format!("watchdog {}", FAN_WATCHDOG_SECS)).await;
}

/// Whether the firmware watchdog is due to be re-armed.
///
/// The watchdog is one-shot: it counts down from the last fan command and hands
/// the fan back to the firmware when it reaches zero. Re-arming only on level
/// change looks right, but a curve sitting at a steady temperature issues no fan
/// commands at all — so the fan silently reverted to automatic control roughly
/// [`FAN_WATCHDOG_SECS`] after the temperature settled, which is the *common*
/// case rather than an edge case. `last_level` still matched the target, so
/// nothing rewrote it and the UI went on reporting the curve as active.
///
/// Re-armed at half the interval so one slow or skipped tick cannot expire it.
fn watchdog_due(since_last_arm: Duration) -> bool {
    since_last_arm >= Duration::from_secs((FAN_WATCHDOG_SECS / 2) as u64)
}

/// Synchronous counterpart to [`restore_fan_to_auto`], for the app exit handler.
///
/// Runs unconditionally on shutdown: once this process is gone nothing is left
/// to manage the fan, so handing it back to the firmware is always the right
/// end state. Writing `level auto` when the fan is already automatic is a no-op.
pub fn restore_fan_to_auto_blocking() {
    use std::fs;
    use std::io::Write;

    if let Ok(mut file) = fs::OpenOptions::new()
        .write(true)
        .open("/proc/acpi/ibm/fan")
    {
        if file.write_all(b"level auto").is_ok() {
            println!("[Fan Curve] Fan returned to automatic control on exit");
            return;
        }
    }

    if let Some(helper) = crate::fan_control::helper_path() {
        match std::process::Command::new("pkexec")
            .arg(helper)
            .arg("level auto")
            .output()
        {
            Ok(o) if o.status.success() => {
                println!("[Fan Curve] Fan returned to automatic control on exit");
            }
            _ => eprintln!("[Fan Curve] Could not restore fan to auto on exit"),
        }
    }
}

/// Set fan speed with pkexec fallback using the dedicated helper script.
/// Only attempts pkexec if the helper and polkit rule are both installed
/// (guaranteeing passwordless operation for the background task).
async fn set_fan_speed_internal(level: i32) -> Result<(), String> {
    let command = if (0..=7).contains(&level) {
        format!("level {}", level)
    } else {
        return Err("Invalid fan level".to_string());
    };

    write_fan_command(&command).await
}

/// Write a single command to /proc/acpi/ibm/fan, elevating via the helper when
/// a direct write is not permitted.
async fn write_fan_command(command: &str) -> Result<(), String> {
    use std::fs;
    use std::io::Write;

    let fan_level_path = "/proc/acpi/ibm/fan";

    // Try direct write first
    if let Ok(mut file) = fs::OpenOptions::new().write(true).open(fan_level_path) {
        if file.write_all(command.as_bytes()).is_ok() {
            return Ok(());
        }
    }

    // Use dedicated helper if installed (polkit rule grants passwordless access).
    // We only check the helper because /etc/polkit-1/rules.d/ is root-only.
    let Some(helper) = crate::fan_control::helper_path() else {
        return Err("No write permission. Grant permissions to enable fan curve mode.".to_string());
    };

    let output = tokio::process::Command::new("pkexec")
        .arg(helper)
        .arg(command)
        .output()
        .await
        .map_err(|e| format!("Failed to execute helper: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        Err("Failed to set fan speed via helper".to_string())
    }
}

/// Background task that monitors temperature and adjusts fan speed
pub async fn fan_curve_background_task(app: AppHandle) {
    let state = app.state::<FanCurveState>();
    let mut last_level: Option<i32> = None;
    let mut last_armed: Option<Instant> = None;
    let mut error_count = 0;
    let mut permission_error_reported = false;
    const MAX_ERRORS: i32 = 5;

    loop {
        // Sleep for 2 seconds between checks
        sleep(Duration::from_secs(2)).await;

        // Check if curve mode is enabled
        let config = match state.lock() {
            Ok(cfg) => cfg.clone(),
            Err(e) => {
                eprintln!("[Fan Curve] Failed to lock state: {}", e);
                continue;
            }
        };

        if !config.enabled {
            // Hand the fan back rather than leaving it wherever the curve last
            // put it. Turning the curve off used to strand the fan at its last
            // manual level indefinitely.
            if last_level.is_some() {
                restore_fan_to_auto().await;
            }
            last_level = None;
            last_armed = None;
            permission_error_reported = false;
            continue;
        }

        // Get current CPU temperature
        let temp = match get_cpu_temperature() {
            Ok(t) => t,
            Err(e) => {
                error_count += 1;
                if error_count <= MAX_ERRORS {
                    eprintln!("[Fan Curve] Failed to read temperature: {}", e);
                }

                // Blind and still steering the fan is the dangerous state: if the
                // curve is holding a low level and load rises, nothing will raise
                // it. Give the fan back to the firmware, which can still see the
                // sensors we cannot.
                if last_level.is_some() {
                    eprintln!("[Fan Curve] Temperature unreadable — returning fan to auto");
                    restore_fan_to_auto().await;
                    last_level = None;
                    last_armed = None;
                    let _ = app.emit_to(
                        "main",
                        "fan-curve-error",
                        serde_json::json!({
                            "error": "Temperature sensors unreadable. Fan returned to automatic control.",
                        }),
                    );
                }
                continue;
            }
        };

        error_count = 0; // Reset error count on success

        // Calculate target fan level
        let target_level = calculate_fan_level(temp, &config.points);

        // Only write to fan hardware when level actually changes
        if last_level != Some(target_level) {
            match set_fan_speed_internal(target_level).await {
                Ok(_) => {
                    println!(
                        "[Fan Curve] Temp: {}°C -> Fan Level: {}",
                        temp, target_level
                    );
                    last_level = Some(target_level);
                    permission_error_reported = false;
                    arm_fan_watchdog().await;
                    last_armed = Some(Instant::now());
                }
                Err(e) => {
                    eprintln!("[Fan Curve] Failed to set fan speed: {}", e);

                    // Notify frontend once about permission issues
                    if !permission_error_reported {
                        permission_error_reported = true;
                        let _ = app.emit_to(
                            "main",
                            "fan-curve-error",
                            serde_json::json!({
                                "error": e,
                            }),
                        );
                    }
                }
            }
        } else if last_level.is_some() && last_armed.is_none_or(|t| watchdog_due(t.elapsed())) {
            // Holding a level still counts as steering the fan, so the watchdog
            // has to be kept alive even though nothing is being changed.
            arm_fan_watchdog().await;
            last_armed = Some(Instant::now());
        }

        // Always emit temperature and current level to frontend for live UI updates.
        //
        // `controlling` says whether that level was actually applied. When every
        // write is failing — no helper installed, /proc not writable — last_level
        // stays None and this reported the level it *wanted*, indistinguishable
        // from one it had set. The fan-curve-error toast fires once and is easy
        // to miss or dismiss, after which the display looked correct forever.
        let controlling = last_level.is_some();
        let display_level = last_level.unwrap_or(target_level);
        if let Err(e) = app.emit_to(
            "main",
            "fan-curve-update",
            serde_json::json!({
                "temperature": temp,
                "fan_level": display_level,
                "controlling": controlling,
            }),
        ) {
            eprintln!("[Fan Curve] Failed to emit event: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The regression this exists for: the curve re-armed the watchdog only when
    /// the level changed, so a machine sitting at a steady temperature issued no
    /// fan commands and the firmware took the fan back after FAN_WATCHDOG_SECS.
    ///
    /// The loop ticks every 2s, so this asserts the re-arm lands with room to
    /// spare rather than on the exact boundary.
    #[test]
    fn watchdog_is_rearmed_well_before_the_firmware_gives_up() {
        let expiry = Duration::from_secs(FAN_WATCHDOG_SECS as u64);

        assert!(
            !watchdog_due(Duration::from_secs(0)),
            "no re-arm needed immediately after arming"
        );

        let due_at = (1..=FAN_WATCHDOG_SECS as u64)
            .map(Duration::from_secs)
            .find(|d| watchdog_due(*d))
            .expect("must become due before the firmware watchdog expires");

        assert!(
            due_at < expiry,
            "re-arm becomes due at {:?} but the firmware gives up at {:?}",
            due_at,
            expiry
        );

        // At least one full 2s tick has to fit between "due" and "expired",
        // otherwise a single slow iteration loses the fan.
        assert!(
            expiry - due_at >= Duration::from_secs(2),
            "only {:?} of slack between due ({:?}) and expiry ({:?}) - one \
             delayed tick would let the watchdog fire",
            expiry - due_at,
            due_at,
            expiry
        );
    }

    /// A level held across many ticks is the case that used to silently stop
    /// steering the fan, so walk the actual loop cadence rather than a single
    /// duration.
    #[test]
    fn holding_one_level_still_keeps_the_watchdog_alive() {
        const TICK: u64 = 2;
        let mut since_arm = Duration::from_secs(0);
        let mut rearms = 0;

        // Five minutes at a dead-steady temperature: no level change, ever.
        for _ in 0..(300 / TICK) {
            since_arm += Duration::from_secs(TICK);
            if watchdog_due(since_arm) {
                rearms += 1;
                since_arm = Duration::from_secs(0);
            }
            assert!(
                since_arm < Duration::from_secs(FAN_WATCHDOG_SECS as u64),
                "watchdog went {:?} without a re-arm - the fan would have \
                 reverted to firmware control",
                since_arm
            );
        }

        assert!(
            rearms >= 9,
            "expected roughly one re-arm per {}s over 5 minutes, got {}",
            FAN_WATCHDOG_SECS / 2,
            rearms
        );
    }

    #[test]
    fn test_calculate_fan_level() {
        let points = vec![
            CurvePoint { temp: 40, level: 0 },
            CurvePoint { temp: 50, level: 1 },
            CurvePoint { temp: 60, level: 3 },
            CurvePoint { temp: 70, level: 5 },
            CurvePoint { temp: 80, level: 7 },
        ];

        // Below first point
        assert_eq!(calculate_fan_level(35, &points), 0);

        // At points
        assert_eq!(calculate_fan_level(40, &points), 0);
        assert_eq!(calculate_fan_level(50, &points), 1);
        assert_eq!(calculate_fan_level(60, &points), 3);

        // Between points (interpolation)
        assert_eq!(calculate_fan_level(45, &points), 1); // Rounded from 0.5
        assert_eq!(calculate_fan_level(55, &points), 2); // Rounded from 2.0

        // Above last point
        assert_eq!(calculate_fan_level(85, &points), 7);
    }
}
