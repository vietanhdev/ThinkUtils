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

### 4. Build with the client ID

The ID is supplied at build time, not committed:

```bash
THINKUTILS_GOOGLE_CLIENT_ID="YOUR_CLIENT_ID.apps.googleusercontent.com" \
  npm run tauri build
```

::: warning There is no client secret, deliberately
A desktop binary cannot keep a secret. Anything compiled in is readable by
anyone holding the binary — which is exactly how this project's first client
secret ended up public, in the initial commit.

ThinkUtils is a **public client using PKCE**, which is Google's own guidance for
installed apps. PKCE is what protects the exchange: the verifier is generated per
authorisation, never leaves the process, and an intercepted authorisation code is
useless without it. A shipped secret would add nothing an attacker cannot read,
while creating something that has to be rotated when it leaks.

If Google's console offers you a secret for a **Desktop app** client, you can
ignore it. Do not add it to the build.
:::

A test asserts `set_client_secret` never appears in `sync.rs`, so re-adding one
to work around an auth error fails CI rather than shipping.

## How It Works

1. User clicks Login — opens Google OAuth in browser
2. User authorizes ThinkUtils for Google Drive access
3. OAuth redirects to `localhost:8765/callback`
4. App exchanges the auth code for a token, proving possession of the PKCE
   verifier it generated in step 1
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
