use serde::{Deserialize, Serialize};
use std::fs;

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

    let temp_script = "/tmp/thinkutils_auth.sh";
    if let Err(e) = fs::write(temp_script, script_content) {
        return ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to create auth script: {}", e)),
        };
    }

    // Make it executable
    let _ = std::process::Command::new("chmod")
        .arg("+x")
        .arg(temp_script)
        .output();

    // Run with pkexec
    match tokio::process::Command::new("pkexec")
        .env("PKEXEC_UID", std::env::var("UID").unwrap_or_default())
        .arg("bash")
        .arg(temp_script)
        .output()
        .await
    {
        Ok(output) => {
            let _ = fs::remove_file(temp_script);

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
            let _ = fs::remove_file(temp_script);
            println!("[Auth] ✗ Failed to execute pkexec: {}", e);
            ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to execute authentication: {}", e)),
            }
        }
    }
}
