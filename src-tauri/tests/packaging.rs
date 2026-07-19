//! Keeps the packaging files in lockstep with the Rust source.
//!
//! Three package formats install the fan helper and the polkit rule, and the app
//! looks for them at paths defined in `fan_control::HELPER_CANDIDATES`. If a
//! packaged copy drifts from the constants, the rule grants access to a path the
//! helper is not installed at — and that fails *silently*: polkit denies, the app
//! falls back to a password prompt, and it looks exactly like a permissions
//! problem rather than a packaging bug.
//!
//! Regenerate the derived files with:
//!
//! ```sh
//! cargo run --example gen-packaging -- ../packaging
//! ```

use std::path::PathBuf;
use thinkutils_lib::fan_control::{
    polkit_rule, HELPER_CANDIDATES, HELPER_SCRIPT, POLKIT_RULE_PACKAGED_PATH,
};

fn repo_file(rel: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/..")).join(rel)
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo_file(rel))
        .unwrap_or_else(|e| panic!("missing packaging file {}: {}", rel, e))
}

/// The committed rule must be exactly what the source generates. Regenerating is
/// one command; letting them diverge costs a silent permissions failure.
#[test]
fn committed_polkit_rule_matches_source() {
    assert_eq!(
        read("packaging/polkit/50-thinkutils.rules"),
        polkit_rule(),
        "packaging/polkit/50-thinkutils.rules is stale - regenerate with:\n  \
         cargo run --example gen-packaging -- ../packaging"
    );
}

#[test]
fn committed_helper_matches_source() {
    assert_eq!(
        read("packaging/helper/thinkutils-fan-control"),
        HELPER_SCRIPT,
        "packaging/helper/thinkutils-fan-control is stale - regenerate with:\n  \
         cargo run --example gen-packaging -- ../packaging"
    );
}

/// Each package format has its own convention, and each must install to a path
/// the app actually searches. A package installing to an unsearched path
/// produces a working install whose fan control silently never works.
#[test]
fn each_package_installs_the_helper_where_the_app_looks() {
    let deb_arch_path = HELPER_CANDIDATES[0]; // /usr/lib/thinkutils/...
    let fedora_path = HELPER_CANDIDATES[1]; // /usr/libexec/thinkutils/...

    let pkgbuild = read("packaging/aur/PKGBUILD");
    assert!(
        pkgbuild.contains(deb_arch_path.trim_start_matches('/')),
        "PKGBUILD must install the helper to {}",
        deb_arch_path
    );

    let spec = read("packaging/copr/thinkutils.spec");
    assert!(
        spec.contains("%{_libexecdir}/thinkutils/thinkutils-fan-control"),
        "spec must install the helper to {} via %{{_libexecdir}}",
        fedora_path
    );
}

/// Distro policy forbids packages writing to /usr/local. A package that does
/// fails lintian/rpmlint and would be rejected outright.
#[test]
fn no_package_writes_to_usr_local() {
    for f in ["packaging/aur/PKGBUILD", "packaging/copr/thinkutils.spec"] {
        let content = read(f);
        for line in content.lines() {
            // Comments explain *why* /usr/local is avoided, so only real
            // install directives count.
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                continue;
            }
            assert!(
                !line.contains("/usr/local"),
                "{} installs to /usr/local, which distro policy forbids:\n  {}",
                f,
                line
            );
        }
    }
}

/// Vendor rules go under /usr/share; /etc belongs to the administrator. A
/// package shipping to /etc creates a conffile it can never cleanly remove.
#[test]
fn packages_ship_the_polkit_rule_under_usr_share() {
    let expected_dir = POLKIT_RULE_PACKAGED_PATH
        .rsplit_once('/')
        .expect("packaged rule path has a directory")
        .0;

    for f in ["packaging/aur/PKGBUILD", "packaging/copr/thinkutils.spec"] {
        let content = read(f);
        assert!(
            content.contains("polkit-1/rules.d"),
            "{} does not install a polkit rule",
            f
        );
        assert!(
            content.contains(expected_dir.trim_start_matches('/'))
                || content.contains("%{_datadir}/polkit-1/rules.d"),
            "{} must install the polkit rule under {}",
            f,
            expected_dir
        );
        assert!(
            !content.contains("/etc/polkit-1/rules.d"),
            "{} must not write into the administrator's /etc namespace",
            f
        );
    }
}

