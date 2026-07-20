// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
mod linux_webkit;

fn main() {
    #[cfg(target_os = "linux")]
    linux_webkit::apply_wayland_renderer_workaround();

    thinkutils_lib::run()
}
