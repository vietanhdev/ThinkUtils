use serde::{Deserialize, Serialize};
use std::process::Command;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct SecurityStatus {
    pub clamav_installed: bool,
    pub clamav_running: bool,
    pub database_version: String,
    pub last_update: String,
    pub definitions_count: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanResult {
    pub success: bool,
    pub scanned_files: u32,
    pub infected_files: u32,
    pub scan_time: String,
    pub threats: Vec<ThreatInfo>,
    pub logs: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThreatInfo {
    pub file_path: String,
    pub threat_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstallResponse {
    pub success: bool,
    pub message: String,
    pub logs: Vec<String>,
    pub error: Option<String>,
}

// Check if ClamAV is installed and get status
#[tauri::command]
pub async fn get_security_status() -> Response<SecurityStatus> {
    let clamav_installed = check_clamav_installed();
    
    if !clamav_installed {
        return Response {
            success: true,
            data: Some(SecurityStatus {
                clamav_installed: false,
                clamav_running: false,
                database_version: "N/A".to_string(),
                last_update: "N/A".to_string(),
                definitions_count: "N/A".to_string(),
            }),
            error: None,
        };
    }

    let clamav_running = check_clamd_running();
    let (db_version, last_update, def_count) = get_database_info();

    Response {
        success: true,
        data: Some(SecurityStatus {
            clamav_installed,
            clamav_running,
            database_version: db_version,
            last_update,
            definitions_count: def_count,
        }),
        error: None,
    }
}

// Update ClamAV virus definitions
#[tauri::command]
pub async fn update_virus_definitions() -> Response<String> {
    if !check_clamav_installed() {
        return Response {
            success: false,
            data: None,
            error: Some("ClamAV is not installed".to_string()),
        };
    }

    match Command::new("pkexec")
        .args(&["freshclam"])
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                Response {
                    success: true,
                    data: Some("Virus definitions updated successfully".to_string()),
                    error: None,
                }
            } else {
                let error_msg = String::from_utf8_lossy(&output.stderr).to_string();
                Response {
                    success: false,
                    data: None,
                    error: Some(format!("Update failed: {}", error_msg)),
                }
            }
        }
        Err(e) => Response {
            success: false,
            data: None,
            error: Some(format!("Failed to run freshclam: {}", e)),
        },
    }
}

// Scan a file or directory
#[tauri::command]
pub async fn scan_path(path: String) -> Response<ScanResult> {
    let mut logs = Vec::new();
    
    if !check_clamav_installed() {
        logs.push("✗ ClamAV is not installed".to_string());
        return Response {
            success: false,
            data: None,
            error: Some("ClamAV is not installed".to_string()),
        };
    }

    if !Path::new(&path).exists() {
        logs.push(format!("✗ Path does not exist: {}", path));
        return Response {
            success: false,
            data: None,
            error: Some("Path does not exist".to_string()),
        };
    }

    logs.push(format!("Starting scan of: {}", path));
    logs.push("Running ClamAV scanner...".to_string());
    
    let start_time = std::time::Instant::now();

    match Command::new("clamscan")
        .args(&["-r", "-v", &path])
        .output()
    {
        Ok(output) => {
            let scan_time = format!("{:.2}s", start_time.elapsed().as_secs_f64());
            let stdout = String::from_utf8_lossy(&output.stdout);
            
            // Add scan output to logs (limit to important lines)
            let mut file_count = 0;
            for line in stdout.lines() {
                if line.contains("Scanning") {
                    file_count += 1;
                    if file_count <= 20 {
                        logs.push(format!("  {}", line.trim()));
                    } else if file_count == 21 {
                        logs.push("  ... (scanning continues)".to_string());
                    }
                } else if line.contains("FOUND") || line.contains("Infected files") || line.contains("Scanned files") {
                    logs.push(format!("  {}", line.trim()));
                }
            }
            
            let (scanned, infected, threats) = parse_scan_output(&stdout);

            if infected > 0 {
                logs.push(format!("⚠ Found {} threat(s) in {} file(s)", infected, scanned));
            } else {
                logs.push(format!("✓ Scan complete: {} files scanned, no threats found", scanned));
            }
            logs.push(format!("Scan completed in {}", scan_time));

            Response {
                success: true,
                data: Some(ScanResult {
                    success: true,
                    scanned_files: scanned,
                    infected_files: infected,
                    scan_time,
                    threats,
                    logs,
                    error: None,
                }),
                error: None,
            }
        }
        Err(e) => {
            logs.push(format!("✗ Scan failed: {}", e));
            Response {
                success: false,
                data: Some(ScanResult {
                    success: false,
                    scanned_files: 0,
                    infected_files: 0,
                    scan_time: "0s".to_string(),
                    threats: Vec::new(),
                    logs,
                    error: Some(format!("Scan failed: {}", e)),
                }),
                error: Some(format!("Scan failed: {}", e)),
            }
        }
    }
}

