mod auth;
mod battery;
pub mod environment;
pub mod fan_control;
mod fan_curve;
pub mod hardware_root;
mod mcp;
mod monitor;
mod performance;
mod permissions;
mod security;
mod settings;
mod sync;
mod system_info;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

#[tauri::command]
async fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| format!("Failed to open URL: {}", e))
}

#[tauri::command]
async fn minimize_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .minimize()
            .map_err(|e| format!("Failed to minimize: {}", e))
    } else {
        Err("Window not found".to_string())
    }
}

#[tauri::command]
async fn toggle_maximize(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let is_maximized = window
            .is_maximized()
            .map_err(|e| format!("Failed to check maximize state: {}", e))?;
        if is_maximized {
            window
                .unmaximize()
                .map_err(|e| format!("Failed to unmaximize: {}", e))
        } else {
            window
                .maximize()
                .map_err(|e| format!("Failed to maximize: {}", e))
        }
    } else {
        Err("Window not found".to_string())
    }
}

#[tauri::command]
async fn close_window(app: AppHandle) -> Result<(), String> {
    // Hide window instead of closing to keep app running in background
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| format!("Failed to hide: {}", e))
    } else {
        Err("Window not found".to_string())
    }
}

#[tauri::command]
async fn start_drag(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .start_dragging()
            .map_err(|e| format!("Failed to start drag: {}", e))
    } else {
        Err("Window not found".to_string())
    }
}

/// Called by the frontend once every template is injected and every view has
/// initialised.
///
/// Its absence in a log means the JS never finished booting, which no pixel
/// check can prove on its own — a window can be fully painted by a frontend
/// that died halfway through init.
#[tauri::command]
fn report_frontend_ready(templates: usize, views: usize) {
    println!("[thinkutils] frontend ready: templates={templates} views={views}");
}

/// Any uncaught frontend exception or rejection.
///
/// This is the check that catches a view dying on an absent sysfs path: the
/// sidebar still paints, the process still lives, and without this line nothing
/// would ever notice.
#[tauri::command]
fn report_frontend_error(msg: String) {
    // Truncated because an unhandled rejection can carry an entire stack.
    let msg: String = msg.chars().take(500).collect();
    eprintln!("[thinkutils] frontend error: {msg}");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // When a second instance is launched, focus the existing window
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.unminimize();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            // --- Diagnostic instrumentation ---
            //
            // These lines are the contract scripts/test-gui-packages-docker.sh
            // asserts against. In a headless container they are the only evidence
            // that the app started correctly, so do not remove or reword them
            // without updating that script. Format: "[thinkutils] <key>: <value>".
            if let Some(w) = app.get_webview_window("main") {
                match w.url() {
                    // An http(s) URL here means the build points at devUrl, which
                    // renders an empty window on any machine without a dev server.
                    Ok(url) => println!("[thinkutils] webview url: {url}"),
                    Err(e) => eprintln!("[thinkutils] warning: could not read webview url: {e}"),
                }
            }

            // Probe the hardware surfaces this app depends on, and say so. In a
            // container every one is absent, and that is a legitimate supported
            // state -- not a failure. Printing it is what lets a test tell
            // "started fine, no ThinkPad here" from "broken", which are otherwise
            // indistinguishable from the outside.
            let present = |p: &str| {
                if std::path::Path::new(p).exists() {
                    "present"
                } else {
                    "absent"
                }
            };
            let ibm_fan = present("/proc/acpi/ibm/fan");
            let bat0 = present("/sys/class/power_supply/BAT0");
            let cpufreq = present("/sys/devices/system/cpu/cpu0/cpufreq");
            println!("[thinkutils] hw probe: ibm_fan={ibm_fan} bat0={bat0} cpufreq={cpufreq}");

            // Only the thinkpad_acpi fan interface distinguishes a supported
            // machine. A battery and cpufreq exist on every Linux laptop, and a
            // container inherits the host's /sys -- so keying the mode off those
            // reported full ThinkPad support from inside a container that had no
            // fan interface at all. The first run of the launch test caught it.
            println!(
                "[thinkutils] hw mode: {}",
                if ibm_fan == "present" {
                    "full"
                } else {
                    "degraded"
                }
            );

            // Load fan curve config from persistent storage
            let saved_config = fan_curve::load_config_from_store(app.handle());
            let fan_curve_state =
                fan_curve::FanCurveState::new(std::sync::Mutex::new(saved_config));
            app.manage(fan_curve_state);

            // Initialize MCP server state (off by default)
            let mcp_state =
                mcp::McpState::new(tokio::sync::Mutex::new(mcp::McpServerState::default()));
            app.manage(mcp_state);

            // Start fan curve background task
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                fan_curve::fan_curve_background_task(app_handle).await;
            });
            // Create tray menu
            let show = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
            let hide = MenuItem::with_id(app, "hide", "Hide Window", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&show, &hide, &quit])?;

            // Load tray icon - use 32x32 icon for all platforms
            let tray_icon = app.default_window_icon().unwrap().clone();

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .icon(tray_icon)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "hide" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.hide();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // Prevent window from closing, hide it instead
            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        // Prevent the window from closing
                        api.prevent_close();
                        // Hide the window instead
                        let _ = window_clone.hide();
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Utilities
            open_url,
            minimize_window,
            toggle_maximize,
            close_window,
            start_drag,
            // Authentication
            auth::authenticate_once,
            // Permissions
            permissions::check_permissions_status,
            permissions::setup_permissions,
            // Settings
            settings::save_app_settings,
            settings::load_app_settings,
            settings::update_setting,
            // Fan Control
            fan_control::get_sensor_data,
            fan_control::get_fan_capability,
            environment::get_system_report,
            report_frontend_ready,
            report_frontend_error,
            fan_control::enable_fan_control,
            fan_control::set_fan_speed,
            fan_control::check_permissions,
            // Fan Curve
            fan_curve::set_fan_curve,
            fan_curve::get_fan_curve,
            fan_curve::enable_fan_curve,
            // Sync
            sync::get_settings,
            sync::save_settings,
            sync::google_auth_init,
            sync::google_auth_status,
            sync::sync_to_cloud,
            sync::sync_from_cloud,
            sync::google_logout,
            // System Info
            system_info::get_system_info,
            // Battery
            battery::get_battery_info,
            battery::get_battery_thresholds,
            battery::set_battery_thresholds,
            battery::get_power_consumption,
            // Performance
            performance::get_cpu_info,
            performance::set_cpu_governor,
            performance::get_power_profile,
            performance::set_power_profile,
            performance::get_turbo_boost_status,
            performance::set_turbo_boost,
            // Monitor
            monitor::get_system_monitor,
            // Security
            security::get_security_status,
            security::update_virus_definitions,
            security::scan_path,
            security::quick_scan,
            security::install_clamav,
            // MCP
            mcp::get_mcp_status,
            mcp::start_mcp_server,
            mcp::stop_mcp_server,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app, event| {
            // Never exit leaving the fan under manual control. The firmware
            // watchdog is the backstop for a hard kill, but on a clean exit we
            // hand the fan back explicitly and immediately.
            if let tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit = event {
                fan_curve::restore_fan_to_auto_blocking();
            }
        });
}
