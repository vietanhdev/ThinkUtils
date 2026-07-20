use chrono::Utc;
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl,
    Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// Google OAuth credentials, supplied at build time. Never hardcode these:
// a desktop binary cannot keep a secret, so anything embedded here is public.
// Set THINKUTILS_GOOGLE_CLIENT_ID / _SECRET when building to enable sync.
// Create credentials at https://console.cloud.google.com (APIs & Services > Credentials).
const GOOGLE_CLIENT_ID: Option<&str> = option_env!("THINKUTILS_GOOGLE_CLIENT_ID");
const GOOGLE_CLIENT_SECRET: Option<&str> = option_env!("THINKUTILS_GOOGLE_CLIENT_SECRET");
const REDIRECT_URI: &str = "http://localhost:8765/callback";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserSettings {
    pub fan_mode: String,
    pub fan_level: u8,
    pub auto_start: bool,
    pub minimize_to_tray: bool,
    pub theme: String,
    pub battery_start_threshold: Option<u8>,
    pub battery_stop_threshold: Option<u8>,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            fan_mode: "auto".to_string(),
            fan_level: 0,
            auto_start: false,
            minimize_to_tray: true,
            theme: "system".to_string(),
            battery_start_threshold: Some(40),
            battery_stop_threshold: Some(80),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SyncState {
    pub is_logged_in: bool,
    pub user_email: Option<String>,
    pub last_sync: Option<String>,
    pub settings: UserSettings,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthInitResponse {
    pub auth_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GoogleUserInfo {
    email: String,
    name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DriveFile {
    id: Option<String>,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct DriveFileList {
    files: Vec<DriveFile>,
}

// Global state for OAuth flow
lazy_static::lazy_static! {
    static ref OAUTH_STATE: Arc<Mutex<HashMap<String, (String, String)>>> = Arc::new(Mutex::new(HashMap::new()));
}

fn get_config_dir() -> Result<PathBuf, String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "Could not find home directory".to_string())?;

    let config_dir = PathBuf::from(home).join(".config").join("thinkutils");

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    Ok(config_dir)
}

fn get_sync_state_path() -> Result<PathBuf, String> {
    Ok(get_config_dir()?.join("sync_state.json"))
}

fn load_sync_state() -> Result<SyncState, String> {
    let path = get_sync_state_path()?;

    if !path.exists() {
        return Ok(SyncState::default());
    }

    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read sync state: {}", e))?;

    serde_json::from_str(&content).map_err(|e| format!("Failed to parse sync state: {}", e))
}

fn save_sync_state(state: &SyncState) -> Result<(), String> {
    let path = get_sync_state_path()?;

    let content = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Failed to serialize sync state: {}", e))?;

    write_private(&path, &content)
}

/// Write a file readable only by its owner.
///
/// The sync state holds OAuth access and refresh tokens, so it must never be
/// left at the default umask where other local users can read it.
fn write_private(path: &std::path::Path, content: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| format!("Failed to write sync state: {}", e))?;
        file.write_all(content.as_bytes())
            .map_err(|e| format!("Failed to write sync state: {}", e))?;

        // .mode() only applies when the file is created, so a file that already
        // existed at 0644 would keep those bits. Tighten it explicitly.
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("Failed to secure sync state: {}", e))?;
    }

    #[cfg(not(unix))]
    fs::write(path, content).map_err(|e| format!("Failed to write sync state: {}", e))?;

    Ok(())
}

/// The HTTP client oauth2 5.x uses for token exchange.
///
/// Redirects are disabled deliberately, per the oauth2 crate's own guidance:
/// following them on a token endpoint opens the client to SSRF. In 4.x this was
/// hidden inside the crate's `async_http_client`; 5.x hands the caller the
/// choice, so the choice has to be made explicitly.
fn oauth_http_client() -> Result<reqwest::Client, String> {
    reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))
}

fn create_oauth_client() -> Result<
    oauth2::basic::BasicClient<
        oauth2::EndpointSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointSet,
    >,
    String,
