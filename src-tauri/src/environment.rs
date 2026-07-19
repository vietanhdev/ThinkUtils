//! Detects what kind of system we're on so setup guidance can name the right
//! packages, the right commands, and the right obstacles.
//!
//! The app previously gave everyone the same advice ("click Grant Permissions"),
//! which is wrong on at least two common systems: one where the kernel module
//! refuses fan writes regardless of privilege, and one where the polkit rule we
//! install is silently ignored.

use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

/// The first polkit release whose rules engine reads JavaScript `.rules` files.
///
/// Debian and Ubuntu shipped 0.105 with that engine patched out, so the passwordless
/// rule this app installs is read by nothing on those systems. Ubuntu 22.04 is the
/// widest affected release still in support.
const POLKIT_FIRST_JS_RULES_VERSION: (u32, u32) = (0, 106);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PackageManager {
    Apt,
    Dnf,
    Pacman,
    Zypper,
    Unknown,
}

impl PackageManager {
    /// The command that installs the given packages, ready to paste.
    pub fn install_command(&self, packages: &[&str]) -> Option<String> {
        if packages.is_empty() {
            return None;
        }
        let list = packages.join(" ");
        Some(match self {
            PackageManager::Apt => format!("sudo apt install {}", list),
            PackageManager::Dnf => format!("sudo dnf install {}", list),
            PackageManager::Pacman => format!("sudo pacman -S {}", list),
            PackageManager::Zypper => format!("sudo zypper install {}", list),
            PackageManager::Unknown => return None,
        })
    }

    /// Package names differ per distro for the same software.
    pub fn package_name(&self, tool: Tool) -> &'static str {
        match (self, tool) {
            (PackageManager::Apt, Tool::Sensors) => "lm-sensors",
            (_, Tool::Sensors) => "lm_sensors",
            (PackageManager::Apt, Tool::Polkit) => "policykit-1",
            (_, Tool::Polkit) => "polkit",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tool {
    Sensors,
    Polkit,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DistroInfo {
    pub id: String,
    pub id_like: Vec<String>,
    pub version_id: String,
    pub pretty_name: String,
}

/// Parse the subset of /etc/os-release we care about.
///
/// Values may be quoted or bare, and ID_LIKE is a space-separated list. Unknown
/// keys are ignored rather than treated as errors — the file is extensible by design.
pub fn parse_os_release(content: &str) -> DistroInfo {
    let mut info = DistroInfo::default();

    for line in content.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'');

        match key.trim() {
            "ID" => info.id = value.to_string(),
            "ID_LIKE" => {
                info.id_like = value.split_whitespace().map(String::from).collect();
            }
            "VERSION_ID" => info.version_id = value.to_string(),
            "PRETTY_NAME" => info.pretty_name = value.to_string(),
            _ => {}
        }
    }

    info
}

/// Pick a package manager from the distro id, falling back to the ID_LIKE chain.
///
/// ID_LIKE is what makes derivatives work without an explicit entry: Linux Mint
/// reports `ID=linuxmint ID_LIKE="ubuntu debian"`, Pop!_OS reports debian, and
/// Nobara reports fedora.
pub fn detect_package_manager(distro: &DistroInfo) -> PackageManager {
    let from_id = |id: &str| match id {
        "ubuntu" | "debian" | "linuxmint" | "pop" | "elementary" | "zorin" | "raspbian" => {
            Some(PackageManager::Apt)
        }
        "fedora" | "rhel" | "centos" | "rocky" | "almalinux" | "nobara" => {
            Some(PackageManager::Dnf)
        }
        "arch" | "manjaro" | "endeavouros" | "cachyos" | "garuda" => Some(PackageManager::Pacman),
        "opensuse" | "opensuse-tumbleweed" | "opensuse-leap" | "sles" => {
            Some(PackageManager::Zypper)
        }
        _ => None,
    };

    if let Some(pm) = from_id(&distro.id) {
        return pm;
    }
    for like in &distro.id_like {
        if let Some(pm) = from_id(like) {
            return pm;
        }
    }
    PackageManager::Unknown
}

/// Extract a (major, minor) version from `pkaction --version` output.
///
/// The format changed across releases: older builds print `pkaction version 0.105`,
/// newer ones print `pkaction version 127`. A bare number is a post-1.0 release, so
/// it is treated as major with minor 0.
pub fn parse_polkit_version(output: &str) -> Option<(u32, u32)> {
    let token = output
        .split_whitespace()
        .find(|t| t.chars().next().is_some_and(|c| c.is_ascii_digit()))?;

    let mut parts = token.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next().and_then(|m| m.parse().ok()).unwrap_or(0);

    Some((major, minor))
}

/// Whether this polkit reads JavaScript `.rules` files.
///
/// Returning `None` for an unknown version is deliberate: "we could not tell" must
/// not render as "your system is broken".
pub fn polkit_supports_js_rules(version: Option<(u32, u32)>) -> Option<bool> {
    version.map(|v| v >= POLKIT_FIRST_JS_RULES_VERSION)
}

/// How this copy of the app was installed. Determines whether it may modify its
/// own files, and where its helper is expected to live.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InstallChannel {
    /// Distro package. Files are package-owned; the app must not rewrite them.
    System,
    AppImage,
    Flatpak,
    Snap,
    /// Cargo/dev build.
    Development,
}

