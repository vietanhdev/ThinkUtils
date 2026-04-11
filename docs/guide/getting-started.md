# Getting Started

ThinkUtils is a native Linux desktop app for ThinkPad laptops, built with [Tauri](https://tauri.app/). It gives you direct control over hardware that's normally locked behind command-line tools or config files.

## Prerequisites

ThinkUtils requires a few system packages:

::: code-group
```bash [Debian/Ubuntu]
sudo apt install lm-sensors policykit-1
```

```bash [Fedora/RHEL]
sudo dnf install lm_sensors polkit
```

```bash [Arch Linux]
sudo pacman -S lm_sensors polkit
```
:::

## Enable Fan Control

ThinkPad fan control requires the `thinkpad_acpi` kernel module with fan control enabled:

1. Create or edit the configuration:
   ```bash
   sudo nano /etc/modprobe.d/thinkpad_acpi.conf
   ```

2. Add this line:
   ```
   options thinkpad_acpi fan_control=1
   ```

3. Reboot or reload the module:
   ```bash
   sudo modprobe -r thinkpad_acpi
   sudo modprobe thinkpad_acpi
   ```

## First Launch

1. Install ThinkUtils (see [Installation](./installation))
2. Launch from your application menu or run `thinkutils`
3. Click **"Setup Permissions"** when prompted and enter your password once
4. All features now work without further password prompts

See [Permissions](./permissions) for details on what gets configured.

## Navigation

Use the left sidebar to switch between features:

| View | Purpose |
|------|---------|
| **Home** | Dashboard with quick controls |
| **Fan Control** | Temperature monitoring and fan speed |
| **Battery** | Charge thresholds and health |
| **Performance** | CPU governor and power profiles |
| **Monitor** | Real-time system stats |
| **System Info** | Hardware details |
| **Security** | Virus scanning |
| **MCP** | AI integration settings |
| **Sync** | Google Drive backup |
