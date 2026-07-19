//! Distro detection against real `/etc/os-release` files.
//!
//! The fixtures in `tests/fixtures/os-release/` were captured verbatim from
//! official container images rather than written by hand — see the README there
//! for how, and for the three things the real files corrected.
//!
//! ThinkUtils tells users which packages to install and which commands to run.
//! Getting the distro wrong means handing someone a command that cannot work, so
//! this is checked against genuine data rather than assumptions.

use thinkutils_lib::environment::{detect_package_manager, parse_os_release, PackageManager, Tool};

fn fixture(name: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/os-release/");
    std::fs::read_to_string(format!("{}{}", path, name))
        .unwrap_or_else(|e| panic!("missing fixture {}: {}", name, e))
}

/// Every fixture must resolve to the right package manager. A wrong answer here
/// means telling a Fedora user to run `apt`.
#[test]
fn every_real_distro_resolves_to_the_right_package_manager() {
    let cases = [
        ("ubuntu-24.04", PackageManager::Apt),
        ("ubuntu-22.04", PackageManager::Apt),
        ("debian-12", PackageManager::Apt),
        ("linuxmint-21", PackageManager::Apt),
        ("fedora-41", PackageManager::Dnf),
        ("rocky-9", PackageManager::Dnf),
        ("arch", PackageManager::Pacman),
        ("opensuse-tumbleweed", PackageManager::Zypper),
    ];

    for (name, expected) in cases {
        let distro = parse_os_release(&fixture(name));
        let got = detect_package_manager(&distro);
        assert_eq!(
            got, expected,
            "{}: ID={:?} ID_LIKE={:?} resolved to {:?}, expected {:?}",
            name, distro.id, distro.id_like, got, expected
        );
    }
}

/// No fixture may fall through to Unknown. Unknown produces no install command
/// at all, which leaves the user with nothing actionable.
#[test]
fn no_real_distro_falls_through_to_unknown() {
    for name in [
        "ubuntu-24.04",
        "ubuntu-22.04",
        "debian-12",
        "linuxmint-21",
        "fedora-41",
        "rocky-9",
        "arch",
        "opensuse-tumbleweed",
    ] {
        let distro = parse_os_release(&fixture(name));
        assert_ne!(
            detect_package_manager(&distro),
            PackageManager::Unknown,
            "{} was not recognised (ID={:?}, ID_LIKE={:?})",
            name,
            distro.id,
            distro.id_like
        );
    }
}

/// Values are quoted on some distros and bare on others. A parser that handles
/// only one form silently half-works.
#[test]
fn quoting_style_does_not_affect_parsing() {
    // Bare values.
    let arch = parse_os_release(&fixture("arch"));
    assert_eq!(arch.id, "arch");

    // Quoted values.
    let suse = parse_os_release(&fixture("opensuse-tumbleweed"));
    assert_eq!(suse.id, "opensuse-tumbleweed");
    assert_eq!(suse.id_like, vec!["opensuse", "suse"]);

    let rocky = parse_os_release(&fixture("rocky-9"));
    assert_eq!(rocky.id, "rocky");
    assert_eq!(rocky.id_like, vec!["rhel", "centos", "fedora"]);
}

/// Arch omits ID_LIKE entirely and uses a build date for VERSION_ID. Neither
/// may break detection.
#[test]
fn handles_missing_id_like_and_non_numeric_versions() {
    let arch = parse_os_release(&fixture("arch"));
    assert!(arch.id_like.is_empty(), "arch should have no ID_LIKE");
    assert!(
        !arch.version_id.is_empty(),
        "arch still reports a VERSION_ID"
    );
    assert_eq!(detect_package_manager(&arch), PackageManager::Pacman);
}

/// Mint is not in the direct id table by way of its derivative status — it is
/// resolved through ID_LIKE. Real Mint 21 sets `ID_LIKE=ubuntu` alone, not
/// `"ubuntu debian"`, so a lookup that expects the pair would miss it.
#[test]
fn derivatives_resolve_through_id_like() {
    let mint = parse_os_release(&fixture("linuxmint-21"));
    assert_eq!(mint.id, "linuxmint");
    assert_eq!(mint.id_like, vec!["ubuntu"]);
    assert_eq!(detect_package_manager(&mint), PackageManager::Apt);
}

/// The generated commands are shown to users verbatim, so assert on the exact
/// strings — including the package name differences between distros.
#[test]
fn generated_commands_are_correct_per_distro() {
    let cases = [
        (
            "ubuntu-24.04",
            "sudo apt install lm-sensors",
            "sudo apt install policykit-1",
        ),
        (
            "fedora-41",
            "sudo dnf install lm_sensors",
            "sudo dnf install polkit",
        ),
        ("arch", "sudo pacman -S lm_sensors", "sudo pacman -S polkit"),
        (
            "opensuse-tumbleweed",
            "sudo zypper install lm_sensors",
            "sudo zypper install polkit",
        ),
    ];

    for (name, want_sensors, want_polkit) in cases {
        let pm = detect_package_manager(&parse_os_release(&fixture(name)));
        assert_eq!(
            pm.install_command(&[pm.package_name(Tool::Sensors)])
                .unwrap(),
            want_sensors,
            "sensors command for {}",
            name
        );
        assert_eq!(
            pm.install_command(&[pm.package_name(Tool::Polkit)])
                .unwrap(),
            want_polkit,
            "polkit command for {}",
            name
        );
    }
}
