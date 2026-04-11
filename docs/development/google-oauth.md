# Google OAuth Setup

Google Drive sync requires OAuth credentials from Google Cloud Console.

## Setup Steps

### 1. Create a Google Cloud Project

1. Go to [Google Cloud Console](https://console.cloud.google.com/)
2. Create a new project (or select an existing one)
3. Enable the **Google Drive API** (APIs & Services > Library)

### 2. Create OAuth Credentials

1. Go to APIs & Services > Credentials
2. Click **Create Credentials > OAuth client ID**
3. Configure the consent screen if prompted (External, app name: ThinkUtils)
4. Application type: **Desktop app**
5. Name: ThinkUtils Desktop
6. Click Create

### 3. Add Redirect URI

In your OAuth client settings, add:
```
http://localhost:8765/callback
```

### 4. Configure the App

Replace the placeholder values in `src-tauri/src/sync.rs`:

```rust
const GOOGLE_CLIENT_ID: &str = "YOUR_CLIENT_ID.apps.googleusercontent.com";
const GOOGLE_CLIENT_SECRET: &str = "YOUR_CLIENT_SECRET";
```

### 5. Rebuild

```bash
npm run tauri build
```

## How It Works

1. User clicks Login — opens Google OAuth in browser
2. User authorizes ThinkUtils for Google Drive access
3. OAuth redirects to `localhost:8765/callback`
4. App exchanges auth code for access token
5. Settings sync as `thinkutils_settings.json` in Google Drive root

## Security

- Access tokens stored locally in `~/.config/thinkutils/sync_state.json`
- Only the Google Drive File scope is requested (not full Drive access)
- Tokens are never transmitted except to Google's OAuth servers

## Troubleshooting

| Problem | Solution |
|---------|----------|
| "Invalid client" error | Verify Client ID and Secret; check redirect URI in Cloud Console |
| "Access denied" error | Ensure Google Drive API is enabled; check consent screen config |
| Callback timeout | Ensure port 8765 is not blocked by firewall |
