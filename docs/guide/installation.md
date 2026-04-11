# Installation

## Ubuntu/Debian (APT Repository)

The easiest way to install and keep ThinkUtils updated:

```bash
echo "deb [trusted=yes] https://gh.vietanh.dev/ThinkUtils/apt ./" | sudo tee /etc/apt/sources.list.d/thinkutils.list
sudo apt update
sudo apt install thinkutils
```

## Manual Download

Download the latest `.deb`, `.rpm`, or `.AppImage` from [GitHub Releases](https://github.com/vietanhdev/ThinkUtils/releases).

::: code-group
```bash [Debian/Ubuntu]
sudo dpkg -i thinkutils_*.deb
```

```bash [Fedora/RHEL]
sudo rpm -i thinkutils-*.rpm
```

```bash [AppImage]
chmod +x ThinkUtils_*.AppImage
./ThinkUtils_*.AppImage
```
:::

## Build from Source

### Prerequisites

- Rust 1.70+
- Node.js and npm

::: code-group
```bash [Debian/Ubuntu]
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
    libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

```bash [Fedora]
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file \
    libappindicator-gtk3-devel librsvg2-devel
```
:::

### Build

```bash
git clone https://github.com/vietanhdev/ThinkUtils.git
cd ThinkUtils
npm install
npm run tauri build
```

Built packages are output to `src-tauri/target/release/bundle/`.

Or build and install in one step:

```bash
./scripts/upgrade.sh
```

## Compatibility

ThinkUtils is designed for IBM/Lenovo ThinkPad laptops running Linux.

**Tested on:**
- ThinkPad T480, T490, T14
- ThinkPad X1 Carbon (various generations)
- ThinkPad P-series workstations

**Requirements:**
- Linux kernel with `thinkpad_acpi` module
- `/proc/acpi/ibm/fan` interface available