// Quick scan of common directories
#[tauri::command]
pub async fn quick_scan() -> Response<ScanResult> {
    let mut logs = Vec::new();
    
    if !check_clamav_installed() {
        logs.push("✗ ClamAV is not installed".to_string());
        return Response {
            success: false,
            data: None,
            error: Some("ClamAV is not installed".to_string()),
        };
    }
    
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/home".to_string());
    let scan_paths = vec![
        format!("{}/Downloads", home_dir),
        format!("{}/Documents", home_dir),
        format!("{}/Desktop", home_dir),
    ];

    logs.push("Starting quick scan of common directories...".to_string());
    
    let mut total_scanned = 0;
    let mut total_infected = 0;
    let mut all_threats = Vec::new();
    let start_time = std::time::Instant::now();

    for path in scan_paths {
        if !Path::new(&path).exists() {
            logs.push(format!("⊘ Skipping (not found): {}", path));
            continue;
        }

        logs.push(format!("Scanning: {}", path));

        if let Ok(output) = Command::new("clamscan")
            .args(&["-r", &path])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let (scanned, infected, mut threats) = parse_scan_output(&stdout);
            
            if infected > 0 {
                logs.push(format!("  ⚠ Found {} threat(s) in {} file(s)", infected, scanned));
            } else {
                logs.push(format!("  ✓ Clean - {} files scanned", scanned));
            }
            
            total_scanned += scanned;
            total_infected += infected;
            all_threats.append(&mut threats);
        } else {
            logs.push(format!("  ✗ Failed to scan {}", path));
        }
    }

    let scan_time = format!("{:.2}s", start_time.elapsed().as_secs_f64());

    if total_infected > 0 {
        logs.push(format!("⚠ Quick scan complete: {} threat(s) found in {} file(s)", total_infected, total_scanned));
    } else {
        logs.push(format!("✓ Quick scan complete: {} files scanned, no threats found", total_scanned));
    }
    logs.push(format!("Total scan time: {}", scan_time));

    Response {
        success: true,
        data: Some(ScanResult {
            success: true,
            scanned_files: total_scanned,
            infected_files: total_infected,
            scan_time,
            threats: all_threats,
            logs,
            error: None,
        }),
        error: None,
    }
}

// Install ClamAV
#[tauri::command]
pub async fn install_clamav() -> InstallResponse {
    let distro = detect_linux_distro();
    let mut logs = vec![format!("Detected distribution: {}", distro)];
    
    match distro.as_str() {
        "debian" | "ubuntu" | "linuxmint" | "pop" => {
            logs.push("Using APT package manager".to_string());
            install_with_apt(logs)
        }
        "fedora" | "rhel" | "centos" | "rocky" | "almalinux" => {
            logs.push("Using DNF package manager".to_string());
            install_with_dnf(logs)
        }
        "arch" | "manjaro" | "endeavouros" => {
            logs.push("Using Pacman package manager".to_string());
            install_with_pacman(logs)
        }
        "opensuse" | "suse" => {
            logs.push("Using Zypper package manager".to_string());
            install_with_zypper(logs)
        }
        _ => {
            logs.push("Automatic installation not supported for this distribution".to_string());
            InstallResponse {
                success: false,
                message: String::new(),
                logs,
                error: Some(format!(
                    "MANUAL_INSTALL:{}",
                    get_manual_install_instructions(&distro)
                )),
            }
        }
    }
}