> {
    let client_id = ClientId::new(
        GOOGLE_CLIENT_ID
            .filter(|s| !s.is_empty())
            .ok_or(
                "Google sync is not configured in this build (THINKUTILS_GOOGLE_CLIENT_ID unset).",
            )?
            .to_string(),
    );
    let client_secret = ClientSecret::new(
        GOOGLE_CLIENT_SECRET
            .filter(|s| !s.is_empty())
            .ok_or("Google sync is not configured in this build (THINKUTILS_GOOGLE_CLIENT_SECRET unset).")?
            .to_string(),
    );
    let auth_url = AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
        .map_err(|e| format!("Invalid auth URL: {}", e))?;
    let token_url = TokenUrl::new("https://oauth2.googleapis.com/token".to_string())
        .map_err(|e| format!("Invalid token URL: {}", e))?;

    // 5.x replaces the four-argument constructor with a builder, and encodes
    // which endpoints are configured in the type -- hence the EndpointSet
    // parameters on the return type above.
    Ok(BasicClient::new(client_id)
        .set_client_secret(client_secret)
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(
            RedirectUrl::new(REDIRECT_URI.to_string())
                .map_err(|e| format!("Invalid redirect URL: {}", e))?,
        ))
}

// Initiate Google OAuth flow
#[tauri::command]
pub async fn google_auth_init() -> ApiResponse<AuthInitResponse> {
    println!("[Google] Initiating OAuth flow");

    let client = match create_oauth_client() {
        Ok(c) => c,
        Err(e) => {
            return ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to create OAuth client: {}", e)),
            }
        }
    };

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/userinfo.email".to_string(),
        ))
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/drive.file".to_string(),
        ))
        .set_pkce_challenge(pkce_challenge)
        .url();

    // Store CSRF token and PKCE verifier for validation
    let mut state = OAUTH_STATE.lock().unwrap();
    state.insert(
        csrf_token.secret().clone(),
        (pkce_verifier.secret().clone(), csrf_token.secret().clone()),
    );

    // Start local server to handle callback
    tokio::spawn(async move {
        if let Err(e) = start_callback_server().await {
            eprintln!("[OAuth] Callback server error: {}", e);
        }
    });

    ApiResponse {
        success: true,
        data: Some(AuthInitResponse {
            auth_url: auth_url.to_string(),
        }),
        error: None,
    }
}

async fn start_callback_server() -> Result<(), String> {
    use tiny_http::{Response, Server};

    let server =
        Server::http("127.0.0.1:8765").map_err(|e| format!("Failed to start server: {}", e))?;

    println!("[OAuth] Callback server listening on {}", REDIRECT_URI);

    // Handle one request then shutdown
    if let Ok(request) = server.recv() {
        let url = format!("http://localhost:8765{}", request.url());

        if let Ok(parsed_url) = url::Url::parse(&url) {
            let params: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();

            let code = params.get("code").cloned();
            let state = params.get("state").cloned();

            if let (Some(code), Some(state)) = (code, state) {
                // Exchange code for token
                tokio::spawn(async move {
                    if let Err(e) = exchange_code_for_token(code, state).await {
                        eprintln!("[OAuth] Token exchange failed: {}", e);
                    }
                });

                let response = Response::from_string(
                    "<html><body><h1>Login Successful!</h1><p>You can close this window and return to ThinkUtils.</p></body></html>"
                );
                let _ = request.respond(response);
            }
        }
    }

    Ok(())
}

async fn exchange_code_for_token(code: String, state: String) -> Result<(), String> {
    println!("[OAuth] Exchanging code for token");

    let (pkce_verifier, _) = {
        let mut oauth_state = OAUTH_STATE.lock().unwrap();
        oauth_state
            .remove(&state)
            .ok_or_else(|| "Invalid state parameter".to_string())?
    };

    let client = create_oauth_client()?;

    let http_client = oauth_http_client()?;
    let token_result = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(oauth2::PkceCodeVerifier::new(pkce_verifier))
        .request_async(&http_client)
        .await
        .map_err(|e| format!("Token exchange failed: {}", e))?;

    let access_token = token_result.access_token().secret().clone();
    let refresh_token = token_result.refresh_token().map(|t| t.secret().clone());

    // Get user info
    let user_info = get_user_info(&access_token).await?;

    // Save tokens and user info
    let mut sync_state = load_sync_state()?;
    sync_state.is_logged_in = true;
    sync_state.user_email = Some(user_info.email);
    sync_state.access_token = Some(access_token);
    sync_state.refresh_token = refresh_token;
    sync_state.last_sync = Some(Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());

    save_sync_state(&sync_state)?;

    println!("[OAuth] Login successful");

    Ok(())
}

async fn get_user_info(access_token: &str) -> Result<GoogleUserInfo, String> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| format!("Failed to get user info: {}", e))?;

    response
        .json::<GoogleUserInfo>()
        .await
        .map_err(|e| format!("Failed to parse user info: {}", e))
}

