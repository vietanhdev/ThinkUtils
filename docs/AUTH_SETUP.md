# Permission Setup

## One-Time Permission Configuration

ThinkUtils uses a smart permission system that only requires your password **once** during initial setup.

### How It Works

1. **First Launch**: When you start ThinkUtils for the first time, you'll see a permission setup dialog
2. **One-Time Setup**: Click "Setup Permissions" and enter your password once
3. **No More Passwords**: After setup, all operations work without any password prompts!

### What Happens During Setup

The app configures file permissions for:
- CPU frequency and governor control (`/sys/devices/system/cpu/...`)
- Fan speed control (`/sys/devices/platform/thinkpad_hwmon/...`)
- Battery charge thresholds (`/sys/class/power_supply/...`)
- Turbo boost settings

These files are given read/write permissions for your user, so no sudo is needed afterward.

### Installation

1. Install the polkit policy (required for the one-time setup):
   ```bash
   ./install-policy.sh
   ```

2. Build and run the app:
   ```bash
   npm run tauri build
   # or for development
   npm run tauri dev
   ```

3. On first launch, click "Setup Permissions" when prompted

### Alternative: Passwordless Setup (Optional)

If you want to skip the password prompt entirely, run:

```bash
./setup-passwordless.sh
```

This creates a polkit rule that allows users in the sudo/wheel group to run ThinkUtils without any password.

### Technical Details

**Permission Setup Approach:**
- Uses `pkexec` once to run a setup script
- The script sets file permissions (chmod 666) on system files
- Changes file ownership to your user
- After setup, your user can directly read/write these files
- No sudo or pkexec needed for normal operations

**Files Modified:**
- `/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor`
- `/sys/devices/system/cpu/intel_pstate/no_turbo`
- `/sys/devices/platform/thinkpad_hwmon/pwm1*`
- `/sys/class/power_supply/BAT*/charge_*_threshold`

### Troubleshooting

**Permission dialog doesn't appear:**
- Check console logs for errors
- Manually run: `npm run tauri dev` and check terminal output

**Setup fails:**
- Ensure you're in the sudo/wheel group: `groups`
- Try the passwordless setup script: `./setup-passwordless.sh`

**Permissions reset after reboot:**
- Some systems reset `/sys` permissions on boot
- You may need to run setup again or use the passwordless approach
