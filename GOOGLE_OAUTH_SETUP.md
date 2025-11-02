# Google OAuth Setup for ThinkUtils

To enable Google Drive sync, you need to create OAuth credentials in Google Cloud Console.

## Steps to Configure

### 1. Create a Google Cloud Project

1. Go to [Google Cloud Console](https://console.cloud.google.com/)
2. Create a new project or select an existing one
3. Enable the Google Drive API:
   - Go to "APIs & Services" > "Library"
   - Search for "Google Drive API"
   - Click "Enable"

### 2. Create OAuth Credentials

1. Go to "APIs & Services" > "Credentials"
2. Click "Create Credentials" > "OAuth client ID"
3. Configure the OAuth consent screen if prompted:
   - User Type: External
   - App name: ThinkUtils
   - User support email: Your email
   - Developer contact: Your email
4. Application type: Desktop app
5. Name: ThinkUtils Desktop
6. Click "Create"

### 3. Configure the Application

1. Download the credentials JSON or copy the Client ID and Client Secret
2. Open `src-tauri/src/sync.rs`
3. Replace the placeholder values:

```rust
const GOOGLE_CLIENT_ID: &str = "YOUR_CLIENT_ID.apps.googleusercontent.com";
const GOOGLE_CLIENT_SECRET: &str = "YOUR_CLIENT_SECRET";
```

### 4. Add Authorized Redirect URI

In Google Cloud Console:
1. Go to your OAuth client credentials
2. Add authorized redirect URI: `http://localhost:8765/callback`
3. Save changes

### 5. Rebuild the Application

```bash
cd src-tauri
cargo build --release
```

## How It Works

1. **Login**: Opens Google OAuth in your browser
2. **Authorization**: You grant ThinkUtils access to Google Drive
3. **Callback**: OAuth redirects to local server (localhost:8765)
4. **Token Exchange**: App exchanges auth code for access token
5. **Sync**: Settings are saved to/loaded from Google Drive as JSON

## File Storage

Settings are stored in Google Drive as `thinkutils_settings.json` in your Drive root.

## Security

- Access tokens are stored locally in `~/.config/thinkutils/sync_state.json`
- Only the Google Drive File scope is requested (not full Drive access)
- Tokens are never transmitted except to Google's OAuth servers

## Troubleshooting

### "Invalid client" error
- Check that Client ID and Secret are correct
- Ensure redirect URI is configured in Google Cloud Console

### "Access denied" error
- Make sure Google Drive API is enabled
- Check OAuth consent screen configuration

### Callback timeout
- Ensure port 8765 is not blocked by firewall
- Check that redirect URI matches exactly: `http://localhost:8765/callback`

## Alternative: Local Sync Only

If you don't want to set up Google OAuth, you can use the local file sync:
- Settings are saved to `~/.config/thinkutils/sync_state.json`
- No cloud sync, but settings persist between sessions
- Simply comment out the Google OAuth code and use local storage only