fn detect_linux_distro() -> String {
    // Try to read /etc/os-release
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("ID=") {
                return line.replace("ID=", "").trim_matches('"').to_lowercase();
            }
        }
    }
    
    // Fallback checks
    if Path::new("/etc/debian_version").exists() {
        return "debian".to_string();
    }
    if Path::new("/etc/fedora-release").exists() {
        return "fedora".to_string();
    }
    if Path::new("/etc/arch-release").exists() {
        return "arch".to_string();
    }
    
    "unknown".to_string()
}

fn install_with_apt(mut logs: Vec<String>) -> InstallResponse {
    logs.push("Running: pkexec apt-get install -y clamav clamav-daemon".to_string());
    
    match Command::new("pkexec")
        .args(&["apt-get", "install", "-y", "clamav", "clamav-daemon"])
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            // Add output to logs
            for line in stdout.lines() {
                if !line.trim().is_empty() {
                    logs.push(format!("  {}", line));
                }
            }
            
            if output.status.success() {
                logs.push("✓ ClamAV packages installed successfully".to_string());
                logs.push("Updating virus definitions...".to_string());
                
                // Update definitions after install
                match Command::new("pkexec").args(&["freshclam"]).output() {
                    Ok(fresh_output) => {
                        let fresh_stdout = String::from_utf8_lossy(&fresh_output.stdout);
                        for line in fresh_stdout.lines().take(10) {
                            if !line.trim().is_empty() {
                                logs.push(format!("  {}", line));
                            }
                        }
                        if fresh_output.status.success() {
                            logs.push("✓ Virus definitions updated".to_string());
                        } else {
                            logs.push("⚠ Virus definitions update may have failed (run 'Update Definitions' manually)".to_string());
                        }
                    }
                    Err(_) => {
                        logs.push("⚠ Could not update definitions (run 'Update Definitions' manually)".to_string());
                    }
                }
                
                InstallResponse {
                    success: true,
                    message: "ClamAV installed successfully".to_string(),
                    logs,
                    error: None,
                }
            } else {
                for line in stderr.lines() {
                    if !line.trim().is_empty() {
                        logs.push(format!("  ERROR: {}", line));
                    }
                }
                logs.push("✗ Installation failed".to_string());
                
                InstallResponse {
                    success: false,
                    message: String::new(),
                    logs,
                    error: Some("Installation failed. Check logs for details.".to_string()),
                }
            }
        }
        Err(e) => {
            logs.push(format!("✗ Failed to execute installer: {}", e));
            InstallResponse {
                success: false,
                message: String::new(),
                logs,
                error: Some(format!("Failed to install: {}", e)),
            }
        }
    }
}