// Check auth status
#[tauri::command]
pub async fn google_auth_status() -> ApiResponse<SyncState> {
    match load_sync_state() {
        Ok(state) => ApiResponse {
            success: true,
            data: Some(state),
            error: None,
        },
        Err(e) => ApiResponse {
            success: false,
            data: None,
            error: Some(e),
        },
    }
}

// Sync settings to Google Drive
#[tauri::command]
pub async fn sync_to_cloud(settings: UserSettings) -> ApiResponse<String> {
    println!("[Sync] Syncing settings to Google Drive");

    let mut state = match load_sync_state() {
        Ok(s) => s,
        Err(e) => {
            return ApiResponse {
                success: false,
                data: None,
                error: Some(e),
            }
        }
    };

    if !state.is_logged_in || state.access_token.is_none() {
        return ApiResponse {
            success: false,
            data: None,
            error: Some("Not logged in".to_string()),
        };
    }

    let access_token = state.access_token.as_ref().unwrap();

    // Upload to Google Drive
    match upload_to_drive(access_token, &settings).await {
        Ok(_) => {
            state.settings = settings;
            state.last_sync = Some(Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());

            if let Err(e) = save_sync_state(&state) {
                return ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e),
                };
            }

            ApiResponse {
                success: true,
                data: Some("Settings synced to Google Drive".to_string()),
                error: None,
            }
        }
        Err(e) => ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to sync: {}", e)),
        },
    }
}

async fn upload_to_drive(access_token: &str, settings: &UserSettings) -> Result<(), String> {
    let client = reqwest::Client::new();

    // Check if file exists
    let file_id = find_settings_file(access_token).await?;

    let settings_json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    if let Some(id) = file_id {
        // Update existing file
        let url = format!(
            "https://www.googleapis.com/upload/drive/v3/files/{}?uploadType=media",
            id
        );
        client
            .patch(&url)
            .bearer_auth(access_token)
            .header("Content-Type", "application/json")
            .body(settings_json)
            .send()
            .await
            .map_err(|e| format!("Failed to update file: {}", e))?;
    } else {
        // Create new file
        let metadata = serde_json::json!({
            "name": "thinkutils_settings.json",
            "mimeType": "application/json"
        });

        let boundary = "thinkutils_boundary";
        let body = format!(
            "--{}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{}\r\n--{}\r\nContent-Type: application/json\r\n\r\n{}\r\n--{}--",
            boundary,
            metadata,
            boundary,
            settings_json,
            boundary
        );

        client
            .post("https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart")
            .bearer_auth(access_token)
            .header(
                "Content-Type",
                format!("multipart/related; boundary={}", boundary),
            )
            .body(body)
            .send()
            .await
            .map_err(|e| format!("Failed to create file: {}", e))?;
    }

    Ok(())
}

async fn find_settings_file(access_token: &str) -> Result<Option<String>, String> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://www.googleapis.com/drive/v3/files")
        .bearer_auth(access_token)
        .query(&[("q", "name='thinkutils_settings.json'")])
        .send()
        .await
        .map_err(|e| format!("Failed to search files: {}", e))?;

    let file_list: DriveFileList = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse file list: {}", e))?;

    Ok(file_list.files.first().and_then(|f| f.id.clone()))
}

// Download settings from Google Drive
#[tauri::command]
pub async fn sync_from_cloud() -> ApiResponse<UserSettings> {
    println!("[Sync] Downloading settings from Google Drive");

    let state = match load_sync_state() {
        Ok(s) => s,
        Err(e) => {
            return ApiResponse {
                success: false,
                data: None,
                error: Some(e),
            }
        }
    };

    if !state.is_logged_in || state.access_token.is_none() {
        return ApiResponse {
            success: false,
            data: None,
            error: Some("Not logged in".to_string()),
        };
    }

    let access_token = state.access_token.as_ref().unwrap();

    match download_from_drive(access_token).await {
        Ok(settings) => ApiResponse {
            success: true,
            data: Some(settings),
            error: None,
        },
        Err(e) => ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to download: {}", e)),
        },
    }
}

async fn download_from_drive(access_token: &str) -> Result<UserSettings, String> {
    let file_id = find_settings_file(access_token)
        .await?
        .ok_or_else(|| "Settings file not found in Google Drive".to_string())?;

    let client = reqwest::Client::new();
    let url = format!(
        "https://www.googleapis.com/drive/v3/files/{}?alt=media",
        file_id
    );

    let response = client
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| format!("Failed to download file: {}", e))?;

    let settings: UserSettings = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse settings: {}", e))?;

    Ok(settings)
}

