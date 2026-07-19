# NETWORK: %build fetches crates from crates.io, which requires the COPR
# *project* to have networking enabled:
#     copr-cli modify thinkutils --enable-net on
# mock disables builder networking by default. Without it every build dies with
# "Could not resolve host: index.crates.io". This is a property of the project,
# not of this spec, so it does not travel with the repository.
Name:           thinkutils
Version:        0.1.10
Release:        1%{?dist}
Summary:        ThinkPad fan control, battery care and system monitoring

License:        LGPL-3.0-or-later
URL:            https://github.com/vietanhdev/ThinkUtils
Source0:        %{url}/archive/refs/tags/v%{version}.tar.gz#/ThinkUtils-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc
BuildRequires:  pkgconfig(webkit2gtk-4.1)
BuildRequires:  pkgconfig(gtk+-3.0)
BuildRequires:  pkgconfig(ayatana-appindicator3-0.1)
BuildRequires:  pkgconfig(librsvg-2.0)
BuildRequires:  pkgconfig(openssl)
BuildRequires:  desktop-file-utils

Requires:       webkit2gtk4.1
Requires:       polkit

Recommends:     lm_sensors

# thinkpad_acpi is an x86 platform driver; there is no ThinkPad to run this on
# anywhere else.
ExclusiveArch:  x86_64

%description
ThinkUtils is a desktop utility for Lenovo ThinkPad laptops running Linux,
providing manual and temperature-curve fan control via thinkpad_acpi, battery
charge threshold management, CPU governor and turbo-boost control, and live
system monitoring.

Privileged operations go through a dedicated, package-owned helper authorised by
a narrow polkit rule; the application itself runs unprivileged.

%prep
%autosetup -n ThinkUtils-%{version}

%build
# Plain cargo, not `tauri build`: the bundler downloads linuxdeploy and emits
# .deb/.AppImage artifacts that are meaningless inside an RPM build.
cd src-tauri && cargo build --release

%install
install -Dpm0755 src-tauri/target/release/thinkutils %{buildroot}%{_bindir}/thinkutils

# %{_libexecdir} is the canonical Fedora home for an internal helper that must
# not be on $PATH. NEVER /usr/local -- forbidden by the packaging guidelines.
# Matches HELPER_CANDIDATES[1] in src-tauri/src/fan_control.rs.
install -Dpm0755 packaging/helper/thinkutils-fan-control \
    %{buildroot}%{_libexecdir}/thinkutils/thinkutils-fan-control

install -Dpm0644 packaging/polkit/50-thinkutils.rules \
    %{buildroot}%{_datadir}/polkit-1/rules.d/50-thinkutils.rules

desktop-file-install --dir=%{buildroot}%{_datadir}/applications thinkutils.desktop
install -Dpm0644 src-tauri/icons/128x128.png \
    %{buildroot}%{_datadir}/icons/hicolor/128x128/apps/thinkutils.png

%files
%license LICENSE
%doc README.md
%{_bindir}/thinkutils
%dir %{_libexecdir}/thinkutils
%{_libexecdir}/thinkutils/thinkutils-fan-control
%{_datadir}/polkit-1/rules.d/50-thinkutils.rules
%{_datadir}/applications/thinkutils.desktop
%{_datadir}/icons/hicolor/128x128/apps/thinkutils.png

%changelog
* Sun Jul 19 2026 Viet Anh Nguyen <vietanh.dev@gmail.com> - 0.1.10-1
- Initial COPR package.
