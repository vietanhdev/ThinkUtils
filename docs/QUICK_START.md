# Quick Start Guide

## Setup ThinkUtils Permissions

### Step 1: Install Polkit Policy

```bash
./install-policy.sh
```

This installs the policy that allows the one-time permission setup.

### Step 2: Build and Run

```bash
npm run tauri dev
```

### Step 3: Setup Permissions (First Launch Only)

When the app starts, you'll see a permission dialog:

1. Click **"Setup Permissions"**
2. Enter your password when prompted
3. Wait for setup to complete
4. Done! The dialog will close automatically

### Step 4: Use the App

Now you can use all features without any password prompts:
- Change CPU governor
- Control fan speed
- Set battery thresholds
- Toggle turbo boost

## Optional: Completely Passwordless

If you want to skip even the one-time password prompt:

```bash
./setup-passwordless.sh
```

This allows users in the sudo/wheel group to run ThinkUtils without any authentication.

## Troubleshooting

**Dialog doesn't appear:**
- The app may already have permissions configured
- Check if you can change settings without issues

**Setup fails:**
- Ensure you're in the sudo group: `groups | grep sudo`
- Try the passwordless setup: `./setup-passwordless.sh`

**Permissions reset after reboot:**
- Some systems reset `/sys` permissions on boot
- Run the setup again or use passwordless mode

## What's Configured

The setup modifies permissions on these system files:
- `/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor` - CPU governor
- `/sys/devices/system/cpu/intel_pstate/no_turbo` - Turbo boost
- `/sys/devices/platform/thinkpad_hwmon/pwm1*` - Fan control
- `/sys/class/power_supply/BAT*/charge_*_threshold` - Battery limits

After setup, your user can directly read/write these files without sudo.