// Logout
#[tauri::command]
pub async fn google_logout() -> ApiResponse<String> {
    println!("[Google] Logging out");

    match load_sync_state() {
        Ok(mut state) => {
            state.is_logged_in = false;
            state.user_email = None;
            state.access_token = None;
            state.refresh_token = None;

            match save_sync_state(&state) {
                Ok(_) => ApiResponse {
                    success: true,
                    data: Some("Logged out successfully".to_string()),
                    error: None,
                },
                Err(e) => ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e),
                },
            }
        }
        Err(e) => ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to load state: {}", e)),
        },
    }
}

// Get current settings
#[tauri::command]
pub fn get_settings() -> ApiResponse<UserSettings> {
    match load_sync_state() {
        Ok(state) => ApiResponse {
            success: true,
            data: Some(state.settings),
            error: None,
        },
        Err(e) => ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to load settings: {}", e)),
        },
    }
}

// Save settings locally
#[tauri::command]
pub fn save_settings(settings: UserSettings) -> ApiResponse<String> {
    println!("[Settings] Saving: {:?}", settings);

    match load_sync_state() {
        Ok(mut state) => {
            state.settings = settings;
            match save_sync_state(&state) {
                Ok(_) => ApiResponse {
                    success: true,
                    data: Some("Settings saved".to_string()),
                    error: None,
                },
                Err(e) => ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e),
                },
            }
        }
        Err(e) => ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to load current state: {}", e)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unique temp path per call; std has no tempdir and we don't want a dev-dependency
    /// for two tests.
    fn temp_path(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "thinkutils_test_{}_{}_{}",
            tag,
            std::process::id(),
            nanos
        ))
    }

    #[cfg(unix)]
    #[test]
    fn write_private_creates_owner_only_file() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_path("create");
        write_private(&path, "{\"token\":\"secret\"}").expect("write should succeed");

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {:o}", mode);
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"token\":\"secret\"}");

        let _ = fs::remove_file(&path);
    }

    /// The regression this guards: OpenOptions::mode() only applies at creation, so a
    /// sync_state.json already on disk at 0644 (as shipped before this fix) would keep
    /// its world-readable bits and continue leaking tokens.
    #[cfg(unix)]
    #[test]
    fn write_private_tightens_existing_world_readable_file() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_path("tighten");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o644
        );

        write_private(&path, "new").expect("write should succeed");

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "pre-existing file was not tightened; got {:o}",
            mode
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");

        let _ = fs::remove_file(&path);
    }

    /// The redirect policy on the OAuth client is the SSRF guard, and nothing in
    /// the type system holds it in place: swapping `oauth_http_client()` back to
    /// `reqwest::Client::new()` compiles cleanly and silently restores the hole.
    /// oauth2 4.x hid this decision inside the crate; 5.x made it the caller's,
    /// which means it is now ours to regress.
    ///
    /// So assert the behaviour rather than the construction — serve a 302 and
    /// require that it comes back *as* a 302, with the redirect target never
    /// requested.
    #[tokio::test]
    async fn oauth_client_does_not_follow_redirects() {
        use std::io::{Read, Write};
        use std::sync::mpsc;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel::<String>();

        // Detached: if the client correctly declines to follow, the second
        // accept() blocks forever. The test never joins it, and the harness
        // does not wait on detached threads at exit.
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut buf = [0u8; 1024];
                let n = stream.read(&mut buf).unwrap_or(0);
                let path = String::from_utf8_lossy(&buf[..n])
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("?")
                    .to_string();
                if tx.send(path.clone()).is_err() {
                    break;
                }
                let response = if path == "/token" {
                    "HTTP/1.1 302 Found\r\nLocation: /followed\r\nContent-Length: 0\r\n\r\n"
                } else {
                    "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"
                };
                let _ = stream.write_all(response.as_bytes());
            }
        });

        let response = oauth_http_client()
            .expect("client builds")
            .get(format!("http://{}/token", addr))
            .send()
            .await
            .expect("request reaches the local server");

        assert_eq!(
            response.status().as_u16(),
            302,
            "the 302 was consumed instead of returned, so redirects are being followed"
        );

        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap(),
            "/token",
            "server should have seen the initial request"
        );
        assert!(
            rx.recv_timeout(std::time::Duration::from_millis(500))
                .is_err(),
            "client followed the Location header — a token endpoint that redirects \
             can now point this client at an arbitrary host"
        );
    }
}