/// ThinkPads are x86_64 and thinkpad_acpi is an x86 platform driver. Claiming an
/// architecture with no hardware to run on produces builds nobody can use.
#[test]
fn packages_claim_x86_64_only() {
    assert!(read("packaging/aur/PKGBUILD").contains("arch=('x86_64')"));
    assert!(read("packaging/copr/thinkutils.spec").contains("ExclusiveArch:  x86_64"));
}

/// The version appears in the PKGBUILD and the spec as well as the four files
/// CLAUDE.md lists, so bump-version.sh has more to keep in step than it did.
#[test]
fn packaging_versions_match_the_manifest() {
    let pkg_json = read("package.json");
    let version = pkg_json
        .lines()
        .find_map(|l| {
            let l = l.trim();
            l.strip_prefix("\"version\":")
                .map(|v| v.trim().trim_matches(|c| c == '"' || c == ',').to_string())
        })
        .expect("package.json has a version");

    assert!(
        read("packaging/aur/PKGBUILD").contains(&format!("pkgver={}", version)),
        "PKGBUILD pkgver does not match package.json ({})",
        version
    );
    assert!(
        read("packaging/copr/thinkutils.spec").contains(&format!("Version:        {}", version)),
        "spec Version does not match package.json ({})",
        version
    );
}

/// The app enables `withGlobalTauri`, so any injected script reaches the full
/// `__TAURI__` API -- including commands that end in `pkexec`. A null CSP made
/// an XSS in a view (process names from `ps aux` are rendered) into a path to
/// root. Both halves are fixed; this guards the CSP half.
#[test]
fn csp_is_set_and_restrictive() {
    let conf = read("src-tauri/tauri.conf.json");
    let parsed: serde_json::Value = serde_json::from_str(&conf).expect("tauri.conf.json parses");
    let csp = parsed["app"]["security"]["csp"]
        .as_str()
        .expect("csp must be a string, not null");

    for required in [
        "default-src 'self'",
        "script-src 'self'",
        "object-src 'none'",
        "frame-ancestors 'none'",
    ] {
        assert!(csp.contains(required), "CSP is missing {}", required);
    }

    // 'unsafe-inline' on script-src would defeat the entire point; templates do
    // use inline style attributes, so style-src legitimately needs it.
    let script_src = csp
        .split(';')
        .find(|d| d.trim().starts_with("script-src"))
        .expect("script-src directive present");
    assert!(
        !script_src.contains("unsafe-inline") && !script_src.contains("unsafe-eval"),
        "script-src must not allow unsafe-inline or unsafe-eval: {}",
        script_src
    );
}

/// escapeHtml lived privately in security.js, so every other view rendering
/// untrusted strings had no escaping at all. It belongs in utils.js, and the
/// views that render process names, mount points and device labels must use it.
#[test]
fn views_escape_untrusted_strings() {
    assert!(
        read("src/js/utils.js").contains("export function escapeHtml"),
        "escapeHtml must be shared from utils.js, not private to one view"
    );

    for view in ["monitor", "battery", "fan", "security"] {
        let src = read(&format!("src/js/views/{}.js", view));
        assert!(
            src.contains("escapeHtml"),
            "{}.js renders external strings but does not escape them",
            view
        );
    }

    // The specific reachable case: `ps aux` output is attacker-controllable by
    // any local user, who can name a binary `<img src=x onerror=...>`.
    let monitor = read("src/js/views/monitor.js");
    assert!(
        monitor.contains("escapeHtml(proc.name)"),
        "process names from `ps aux` must be escaped"
    );
}
