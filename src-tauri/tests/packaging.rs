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

/// Lines that actually do something, with comments stripped.
///
/// These files carry long comments explaining the constraints they encode, and
/// a naive substring search matches the explanation as readily as a violation.
fn directives(content: &str) -> impl Iterator<Item = &str> {
    content
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
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
    for f in [
        "packaging/aur/PKGBUILD",
        "packaging/copr/thinkutils.spec",
        "packaging/ppa/debian/rules",
    ] {
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

    for f in [
        "packaging/aur/PKGBUILD",
        "packaging/copr/thinkutils.spec",
        "packaging/ppa/debian/rules",
    ] {
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

/// Every manifest must claim exactly the architectures release.yml actually
/// builds.
///
/// The release matrix is the single source of truth, because it is the thing
/// that produces artifacts. A manifest claiming an arch the matrix does not
/// build commits a package repository to something that will never arrive; a
/// manifest omitting one silently drops half the users a release was built for.
/// Neither fails anything on its own -- the packages simply do not exist, and
/// the download page keeps advertising them.
#[test]
fn packaging_architectures_match_what_release_actually_builds() {
    let release = read(".github/workflows/release.yml");

    // Read the arches out of the matrix rather than restating them here, so this
    // cannot pass while disagreeing with the workflow it is meant to track.
    let built: Vec<&str> = ["x86_64", "aarch64"]
        .into_iter()
        .filter(|a| release.contains(&format!("arch: {},", a)))
        .collect();
    assert_eq!(
        built.len(),
        2,
        "expected release.yml to build x86_64 and aarch64, found {:?}",
        built
    );

    let pkgbuild = read("packaging/aur/PKGBUILD");
    for arch in &built {
        assert!(
            pkgbuild.contains(&format!("'{}'", arch)),
            "PKGBUILD arch=() omits {}, which release.yml builds",
            arch
        );
    }

    let spec = read("packaging/copr/thinkutils.spec");
    let exclusive = spec
        .lines()
        .find(|l| l.starts_with("ExclusiveArch:"))
        .expect("spec declares ExclusiveArch");
    for arch in &built {
        assert!(
            exclusive.contains(arch),
            "spec ExclusiveArch omits {}: {}",
            arch,
            exclusive
        );
    }
}

/// The PPA is multi-arch without naming an architecture: Launchpad builds
/// whatever the series enables. So assert the MECHANISM, not a list — narrowing
/// this to `amd64` would silently stop producing arm64 builds with nothing else
/// in the repo changing, and Launchpad reports per-arch results by email rather
/// than failing anything here.
#[test]
fn debian_control_delegates_architecture_to_launchpad() {
    let control = read("packaging/ppa/debian/control.in");
    let arch_line = directives(&control)
        .find(|l| l.starts_with("Architecture:"))
        .expect("control.in declares an Architecture");

    assert_eq!(
        arch_line.trim(),
        "Architecture: any",
        "the PPA must delegate architecture to the series"
    );
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

/// The Debian package must install the helper where the app searches, exactly
/// like the other two formats.
#[test]
fn debian_rules_installs_the_helper_where_the_app_looks() {
    let rules = read("packaging/ppa/debian/rules");
    assert!(
        rules.contains(HELPER_CANDIDATES[0].trim_start_matches('/')),
        "debian/rules must install the helper to {}",
        HELPER_CANDIDATES[0]
    );
}

/// Launchpad builders have no network. If debian/rules ever stops passing
/// --offline, or CARGO_NET_OFFLINE is dropped, the build reaches for crates.io
/// and fails on the builder while still succeeding locally.
#[test]
fn debian_rules_builds_offline() {
    let rules = read("packaging/ppa/debian/rules");
    assert!(rules.contains("CARGO_NET_OFFLINE = true"));
    assert!(rules.contains("--offline"));
    assert!(
        rules.contains("--locked"),
        "--locked keeps the builder from silently resolving a different tree"
    );
}

/// dh_clean deletes every *.orig file as patch cruft, which removes cargo's
/// vendored Cargo.toml.orig files that .cargo-checksum.json references. The
/// offline build then fails with a checksum error -- on Launchpad only, since a
/// local build never runs dh_clean. This override is easy to drop and expensive
/// to rediscover.
#[test]
fn debian_rules_protects_vendored_orig_files() {
    let rules = read("packaging/ppa/debian/rules");
    assert!(
        rules.contains("dh_clean -X.orig"),
        "debian/rules must keep dh_clean from deleting vendored *.orig files"
    );
}

/// `tauri build` downloads linuxdeploy and AppImage runtimes at build time,
/// which cannot work on a network-less builder.
#[test]
fn debian_rules_does_not_invoke_the_tauri_bundler() {
    // Comments explain WHY the bundler is avoided, so only real recipe lines
    // count -- an earlier version of this test failed on its own rationale.
    for line in directives(&read("packaging/ppa/debian/rules")) {
        assert!(
            !line.contains("tauri build") && !line.contains("npm run tauri"),
            "debian/rules must build with plain cargo, not the Tauri bundler:\n  {}",
            line
        );
    }
}

/// The control template must stay a template: a literal Build-Depends here
/// would silently ship noble the wrong toolchain.
#[test]
fn debian_control_is_templated_for_per_series_rust() {
    let control = read("packaging/ppa/debian/control.in");
    assert!(control.contains("@RUST_BUILD_DEPS@"));
    for line in directives(&control) {
        assert!(
            !line.contains("librust-"),
            "dependencies are vendored, so no librust-* packages should be required:\n  {}",
            line
        );
        assert!(
            !line.contains("npm") && !line.contains("nodejs"),
            "npm is not a build dependency; adding one would require vendoring node_modules too:\n  {}",
            line
        );
    }
}

/// The permission dialog's action row must live OUTSIDE the scrolling body.
///
/// `.dialog-content` scrolls, and while the buttons were inside it the primary
/// action sat below the fold at the app's own 700px minimum window height, with
/// nothing indicating it was there. A first-run user could not find "Setup
/// Permissions" without discovering they had to scroll a dialog.
#[test]
fn dialog_actions_are_not_inside_the_scrolling_body() {
    let html = read("src/templates/dialogs.html");

    let actions = html
        .find(r#"class="dialog-actions""#)
        .expect("permission dialog has an action row");
    let content_close = html[..actions]
        .rfind("</div>")
        .expect("something closes before the actions");

    // The action row must come after .dialog-content closes. Comparing indices
    // is enough because the row is the last element in the container.
    assert!(
        content_close < actions,
        "the action row must follow the scrolling content, not sit inside it"
    );

    assert!(
        html.contains(r#"id="setup-permissions""#),
        "the primary action must still exist"
    );
}

/// The pinned row only stays pinned if the container is a flex column. As a
/// plain block, the row is pushed past max-height rather than held in view --
/// which looks identical in a wide window and breaks in a short one.
#[test]
fn the_dialog_container_is_a_flex_column() {
    let css = read("src/styles/dialogs.css");
    let start = css
        .find(".dialog-container {")
        .expect(".dialog-container is styled");
    let block = &css[start..start + css[start..].find('}').expect("rule closes")];

    assert!(block.contains("display: flex"), "container must be flex");
    assert!(
        block.contains("flex-direction: column"),
        "container must stack header, content and actions vertically"
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
