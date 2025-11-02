# ThinkUtils

A modern desktop application for ThinkPad users on Linux, built with Tauri (Rust + JavaScript).

![ThinkUtils](https://img.shields.io/badge/platform-Linux-blue)
![License](https://img.shields.io/badge/license-LGPL%20v3-green)
![Tauri](https://img.shields.io/badge/Built%20with-Tauri-orange)

## Features

### 🏠 Home Dashboard
- **Power Mode Control**: Switch between Conservation, Balanced, and Performance modes
- **Graphics Mode**: Toggle discrete GPU on/off for hybrid graphics
- **Always-on USB**: Configure USB charging when laptop is off or sleeping
- **Instant Boot**: Enable flip-to-start functionality

### 🌀 Fan Control
- **Real-time Monitoring**: View CPU/GPU temperatures and fan speeds
- **Multiple Control Modes**:
  - **Auto Mode**: System-managed cooling (default)
  - **Manual Mode**: Set custom fan speed levels (0-7)
  - **Maximum Mode**: Full speed cooling for intensive tasks
- **Temperature Sensors**: Monitor all system temperature sensors
- **Permission Management**: Secure elevated access handling

### 🔋 Battery Management
- **Multi-Battery Support**: Monitor all installed batteries
- **Charge Thresholds**: Set start/stop charging limits to extend battery lifespan
- **Battery Health**: View capacity, health status, and charge cycles
- **Real-time Stats**: Current charge level, voltage, and power consumption

### ⚡ Performance Tuning
- **CPU Governor Control**: Switch between performance, powersave, schedutil, and other governors
- **Power Profiles**: System-wide power management (performance, balanced, power-saver)
- **Turbo Boost**: Enable/disable CPU turbo frequencies
- **Frequency Monitoring**: Real-time CPU frequency and range display

### 📊 System Monitor
- **CPU Usage**: Per-core utilization and load averages
- **Memory Stats**: RAM and swap usage with detailed metrics
- **Disk Usage**: Monitor all mounted filesystems
- **Network Activity**: Real-time upload/download speeds per interface
- **Process Monitor**: View top processes by CPU and memory usage

### 💻 System Information
- **Hardware Details**: Model, CPU, memory, and OS information
- **Kernel Version**: Current Linux kernel
- **Hostname**: System identification

### 🔄 Google Drive Sync
- **Settings Backup**: Sync your ThinkUtils configuration to Google Drive
- **Cross-Device**: Access settings from any device
- **OAuth Integration**: Secure Google account authentication

### 🎨 Modern UI
- **Dark Theme**: Sleek dark interface with ThinkPad red accents
- **Custom Titlebar**: Frameless window with native controls
- **Responsive Design**: Clean, organized layout
- **Real-time Updates**: Live data refresh for all metrics

## How it Works

ThinkUtils uses:
- **Rust backend** for system access and fan control
- **sensors command** to read temperature data
- **/proc/acpi/ibm/fan** to control fan speed
- **pkexec** for elevated permissions when needed

## Prerequisites

### Debian / Ubuntu
```bash
sudo apt install lm-sensors policykit-1
```

### Fedora / RHEL
```bash
sudo dnf install lm_sensors polkit
```

### Arch Linux
```bash
sudo pacman -S lm_sensors polkit
```

## Setup ThinkPad Fan Control

Before using ThinkUtils, you need to enable fan control:

1. Create or edit the thinkpad_acpi configuration:
```bash
sudo nano /etc/modprobe.d/thinkpad_acpi.conf
```

2. Add this line:
```
options thinkpad_acpi fan_control=1
```

3. Reboot your system or reload the module:
```bash
sudo modprobe -r thinkpad_acpi
sudo modprobe thinkpad_acpi
```

## Development

### Prerequisites
- Rust (1.70 or later)
- Node.js and npm
- Development dependencies for your distribution

**Debian/Ubuntu:**
```bash
sudo apt install libwebkit2gtk-4.1-dev \
    build-essential \
    curl \
    wget \
    file \
    libssl-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev
```

**Fedora:**
```bash
sudo dnf install webkit2gtk4.1-devel \
    openssl-devel \
    curl \
    wget \
    file \
    libappindicator-gtk3-devel \
    librsvg2-devel
```

### Run in Development Mode

```bash
npm install
npm run tauri dev
```

### Build for Production

```bash
npm run tauri build
```

The built packages will be in `src-tauri/target/release/bundle/`

## Usage

### Running the Application

Launch ThinkUtils from your application menu or run:
```bash
thinkutils
```

### Navigation

Use the left sidebar to navigate between features:
- **Home**: Quick settings and system overview
- **Fan Control**: Manage cooling and temperatures
- **Battery**: Battery health and charge thresholds
- **Performance**: CPU governor and power profiles
- **Monitor**: Real-time system resource monitoring
- **System Info**: Hardware and OS details
- **Sync**: Google Drive settings backup
- **About**: Application information

### Home Dashboard

The home screen provides quick access to essential controls:

1. **CPU/GPU Cards**: Monitor utilization and temperature
2. **Power Mode**: Click buttons to switch between Conservation, Balanced, or Performance
3. **Graphics Mode**: Toggle discrete GPU on/off
4. **Always-on USB**: Enable USB charging when laptop is off or sleeping
5. **Instant Boot**: Enable flip-to-start feature

### Fan Control

Control your ThinkPad's cooling system:

1. **Auto Mode** (Default): System manages fan speed automatically
2. **Manual Mode**: Set specific fan level (0-7)
   - 0: Silent (lowest speed)
   - 7: Maximum speed
3. **Maximum Mode**: Run fan at full speed for intensive cooling

The status panel shows real-time temperature sensors and fan speeds.

### Battery Management

Optimize battery health and longevity:

1. View all installed batteries with current charge levels
2. Set **Start Charging** threshold (when to begin charging)
3. Set **Stop Charging** threshold (when to stop charging)
4. Click **Apply Thresholds** to save settings

**Recommended for longevity**: Start at 40%, Stop at 80%

### Performance Tuning

Optimize CPU performance and power consumption:

1. **CPU Governor**: Select scaling policy (performance, powersave, schedutil, etc.)
2. **Power Profile**: Choose system-wide power mode
3. **Turbo Boost**: Toggle CPU turbo frequencies on/off

Monitor current frequency and min/max ranges in real-time.

### System Monitor

Track system resources:

- **CPU**: Per-core usage and load averages
- **Memory**: RAM and swap utilization
- **Disk**: Usage per mounted filesystem
- **Network**: Real-time bandwidth per interface
- **Processes**: Top processes by CPU/memory

All metrics update automatically.

### Google Drive Sync

Backup and sync your settings:

1. Click **Sign in with Google** (requires OAuth configuration)
2. Authorize ThinkUtils to access Google Drive
3. Use **Sync Now** to backup current settings
4. Use **Download Settings** to restore from cloud

**Note**: Requires Google OAuth credentials configured in source code.

### Permissions

ThinkUtils may request elevated permissions for:
- Fan control access
- Battery threshold configuration
- CPU governor changes
- Power profile management

All requests are handled securely using `pkexec`.

## Compatibility

ThinkUtils is designed for IBM/Lenovo ThinkPad laptops running Linux.

**Tested on:**
- ThinkPad T480, T490, T14
- ThinkPad X1 Carbon (various generations)
- ThinkPad P-series workstations

**Requirements:**
- Linux kernel with thinkpad_acpi module
- /proc/acpi/ibm/fan interface available

## Troubleshooting

### Fan control not working
1. Verify thinkpad_acpi is loaded:
   ```bash
   lsmod | grep thinkpad_acpi
   ```
2. Check if fan control is enabled:
   ```bash
   cat /etc/modprobe.d/thinkpad_acpi.conf
   ```
3. Verify the file exists:
   ```bash
   ls -l /proc/acpi/ibm/fan
   ```

### Permission errors
ThinkUtils will automatically request elevated permissions. If issues persist:
```bash
sudo chmod 666 /proc/acpi/ibm/fan
```

Note: This needs to be done after each reboot unless you set up udev rules.

### No temperature data
Ensure lm-sensors is installed and configured:
```bash
sudo sensors-detect
sensors
```

## Project Structure

```
thinkutils/
├── src/                    # Frontend (HTML/CSS/JS)
│   ├── index.html         # Main UI with all views
│   ├── styles.css         # ThinkPad-themed dark styling
│   ├── main.js            # Application logic and Tauri commands
│   └── icons/             # SVG icons
│       └── fan.svg        # Custom fan icon
├── src-tauri/             # Backend (Rust)
│   ├── src/
│   │   └── lib.rs         # Tauri commands and system access
│   ├── tauri.conf.json    # Tauri configuration
│   └── icons/             # Application icons
├── ubuntu-installer.sh    # Package installation script
└── docs/                  # Documentation
    ├── THINKUTILS_MASTER_PLAN.md
    ├── IMPLEMENTATION_GUIDE.md
    ├── ROADMAP_VISUAL.md
    ├── FAN_CONTROL_FEATURE.md
    ├── BATTERY_FEATURE.md
    ├── PERFORMANCE_FEATURE.md
    └── MONITOR_FEATURE.md
```

## Contributing

Contributions are welcome! Areas for improvement:
- Additional ThinkPad utilities (battery, performance, etc.)
- Support for more ThinkPad models
- UI/UX enhancements
- Bug fixes and optimizations

## License

ThinkUtils is dual-licensed:

- **LGPL v3** - For open source projects
- **Commercial License** - For commercial/proprietary projects

For commercial licensing inquiries, please contact: https://www.vietanh.dev/contact

## Credits

- Built with [Tauri](https://tauri.app/)
- Icons from Lucide

## Disclaimer

ThinkUtils modifies system fan controls. While generally safe, use at your own risk. The authors are not responsible for any hardware damage that may occur from improper use.

**Always monitor temperatures when using manual fan control.**

---

**Made with ❤️ for ThinkPad enthusiasts**
