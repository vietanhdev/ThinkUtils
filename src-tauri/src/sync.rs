use serde::{Deserialize, Serialize};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

// Google OAuth credentials - Users need to create their own at https://console.cloud.google.com
const GOOGLE_CLIENT_ID: &str = "787652804555-akmgh2mr0kdif7hafo43rhnso0q1ds4f.apps.googleusercontent.com";
const GOOGLE_CLIENT_SECRET: &str = "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";
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncState {
    pub is_logged_in: bool,
    pub user_email: Option<String>,
    pub last_sync: Option<String>,
    pub settings: UserSettings,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            is_logged_in: false,
            user_email: None,
            last_sync: None,
            settings: UserSettings::default(),
            access_token: None,
            refresh_token: None,
        }
    }
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
    
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read sync state: {}", e))?;
    
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse sync state: {}", e))
}

fn save_sync_state(state: &SyncState) -> Result<(), String> {
    let path = get_sync_state_path()?;
    
    let content = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Failed to serialize sync state: {}", e))?;
    
    fs::write(&path, content)
        .map_err(|e| format!("Failed to write sync state: {}", e))?;
    
    Ok(())
}

fn create_oauth_client() -> Result<BasicClient, String> {
    let client_id = ClientId::new(GOOGLE_CLIENT_ID.to_string());
    let client_secret = ClientSecret::new(GOOGLE_CLIENT_SECRET.to_string());
    let auth_url = AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
        .map_err(|e| format!("Invalid auth URL: {}", e))?;
    let token_url = TokenUrl::new("https://oauth2.googleapis.com/token".to_string())
        .map_err(|e| format!("Invalid token URL: {}", e))?;
    
    Ok(BasicClient::new(
        client_id,
        Some(client_secret),
        auth_url,
        Some(token_url),
    )
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
        Err(e) => return ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to create OAuth client: {}", e)),
        },
    };
    
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    
    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("https://www.googleapis.com/auth/userinfo.email".to_string()))
        .add_scope(Scope::new("https://www.googleapis.com/auth/drive.file".to_string()))
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
    use tiny_http::{Server, Response};
    
    let server = Server::http("127.0.0.1:8765")
        .map_err(|e| format!("Failed to start server: {}", e))?;
    
    println!("[OAuth] Callback server listening on {}", REDIRECT_URI);
    
    // Handle one request then shutdown
    if let Ok(request) = server.recv() {
        let url = format!("http://localhost:8765{}", request.url());
        
        if let Ok(parsed_url) = url::Url::parse(&url) {
            let params: HashMap<String, String> = parsed_url
                .query_pairs()
                .into_owned()
                .collect();
            
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
        oauth_state.remove(&state)
            .ok_or_else(|| "Invalid state parameter".to_string())?
    };
    
    let client = create_oauth_client()?;
    
    let token_result = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(oauth2::PkceCodeVerifier::new(pkce_verifier))
        .request_async(async_http_client)
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
        Err(e) => return ApiResponse {
            success: false,
            data: None,
            error: Some(e),
        },
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
        let url = format!("https://www.googleapis.com/upload/drive/v3/files/{}?uploadType=media", id);
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
            metadata.to_string(),
            boundary,
            settings_json,
            boundary
        );
        
        client
            .post("https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart")
            .bearer_auth(access_token)
            .header("Content-Type", format!("multipart/related; boundary={}", boundary))
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
        Err(e) => return ApiResponse {
            success: false,
            data: None,
            error: Some(e),
        },
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
    let url = format!("https://www.googleapis.com/drive/v3/files/{}?alt=media", file_id);
    
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
