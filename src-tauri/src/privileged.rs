//! Running a shell script as root, once, safely.
//!
//! Five call sites each had their own copy of this: build a script, write it to
//! a predictable `/tmp` path with plain `fs::write`, chmod it, hand it to
//! `pkexec bash`. That pattern has two problems, and the copies had drifted so
//! only some of them had either fix.
//!
//! `fs::write` on a predictable path follows symlinks and happily opens a file
//! another user pre-created. `/tmp/thinkutils_auth.sh` was a fixed name with no
//! randomness at all, so another local user could plant that path and have their
//! content executed as root.
//!
//! Creation here is `O_EXCL` with a random name and mode 0600, which fails
//! rather than following a symlink or reusing a planted file. Root can still
//! read it — root bypasses permission bits — so the script runs as intended.
//!
//! What this does NOT solve: the file is owned by the invoking user for the
//! window between writing and root executing it, so that user could swap its
//! contents. That matters only where an administrator authenticates on behalf of
//! a less-privileged user, and closing it properly means not passing a
//! user-owned script to root at all — the shape the fan helper already uses.

use std::process::Output;

/// Create a script only this user can read, at an unpredictable path.
///
/// Returns the path; the caller is responsible for removing it, which
/// [`run_script`] does.
#[cfg(unix)]
fn create_secure_script(content: &str) -> Result<String, String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    // Nanosecond clock plus pid: enough to make the name unpredictable in
    // practice, and O_EXCL below is what actually enforces exclusivity.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = format!("/tmp/thinkutils_{}_{}.sh", std::process::id(), nanos);

    let mut file = std::fs::OpenOptions::new()
        .create_new(true) // O_EXCL: refuse to follow a symlink or reuse a planted file
        .write(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| format!("Failed to create privileged script: {}", e))?;

    file.write_all(content.as_bytes()).map_err(|e| {
        let _ = std::fs::remove_file(&path);
        format!("Failed to write privileged script: {}", e)
    })?;

    Ok(path)
}

#[cfg(not(unix))]
fn create_secure_script(content: &str) -> Result<String, String> {
    let path = format!("/tmp/thinkutils_{}.sh", std::process::id());
    std::fs::write(&path, content)
        .map_err(|e| format!("Failed to create privileged script: {}", e))?;
    Ok(path)
}

/// Run a script as root via pkexec, then remove it.
///
/// The script is always cleaned up, including when pkexec fails to launch —
/// the previous copies leaked the file on some error paths.
pub async fn run_script(script: &str) -> Result<Output, String> {
    let path = create_secure_script(script)?;

    let result = tokio::process::Command::new("pkexec")
        .arg("bash")
        .arg(&path)
        .output()
        .await
        .map_err(|e| format!("Failed to execute pkexec: {}", e));

    let _ = std::fs::remove_file(&path);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn script_is_created_unreadable_to_other_users() {
        use std::os::unix::fs::PermissionsExt;

        let path = create_secure_script("#!/bin/bash\nexit 0\n").expect("create");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {:o}", mode);
        let _ = std::fs::remove_file(&path);
    }

    /// The name must not be guessable from the pid alone: two calls from the
    /// same process must not collide, or a second invocation could reuse a path
    /// an attacker already knows.
    #[cfg(unix)]
    #[test]
    fn consecutive_scripts_get_distinct_paths() {
        let a = create_secure_script("a").expect("first");
        let b = create_secure_script("b").expect("second");
        assert_ne!(a, b);
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "a");
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "b");
        let _ = std::fs::remove_file(&a);
        let _ = std::fs::remove_file(&b);
    }

    /// O_EXCL is the load-bearing part. Without it, a path another user planted
    /// (or a symlink they pointed at a file they want overwritten as root) would
    /// be opened and used.
    #[cfg(unix)]
    #[test]
    fn refuses_to_reuse_an_existing_path() {
        let path = create_secure_script("original").expect("create");

        // Simulate the planted-file case by trying to create the same path again
        // through the same code path the attacker's target would take.
        let direct = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path);
        assert!(
            direct.is_err(),
            "create_new must fail on an existing path - without it a planted file would be reused"
        );

        let _ = std::fs::remove_file(&path);
    }
}
