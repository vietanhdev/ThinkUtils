# ThinkUtils Permission System

## Overview

ThinkUtils now features a user-friendly permission system that eliminates the need for repeated password entry.

## User Experience

### First Launch
1. App starts and checks if permissions are configured
2. If not configured, shows a friendly dialog explaining what's needed
3. User clicks "Setup Permissions" button
4. Password prompt appears **once**
5. Permissions are configured
6. Dialog closes automatically
7. App works without any more password prompts!

### Subsequent Launches
- No permission dialog
- No password prompts
- Everything just works

## Implementation

### Backend (Rust)

**New Module: `src-tauri/src/permissions.rs`**
- `check_permissions_status()` - Checks if system files are writable
- `setup_permissions()` - Uses pkexec once to configure file permissions

**System Files Configured:**
- CPU governor files
- Turbo boost control
- Fan control (pwm1, pwm1_enable)
- Battery thresholds

### Frontend (JavaScript)

**New Functions in `src/js/app.js`:**
- `checkAndSetupPermissions()` - Called at startup
- `showPermissionDialog()` - Displays the setup dialog
- `setupPermissions()` - Invokes the backend setup
- `setupPermissionDialog()` - Handles button clicks

**New UI in `src/index.html`:**
- Permission dialog with clear explanation
- "Setup Permissions" button (primary action)
- "Skip for Now" button (secondary action)

### Styling

**New CSS in `src/styles.css`:**
- `.dialog-button-primary` - Red themed primary button
- `.dialog-button-secondary` - Subtle secondary button

## Installation Scripts

### `install-policy.sh`
Installs the polkit policy required for the one-time setup.

### `setup-passwordless.sh`
Optional script that creates a polkit rule for completely passwordless operation (for sudo/wheel group users).

## Benefits

1. **Better UX**: Users only see one password prompt, ever
2. **No Caching Issues**: Doesn't rely on polkit's authentication cache
3. **Persistent**: Permissions stay configured (unless system resets /sys)
4. **Transparent**: Clear explanation of what's being configured
5. **Optional**: Users can skip if they prefer manual permission management

## Security Considerations

- Only modifies permissions on specific system files needed for laptop control
- Requires user to be in sudo/wheel group
- Uses standard polkit authentication
- File permissions are set to 666 (read/write for all) which is standard for these sysfs files
- No background processes or daemons required