pub fn detect_install_channel(
    exe_path: &str,
    appimage_env: Option<&str>,
    snap_env: Option<&str>,
    flatpak_marker_exists: bool,
) -> InstallChannel {
    if flatpak_marker_exists {
        return InstallChannel::Flatpak;
    }
    if snap_env.is_some_and(|s| !s.is_empty()) {
        return InstallChannel::Snap;
    }
    if appimage_env.is_some_and(|s| !s.is_empty()) {
        return InstallChannel::AppImage;
    }
    if exe_path.starts_with("/usr/bin/") || exe_path.starts_with("/usr/local/bin/") {
        return InstallChannel::System;
    }
    InstallChannel::Development
}

/// One thing the user has to do, with the exact command for their system.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetupStep {
    pub title: String,
    pub detail: String,
    /// Copy-pasteable, already correct for this distro. None when the step is
    /// performed by the app itself or requires no shell.
    pub command: Option<String>,
    /// True when the app can perform this step itself.
    pub actionable_in_app: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemReport {
    pub distro: DistroInfo,
    pub package_manager: PackageManager,
    pub polkit_version: Option<String>,
    /// None means undetermined, not unsupported.
    pub polkit_supports_js_rules: Option<bool>,
    pub install_channel: InstallChannel,
    pub steps: Vec<SetupStep>,
}

/// Build the ordered list of setup steps for this specific system.
///
/// Ordering is deliberate: a step that cannot succeed until an earlier one is done
/// must come after it. Installing polkit before relying on a polkit rule, enabling
/// the module parameter before granting permissions to write to the fan.
#[allow(clippy::too_many_arguments)]
pub fn build_setup_steps(
    pm: &PackageManager,
    js_rules_supported: Option<bool>,
    channel: &InstallChannel,
    sensors_present: bool,
    pkexec_present: bool,
    fan_control_enabled: bool,
    modprobe_conf_present: bool,
    helper_installed: bool,
) -> Vec<SetupStep> {
    let mut steps = Vec::new();

    if !pkexec_present {
        steps.push(SetupStep {
            title: "Install polkit".to_string(),
            detail: "ThinkUtils uses polkit to make hardware changes without running the whole app as root.".to_string(),
            command: pm.install_command(&[pm.package_name(Tool::Polkit)]),
            actionable_in_app: false,
        });
    }

    if !sensors_present {
        steps.push(SetupStep {
            title: "Install lm-sensors".to_string(),
            detail: "Needed to read temperatures. Fan control works without it, but the fan curve has nothing to follow.".to_string(),
            command: pm.install_command(&[pm.package_name(Tool::Sensors)]),
            actionable_in_app: false,
        });
    }

    if !fan_control_enabled {
        let detail = if modprobe_conf_present {
            "The setting is saved but the running module still has it off. Reboot, or reload the module."
                .to_string()
        } else {
            "The thinkpad_acpi module refuses fan changes unless it was loaded with fan_control=1. This is a kernel module setting, so no amount of granting permissions will change it."
                .to_string()
        };

        steps.push(SetupStep {
            title: "Enable fan control in the kernel module".to_string(),
            detail,
            command: Some(if modprobe_conf_present {
                "sudo modprobe -r thinkpad_acpi && sudo modprobe thinkpad_acpi".to_string()
            } else {
                "echo 'options thinkpad_acpi fan_control=1' | sudo tee /etc/modprobe.d/thinkpad_acpi.conf\nsudo modprobe -r thinkpad_acpi && sudo modprobe thinkpad_acpi".to_string()
            }),
            actionable_in_app: true,
        });
    }

    if !helper_installed && *channel != InstallChannel::System {
        steps.push(SetupStep {
            title: "Install the fan control helper".to_string(),
            detail: "Installs a small root-owned helper that accepts only fan level commands, plus a polkit rule scoped to it.".to_string(),
            command: None,
            actionable_in_app: true,
        });
    }

    // Only warn once the rule would actually be relied upon.
    if js_rules_supported == Some(false) {
        steps.push(SetupStep {
            title: "Expect a password prompt for fan changes".to_string(),
            detail: "This system's polkit is older than 0.106 and ignores JavaScript rules, which is how ThinkUtils grants passwordless fan control. Everything works, but each fan change asks for your password. Upgrading the distribution is the only fix.".to_string(),
            command: None,
            actionable_in_app: false,
        });
    }

    steps
}

