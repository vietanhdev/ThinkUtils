# Settings Sync

Sync your ThinkUtils configuration to Google Drive and restore it on any ThinkPad.

![Settings Sync](/screenshots/settings_sync.png)

## Features

- **Cloud Backup**: Save fan mode, battery thresholds, and app preferences to Google Drive
- **Cross-Device**: Keep settings consistent across multiple ThinkPads
- **Secure**: OAuth authentication — your credentials never touch ThinkUtils servers

## Usage

1. Click **Sign in with Google**
2. Authorize ThinkUtils to access Google Drive
3. **Sync Now** to backup current settings
4. **Download Settings** to restore from cloud

Settings are stored as `thinkutils_settings.json` in your Google Drive root.

## Setup

Google Drive sync requires OAuth credentials. This is a one-time setup for developers building from source.

See [Google OAuth Setup](/development/google-oauth) for configuration instructions.

::: info
Pre-built releases may come with OAuth already configured. Check the release notes.
:::
