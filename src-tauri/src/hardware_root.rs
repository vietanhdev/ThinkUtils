//! Resolves the root under which `/proc` and `/sys` are read.
//!
//! Normally that root is `/`. Tests and CI set `THINKUTILS_HARDWARE_ROOT` to a
//! captured hardware profile instead, which is how a container with no ThinkPad
//! can still exercise the dual-fan read path.
//!
//! Only reads are redirected. Writes always target the real paths and are gated
//! by the usual permission checks, so a fixture can never be mistaken for a
//! writable device.

use std::path::{Path, PathBuf};

/// Environment variable naming a captured hardware profile to read instead of
/// the live machine. Set by tests and by the hardware-matrix CI job.
pub const HARDWARE_ROOT_ENV: &str = "THINKUTILS_HARDWARE_ROOT";

/// Resolve an absolute hardware path against the configured root.
///
/// Returns the path unchanged when no root is set, which is the shipped
/// behaviour -- the indirection costs one env lookup and nothing else.
pub fn resolve(path: &str) -> PathBuf {
    match std::env::var(HARDWARE_ROOT_ENV) {
        Ok(root) if !root.is_empty() => {
            // The captured tree mirrors absolute paths verbatim, so joining is a
            // matter of stripping the leading slash.
            Path::new(&root).join(path.trim_start_matches('/'))
        }
        _ => PathBuf::from(path),
    }
}

/// True when reads are being served from a fixture rather than real hardware.
///
/// Callers use this to refuse writes: a test must never believe it changed a fan
/// speed because a fixture file happened to be writable.
pub fn is_simulated() -> bool {
    std::env::var(HARDWARE_ROOT_ENV).is_ok_and(|v| !v.is_empty())
}

/// Read a hardware file through the configured root.
pub fn read_to_string(path: &str) -> std::io::Result<String> {
    std::fs::read_to_string(resolve(path))
}

/// Whether a hardware path exists under the configured root.
pub fn exists(path: &str) -> bool {
    resolve(path).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The env var is process-global, so these run under one lock and restore it
    /// afterwards. Without that, a parallel test seeing another's root would fail
    /// in a way that looks like a logic bug.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_root<T>(root: Option<&str>, f: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let previous = std::env::var(HARDWARE_ROOT_ENV).ok();

        match root {
            Some(r) => std::env::set_var(HARDWARE_ROOT_ENV, r),
            None => std::env::remove_var(HARDWARE_ROOT_ENV),
        }

        let out = f();

        match previous {
            Some(p) => std::env::set_var(HARDWARE_ROOT_ENV, p),
            None => std::env::remove_var(HARDWARE_ROOT_ENV),
        }
        out
    }

    #[test]
    fn unset_root_leaves_paths_untouched() {
        with_root(None, || {
            assert_eq!(
                resolve("/proc/acpi/ibm/fan"),
                PathBuf::from("/proc/acpi/ibm/fan")
            );
            assert!(!is_simulated());
        });
    }

    #[test]
    fn set_root_rebases_absolute_paths() {
        with_root(Some("/fixtures/p1"), || {
            assert_eq!(
                resolve("/proc/acpi/ibm/fan"),
                PathBuf::from("/fixtures/p1/proc/acpi/ibm/fan")
            );
            assert_eq!(
                resolve("/sys/class/hwmon/hwmon6/fan2_input"),
                PathBuf::from("/fixtures/p1/sys/class/hwmon/hwmon6/fan2_input")
            );
            assert!(is_simulated());
        });
    }

    /// An empty value is treated as unset. Otherwise `THINKUTILS_HARDWARE_ROOT=`
    /// in a shell would silently rebase every path onto the filesystem root and
    /// look like it was working.
    #[test]
    fn empty_root_is_treated_as_unset() {
        with_root(Some(""), || {
            assert_eq!(
                resolve("/proc/acpi/ibm/fan"),
                PathBuf::from("/proc/acpi/ibm/fan")
            );
            assert!(!is_simulated());
        });
    }
}