fn install_with_dnf(mut logs: Vec<String>) -> InstallResponse {
    logs.push("Running: pkexec dnf install -y clamav clamav-update clamd".to_string());
    
    match Command::new("pkexec")
        .args(&["dnf", "install", "-y", "clamav", "clamav-update", "clamd"])
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            for line in stdout.lines() {
                if !line.trim().is_empty() {
                    logs.push(format!("  {}", line));
                }
            }
            
            if output.status.success() {
                logs.push("✓ ClamAV packages installed successfully".to_string());
                logs.push("Updating virus definitions...".to_string());
                
                match Command::new("pkexec").args(&["freshclam"]).output() {
                    Ok(fresh_output) => {
                        let fresh_stdout = String::from_utf8_lossy(&fresh_output.stdout);
                        for line in fresh_stdout.lines().take(10) {
                            if !line.trim().is_empty() {
                                logs.push(format!("  {}", line));
                            }
                        }
                        if fresh_output.status.success() {
                            logs.push("✓ Virus definitions updated".to_string());
                        } else {
                            logs.push("⚠ Virus definitions update may have failed".to_string());
                        }
                    }
                    Err(_) => {
                        logs.push("⚠ Could not update definitions".to_string());
                    }
                }
                
                InstallResponse {
                    success: true,
                    message: "ClamAV installed successfully".to_string(),
                    logs,
                    error: None,
                }
            } else {
                for line in stderr.lines() {
                    if !line.trim().is_empty() {
                        logs.push(format!("  ERROR: {}", line));
                    }
                }
                logs.push("✗ Installation failed".to_string());
                
                InstallResponse {
                    success: false,
                    message: String::new(),
                    logs,
                    error: Some("Installation failed. Check logs for details.".to_string()),
                }
            }
        }
        Err(e) => {
            logs.push(format!("✗ Failed to execute installer: {}", e));
            InstallResponse {
                success: false,
                message: String::new(),
                logs,
                error: Some(format!("Failed to install: {}", e)),
            }
        }
    }
}

fn install_with_pacman(mut logs: Vec<String>) -> InstallResponse {
    logs.push("Running: pkexec pacman -S --noconfirm clamav".to_string());
    
    match Command::new("pkexec")
        .args(&["pacman", "-S", "--noconfirm", "clamav"])
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            for line in stdout.lines() {
                if !line.trim().is_empty() {
                    logs.push(format!("  {}", line));
                }
            }
            
            if output.status.success() {
                logs.push("✓ ClamAV packages installed successfully".to_string());
                logs.push("Updating virus definitions...".to_string());
                
                match Command::new("pkexec").args(&["freshclam"]).output() {
                    Ok(fresh_output) => {
                        let fresh_stdout = String::from_utf8_lossy(&fresh_output.stdout);
                        for line in fresh_stdout.lines().take(10) {
                            if !line.trim().is_empty() {
                                logs.push(format!("  {}", line));
                            }
                        }
                        if fresh_output.status.success() {
                            logs.push("✓ Virus definitions updated".to_string());
                        } else {
                            logs.push("⚠ Virus definitions update may have failed".to_string());
                        }
                    }
                    Err(_) => {
                        logs.push("⚠ Could not update definitions".to_string());
                    }
                }
                
                InstallResponse {
                    success: true,
                    message: "ClamAV installed successfully".to_string(),
                    logs,
                    error: None,
                }
            } else {
                for line in stderr.lines() {
                    if !line.trim().is_empty() {
                        logs.push(format!("  ERROR: {}", line));
                    }
                }
                logs.push("✗ Installation failed".to_string());
                
                InstallResponse {
                    success: false,
                    message: String::new(),
                    logs,
                    error: Some("Installation failed. Check logs for details.".to_string()),
                }
            }
        }
        Err(e) => {
            logs.push(format!("✗ Failed to execute installer: {}", e));
            InstallResponse {
                success: false,
                message: String::new(),
                logs,
                error: Some(format!("Failed to install: {}", e)),
            }
        }
    }
}

