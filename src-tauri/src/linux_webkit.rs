use std::ffi::OsStr;

const DMABUF_RENDERER_ENV: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";
const SKIP_WORKAROUND_ENV: &str = "THINKUTILS_SKIP_WAYLAND_WORKAROUND";

pub fn apply_wayland_renderer_workaround() {
    if !should_apply_workaround(
        std::env::var_os("XDG_SESSION_TYPE").as_deref(),
        std::env::var_os("WAYLAND_DISPLAY").as_deref(),
        std::env::var_os(DMABUF_RENDERER_ENV).as_deref(),
        std::env::var_os(SKIP_WORKAROUND_ENV).as_deref(),
    ) {
        return;
    }

    // This must run before Tauri initializes WebKitGTK. It selects WebKit's
    // fallback renderer when the DMA-BUF/EGL path is unavailable on Wayland.
    std::env::set_var(DMABUF_RENDERER_ENV, "1");
    eprintln!("[thinkutils] Wayland detected; enabled WebKitGTK EGL compatibility mode");
}

fn should_apply_workaround(
    session_type: Option<&OsStr>,
    wayland_display: Option<&OsStr>,
    renderer_override: Option<&OsStr>,
    skip_workaround: Option<&OsStr>,
) -> bool {
    let is_wayland = session_type
        .and_then(OsStr::to_str)
        .is_some_and(|value| value.eq_ignore_ascii_case("wayland"))
        || wayland_display.is_some();

    is_wayland && renderer_override.is_none() && skip_workaround.is_none()
}

#[cfg(test)]
mod tests {
    use super::should_apply_workaround;
    use std::ffi::OsStr;

    #[test]
    fn applies_to_wayland_sessions_without_an_override() {
        assert!(should_apply_workaround(
            Some(OsStr::new("wayland")),
            None,
            None,
            None,
        ));
        assert!(should_apply_workaround(
            None,
            Some(OsStr::new("wayland-0")),
            None,
            None,
        ));
    }

    #[test]
    fn leaves_x11_and_user_choices_unchanged() {
        assert!(!should_apply_workaround(
            Some(OsStr::new("x11")),
            None,
            None,
            None,
        ));
        assert!(!should_apply_workaround(
            Some(OsStr::new("wayland")),
            None,
            Some(OsStr::new("0")),
            None,
        ));
        assert!(!should_apply_workaround(
            Some(OsStr::new("wayland")),
            None,
            None,
            Some(OsStr::new("1")),
        ));
    }
}