fn command_exists(name: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {}", name))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tauri::command]
pub fn get_system_report() -> ApiResponse<SystemReport> {
    let distro = parse_os_release(&fs::read_to_string("/etc/os-release").unwrap_or_default());
    let package_manager = detect_package_manager(&distro);

    let polkit_raw = Command::new("pkaction")
        .arg("--version")
        .output()
        .ok()
        .map(|o| {
            let mut s = String::from_utf8_lossy(&o.stdout).to_string();
            s.push_str(&String::from_utf8_lossy(&o.stderr));
            s
        });
    let polkit_parsed = polkit_raw.as_deref().and_then(parse_polkit_version);

    let exe_path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let install_channel = detect_install_channel(
        &exe_path,
        std::env::var("APPIMAGE").ok().as_deref(),
        std::env::var("SNAP").ok().as_deref(),
        std::path::Path::new("/.flatpak-info").exists(),
    );

    let fan_proc = fs::read_to_string("/proc/acpi/ibm/fan").unwrap_or_default();
    let fan_control_enabled = fan_proc
        .lines()
        .any(|l| l.trim_start().starts_with("commands:"));
    let modprobe_conf_present = fs::read_to_string(crate::fan_control::MODPROBE_CONF_PATH)
        .map(|c| c.contains("fan_control=1"))
        .unwrap_or(false);

    let steps = build_setup_steps(
        &package_manager,
        polkit_supports_js_rules(polkit_parsed),
        &install_channel,
        command_exists("sensors"),
        command_exists("pkexec"),
        fan_control_enabled,
        modprobe_conf_present,
        std::path::Path::new(crate::fan_control::HELPER_PATH).exists(),
    );

    ApiResponse {
        success: true,
        data: Some(SystemReport {
            distro,
            package_manager,
            polkit_version: polkit_parsed.map(|(a, b)| {
                if a == 0 {
                    format!("{}.{}", a, b)
                } else {
                    a.to_string()
                }
            }),
            polkit_supports_js_rules: polkit_supports_js_rules(polkit_parsed),
            install_channel,
            steps,
        }),
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- os-release parsing --

    #[test]
    fn parses_quoted_and_bare_values() {
        let info = parse_os_release(
            "PRETTY_NAME=\"Ubuntu 26.04 LTS\"\nVERSION_ID=\"26.04\"\nID=ubuntu\nID_LIKE=debian\n",
        );
        assert_eq!(info.id, "ubuntu");
        assert_eq!(info.id_like, vec!["debian"]);
        assert_eq!(info.version_id, "26.04");
        assert_eq!(info.pretty_name, "Ubuntu 26.04 LTS");
    }

    /// Real multi-valued example, taken from Rocky 9. Mint is deliberately not
    /// used here: it sets ID_LIKE=ubuntu alone, which the fixture tests confirm.
    #[test]
    fn parses_multi_valued_id_like() {
        let info = parse_os_release("ID=\"rocky\"\nID_LIKE=\"rhel centos fedora\"\n");
        assert_eq!(info.id_like, vec!["rhel", "centos", "fedora"]);
    }

    #[test]
    fn tolerates_missing_and_junk_lines() {
        let info = parse_os_release("# a comment\n\nNOT_A_PAIR\nID=arch\n");
        assert_eq!(info.id, "arch");
        assert!(info.pretty_name.is_empty());
    }

    // -- package manager mapping --

    #[test]
    fn maps_known_distros_directly() {
        for (id, want) in [
            ("ubuntu", PackageManager::Apt),
            ("debian", PackageManager::Apt),
            ("fedora", PackageManager::Dnf),
            ("arch", PackageManager::Pacman),
            ("opensuse", PackageManager::Zypper),
        ] {
            let d = DistroInfo {
                id: id.to_string(),
                ..Default::default()
            };
            assert_eq!(detect_package_manager(&d), want, "for {}", id);
        }
    }

    /// Derivatives are the common case; without the ID_LIKE fallback every Mint,
    /// Pop!_OS and EndeavourOS user would get no install command at all.
    #[test]
    fn falls_back_to_id_like_for_derivatives() {
        let d = DistroInfo {
            id: "some-new-distro".to_string(),
            id_like: vec!["arch".to_string()],
            ..Default::default()
        };
        assert_eq!(detect_package_manager(&d), PackageManager::Pacman);
    }

    #[test]
    fn unknown_distro_yields_no_command_rather_than_a_wrong_one() {
        let d = DistroInfo {
            id: "mystery".to_string(),
            ..Default::default()
        };
        let pm = detect_package_manager(&d);
        assert_eq!(pm, PackageManager::Unknown);
        assert_eq!(pm.install_command(&["lm-sensors"]), None);
    }

    #[test]
    fn uses_distro_correct_package_names() {
        assert_eq!(
            PackageManager::Apt.package_name(Tool::Sensors),
            "lm-sensors"
        );
        assert_eq!(
            PackageManager::Dnf.package_name(Tool::Sensors),
            "lm_sensors"
        );
        assert_eq!(
            PackageManager::Apt.package_name(Tool::Polkit),
            "policykit-1"
        );
        assert_eq!(PackageManager::Pacman.package_name(Tool::Polkit), "polkit");
    }

    // -- polkit version --

    #[test]
    fn parses_both_polkit_version_formats() {
        assert_eq!(parse_polkit_version("pkaction version 127"), Some((127, 0)));
        assert_eq!(
            parse_polkit_version("pkaction version 0.105"),
            Some((0, 105))
        );
        assert_eq!(
            parse_polkit_version("pkaction version 0.116"),
            Some((0, 116))
        );
        assert_eq!(parse_polkit_version("garbage"), None);
    }

    /// The reason this detection exists: Debian and Ubuntu patched the JS rules
    /// engine out of 0.105, so the passwordless rule we install is read by nothing
    /// on Ubuntu 22.04. Reporting success there would be a lie.
    #[test]
    fn identifies_polkit_versions_that_ignore_js_rules() {
        assert_eq!(polkit_supports_js_rules(Some((0, 105))), Some(false));
        assert_eq!(polkit_supports_js_rules(Some((0, 106))), Some(true));
        assert_eq!(polkit_supports_js_rules(Some((0, 116))), Some(true));
        assert_eq!(polkit_supports_js_rules(Some((127, 0))), Some(true));
    }

    /// "Could not determine" must stay distinct from "not supported".
    #[test]
    fn unknown_polkit_version_is_not_reported_as_unsupported() {
        assert_eq!(polkit_supports_js_rules(None), None);
    }

    // -- install channel --

    #[test]
    fn detects_install_channels_in_priority_order() {
        assert_eq!(
            detect_install_channel("/usr/bin/thinkutils", None, None, true),
            InstallChannel::Flatpak
        );
        assert_eq!(
            detect_install_channel("/usr/bin/thinkutils", None, Some("/snap/x"), false),
            InstallChannel::Snap
        );
        assert_eq!(
            detect_install_channel("/tmp/.mount_x/thinkutils", Some("/x.AppImage"), None, false),
            InstallChannel::AppImage
        );
        assert_eq!(
            detect_install_channel("/usr/bin/thinkutils", None, None, false),
            InstallChannel::System
        );
        assert_eq!(
            detect_install_channel("/home/u/proj/target/debug/thinkutils", None, None, false),
            InstallChannel::Development
        );
    }

    #[test]
    fn empty_env_vars_do_not_count_as_set() {
        assert_eq!(
            detect_install_channel("/usr/bin/thinkutils", Some(""), Some(""), false),
            InstallChannel::System
        );
    }

    // -- setup steps --

    fn steps_for_healthy_system() -> Vec<SetupStep> {
        build_setup_steps(
            &PackageManager::Apt,
            Some(true),
            &InstallChannel::System,
            true,
            true,
            true,
            true,
            true,
        )
    }

    #[test]
    fn a_ready_system_needs_no_steps() {
        assert!(steps_for_healthy_system().is_empty());
    }

    #[test]
    fn missing_tools_produce_distro_correct_commands() {
        let steps = build_setup_steps(
            &PackageManager::Dnf,
            Some(true),
            &InstallChannel::System,
            false,
            false,
            true,
            true,
            true,
        );
        let commands: Vec<_> = steps.iter().filter_map(|s| s.command.clone()).collect();
        assert!(commands.iter().any(|c| c == "sudo dnf install polkit"));
        assert!(commands.iter().any(|c| c == "sudo dnf install lm_sensors"));
    }

    /// Once the config is written, telling the user to write it again is noise --
    /// what they actually need is the reload.
    #[test]
    fn fan_control_step_changes_once_the_config_is_written() {
        let before = build_setup_steps(
            &PackageManager::Apt,
            Some(true),
            &InstallChannel::System,
            true,
            true,
            false,
            false,
            true,
        );
        assert!(before[0]
            .command
            .as_ref()
            .unwrap()
            .contains("tee /etc/modprobe.d"));

        let after = build_setup_steps(
            &PackageManager::Apt,
            Some(true),
            &InstallChannel::System,
            true,
            true,
            false,
            true,
            true,
        );
        let cmd = after[0].command.as_ref().unwrap();
        assert!(
            !cmd.contains("tee /etc/modprobe.d"),
            "should not re-write the config"
        );
        assert!(cmd.contains("modprobe -r"));
    }

    #[test]
    fn old_polkit_produces_an_honest_warning() {
        let steps = build_setup_steps(
            &PackageManager::Apt,
            Some(false),
            &InstallChannel::System,
            true,
            true,
            true,
            true,
            true,
        );
        assert_eq!(steps.len(), 1);
        assert!(steps[0].detail.contains("0.106"));
        assert!(
            !steps[0].actionable_in_app,
            "the app cannot fix the distro's polkit"
        );
    }

    /// A packaged install owns the helper; offering to install it would either
    /// fail or overwrite a package-managed file.
    #[test]
    fn packaged_installs_are_not_told_to_install_the_helper() {
        let packaged = build_setup_steps(
            &PackageManager::Apt,
            Some(true),
            &InstallChannel::System,
            true,
            true,
            true,
            true,
            false,
        );
        assert!(!packaged.iter().any(|s| s.title.contains("helper")));

        let appimage = build_setup_steps(
            &PackageManager::Apt,
            Some(true),
            &InstallChannel::AppImage,
            true,
            true,
            true,
            true,
            false,
        );
        assert!(appimage.iter().any(|s| s.title.contains("helper")));
    }

    /// Steps must be ordered so nothing depends on a later one.
    #[test]
    fn polkit_install_precedes_the_helper_step() {
        let steps = build_setup_steps(
            &PackageManager::Apt,
            Some(true),
            &InstallChannel::AppImage,
            true,
            false,
            true,
            true,
            false,
        );
        let polkit = steps
            .iter()
            .position(|s| s.title.contains("polkit"))
            .unwrap();
        let helper = steps
            .iter()
            .position(|s| s.title.contains("helper"))
            .unwrap();
        assert!(polkit < helper);
    }
}