fn install_with_zypper(mut logs: Vec<String>) -> InstallResponse {
    logs.push("Running: pkexec zypper install -y clamav".to_string());
    
    match Command::new("pkexec")
        .args(&["zypper", "install", "-y", "clamav"])
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            for line in stdout.lines() {
                if !line.trim().is_empty() {
                    logs.push(format!("  {}", line));
                }
            }
            
            if output.status.success() {
                logs.push("✓ ClamAV packages installed successfully".to_string());
                logs.push("Updating virus definitions...".to_string());
                
                match Command::new("pkexec").args(&["freshclam"]).output() {
                    Ok(fresh_output) => {
                        let fresh_stdout = String::from_utf8_lossy(&fresh_output.stdout);
                        for line in fresh_stdout.lines().take(10) {
                            if !line.trim().is_empty() {
                                logs.push(format!("  {}", line));
                            }
                        }
                        if fresh_output.status.success() {
                            logs.push("✓ Virus definitions updated".to_string());
                        } else {
                            logs.push("⚠ Virus definitions update may have failed".to_string());
                        }
                    }
                    Err(_) => {
                        logs.push("⚠ Could not update definitions".to_string());
                    }
                }
                
                InstallResponse {
                    success: true,
                    message: "ClamAV installed successfully".to_string(),
                    logs,
                    error: None,
                }
            } else {
                for line in stderr.lines() {
                    if !line.trim().is_empty() {
                        logs.push(format!("  ERROR: {}", line));
                    }
                }
                logs.push("✗ Installation failed".to_string());
                
                InstallResponse {
                    success: false,
                    message: String::new(),
                    logs,
                    error: Some("Installation failed. Check logs for details.".to_string()),
                }
            }
        }
        Err(e) => {
            logs.push(format!("✗ Failed to execute installer: {}", e));
            InstallResponse {
                success: false,
                message: String::new(),
                logs,
                error: Some(format!("Failed to install: {}", e)),
            }
        }
    }
}

fn get_manual_install_instructions(distro: &str) -> String {
    match distro {
        "gentoo" => "emerge -av app-antivirus/clamav",
        "alpine" => "apk add clamav clamav-daemon",
        "void" => "xbps-install -S clamav",
        "solus" => "eopkg install clamav",
        _ => "Please install ClamAV using your distribution's package manager. Package name is usually 'clamav'.",
    }.to_string()
}

// Helper functions
fn check_clamav_installed() -> bool {
    Command::new("which")
        .arg("clamscan")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn check_clamd_running() -> bool {
    Command::new("systemctl")
        .args(&["is-active", "clamav-daemon"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn get_database_info() -> (String, String, String) {
    // Try different database file extensions and names
    let db_paths = vec![
        "/var/lib/clamav/daily.cld",
        "/var/lib/clamav/daily.cvd",
        "/var/lib/clamav/main.cld",
        "/var/lib/clamav/main.cvd",
    ];

    for db_path in db_paths {
        if Path::new(db_path).exists() {
            match Command::new("sigtool").args(&["--info", db_path]).output() {
                Ok(output) if output.status.success() => {
                    let info = String::from_utf8_lossy(&output.stdout);
                    let version = extract_field(&info, "Version:");
                    let signatures = extract_field(&info, "Signatures:");
                    let build_time = extract_field(&info, "Build time:");
                    
                    // If we got valid data, return it
                    if version != "Unknown" || signatures != "Unknown" {
                        return (version, build_time, signatures);
                    }
                }
                _ => continue,
            }
        }
    }

    // If no database files found or readable, check if freshclam needs to run
    ("Not initialized".to_string(), "Never".to_string(), "0".to_string())
}

fn extract_field(text: &str, field: &str) -> String {
    text.lines()
        .find(|line| line.contains(field))
        .and_then(|line| line.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn parse_scan_output(output: &str) -> (u32, u32, Vec<ThreatInfo>) {
    let mut scanned = 0;
    let mut infected = 0;
    let mut threats = Vec::new();

    for line in output.lines() {
        if line.contains("Scanned files:") {
            if let Some(num) = line.split(':').nth(1) {
                scanned = num.trim().parse().unwrap_or(0);
            }
        } else if line.contains("Infected files:") {
            if let Some(num) = line.split(':').nth(1) {
                infected = num.trim().parse().unwrap_or(0);
            }
        } else if line.contains("FOUND") {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 2 {
                let file_path = parts[0].trim().to_string();
                let threat_name = parts[1].replace("FOUND", "").trim().to_string();
                threats.push(ThreatInfo {
                    file_path,
                    threat_name,
                });
            }
        }
    }

    // If we didn't find summary, count lines
    if scanned == 0 {
        scanned = output.lines().filter(|l| !l.is_empty()).count() as u32;
    }

    (scanned, infected, threats)
}
