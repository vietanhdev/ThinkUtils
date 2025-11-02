mod fan_control;
mod sync;
mod system_info;
mod battery;
mod performance;
mod monitor;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, AppHandle,
};

#[tauri::command]
async fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(&url, None::<&str>)
        .map_err(|e| format!("Failed to open URL: {}", e))
}

#[tauri::command]
async fn minimize_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.minimize().map_err(|e| format!("Failed to minimize: {}", e))
    } else {
        Err("Window not found".to_string())
    }
}

#[tauri::command]
async fn toggle_maximize(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let is_maximized = window.is_maximized().map_err(|e| format!("Failed to check maximize state: {}", e))?;
        if is_maximized {
            window.unmaximize().map_err(|e| format!("Failed to unmaximize: {}", e))
        } else {
            window.maximize().map_err(|e| format!("Failed to maximize: {}", e))
        }
    } else {
        Err("Window not found".to_string())
    }
}

#[tauri::command]
async fn close_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.close().map_err(|e| format!("Failed to close: {}", e))
    } else {
        Err("Window not found".to_string())
    }
}

#[tauri::command]
async fn start_drag(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.start_dragging().map_err(|e| format!("Failed to start drag: {}", e))
    } else {
        Err("Window not found".to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
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

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Utilities
            open_url,
            minimize_window,
            toggle_maximize,
            close_window,
            start_drag,
            // Fan Control
            fan_control::get_sensor_data,
            fan_control::set_fan_speed,
            fan_control::check_permissions,
            fan_control::update_permissions,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
