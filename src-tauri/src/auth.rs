use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn authenticate_once() -> ApiResponse<String> {
    println!("[Auth] Requesting one-time authentication...");

    // Create a simple script that does nothing but succeeds
    let script_content = "#!/bin/bash\necho 'Authentication successful'\nexit 0";

    // Was a fixed path, /tmp/thinkutils_auth.sh, written with plain fs::write --
    // so any local user could pre-create it, or point a symlink at it, and have
    // their content executed as root.
    match crate::privileged::run_script(script_content).await {
        Ok(output) => {
            if output.status.success() {
                println!("[Auth] ✓ Authentication successful");
                ApiResponse {
                    success: true,
                    data: Some("Authenticated".to_string()),
                    error: None,
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("[Auth] ✗ Authentication failed: {}", stderr);
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Authentication failed: {}", stderr)),
                }
            }
        }
        Err(e) => {
            println!("[Auth] ✗ Failed to execute pkexec: {}", e);
            ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to execute authentication: {}", e)),
            }
        }
    }
}
