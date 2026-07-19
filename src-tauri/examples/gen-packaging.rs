//! Emit the packaging artifacts that must stay in lockstep with the Rust source.
//!
//! The polkit rule and the fan helper are installed by three different package
//! formats, and the app searches for them at paths defined in `fan_control`. If a
//! packaged copy drifts from the constants, the rule grants access to a path the
//! helper is not at — which fails silently and looks exactly like a permissions
//! problem.
//!
//! Regenerate with:
//!
//! ```sh
//! cargo run --example gen-packaging -- ../packaging
//! ```
//!
//! `packaging_matches_source` in tests/packaging.rs fails if the committed files
//! and these outputs disagree, so the two cannot diverge unnoticed.

use std::io::Write;

fn main() {
    let out_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "../packaging".to_string());

    let polkit_dir = format!("{}/polkit", out_dir);
    let helper_dir = format!("{}/helper", out_dir);
    std::fs::create_dir_all(&polkit_dir).expect("create polkit dir");
    std::fs::create_dir_all(&helper_dir).expect("create helper dir");

    let rule_path = format!("{}/50-thinkutils.rules", polkit_dir);
    std::fs::write(&rule_path, thinkutils_lib::fan_control::polkit_rule()).expect("write rule");
    println!("wrote {}", rule_path);

    let helper_path = format!("{}/thinkutils-fan-control", helper_dir);
    let mut f = std::fs::File::create(&helper_path).expect("create helper");
    f.write_all(thinkutils_lib::fan_control::HELPER_SCRIPT.as_bytes())
        .expect("write helper");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&helper_path, std::fs::Permissions::from_mode(0o755))
            .expect("chmod helper");
    }
    println!("wrote {}", helper_path);
}
