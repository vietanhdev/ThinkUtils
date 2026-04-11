use rmcp::transport::sse_server::SseServer;
use rmcp::{ServerHandler, ServiceExt, model::ServerInfo, schemars, tool};
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

const HELPER_PATH: &str = "/usr/local/bin/thinkutils-fan-control";

// -- Shared state for managing the MCP server lifecycle --

pub struct McpServerState {
    cancel_token: Option<CancellationToken>,
    pub host: String,
    pub port: u16,
}

impl Default for McpServerState {
    fn default() -> Self {
        Self {
            cancel_token: None,
            host: "127.0.0.1".to_string(),
            port: 8765,
        }
    }
}

pub type McpState = Arc<Mutex<McpServerState>>;

// -- API response --

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

// -- MCP tool request types --

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetFanSpeedRequest {
    #[schemars(description = "Fan speed: auto, full-speed, or 0-7")]
    pub speed: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetBatteryThresholdsRequest {
    #[schemars(description = "Start charging below this percentage (0-100)")]
    pub start: u32,
    #[schemars(description = "Stop charging above this percentage (0-100)")]
    pub stop: u32,
}

// -- MCP Server handler --

#[derive(Debug, Clone)]
struct ThinkUtilsHandler;

#[tool(tool_box)]
impl ThinkUtilsHandler {
    #[tool(description = "Get ThinkPad fan status: speed (RPM), level, and status")]
    fn get_fan_status(&self) -> String {
        fs::read_to_string("/proc/acpi/ibm/fan")
            .unwrap_or_else(|e| format!("Error: {}", e))
    }

    #[tool(description = "Set ThinkPad fan speed. Values: 'auto', 'full-speed', or '0' through '7'")]
    fn set_fan_speed(&self, #[tool(aggr)] req: SetFanSpeedRequest) -> String {
        let valid = ["auto", "full-speed", "0", "1", "2", "3", "4", "5", "6", "7"];
        if !valid.contains(&req.speed.as_str()) {
            return format!("Invalid speed '{}'. Use: auto, full-speed, or 0-7", req.speed);
        }
        let command = format!("level {}", req.speed);
        if fs::write("/proc/acpi/ibm/fan", &command).is_ok() {
            return format!("Fan speed set to: {}", req.speed);
        }
        if std::path::Path::new(HELPER_PATH).exists() {
            match Command::new("pkexec").arg(HELPER_PATH).arg(&command).output() {
                Ok(o) if o.status.success() => return format!("Fan speed set to: {}", req.speed),
                Ok(o) => return format!("Failed: {}", String::from_utf8_lossy(&o.stderr)),
                Err(e) => return format!("Error: {}", e),
            }
        }
        "No permission. Grant permissions in ThinkUtils first.".into()
    }

    #[tool(description = "Get CPU temperature readings from all thermal zones")]
    fn get_cpu_temperature(&self) -> String {
        let mut temps = Vec::new();
        for i in 0..10 {
            let path = format!("/sys/class/thermal/thermal_zone{}/temp", i);
            let type_path = format!("/sys/class/thermal/thermal_zone{}/type", i);
            if let (Ok(t), Ok(n)) = (fs::read_to_string(&path), fs::read_to_string(&type_path)) {
                if let Ok(m) = t.trim().parse::<i32>() {
                    temps.push(format!("{}: {:.1}°C", n.trim(), m as f64 / 1000.0));
                }
            }
        }
        if temps.is_empty() { "No thermal zones found".into() } else { temps.join("\n") }
    }

    #[tool(description = "Get battery information: status, capacity, health, charge thresholds")]
    fn get_battery_info(&self) -> String {
        let bat = "/sys/class/power_supply/BAT0";
        if !std::path::Path::new(bat).exists() { return "No battery found".into(); }
        let r = |f: &str| fs::read_to_string(format!("{}/{}", bat, f))
            .map(|s| s.trim().to_string()).unwrap_or("N/A".into());
        format!(
            "Status: {}\nCapacity: {}%\nCycle Count: {}\nTechnology: {}\nStart Threshold: {}%\nStop Threshold: {}%",
            r("status"), r("capacity"), r("cycle_count"), r("technology"),
            r("charge_start_threshold"), r("charge_stop_threshold"),
        )
    }

    #[tool(description = "Set battery charge thresholds (start and stop percentages)")]
    fn set_battery_thresholds(&self, #[tool(aggr)] req: SetBatteryThresholdsRequest) -> String {
        if req.start >= req.stop { return "Start must be less than stop".into(); }
        if req.stop > 100 { return "Thresholds must be 0-100".into(); }
        let mut r = Vec::new();
        match fs::write("/sys/class/power_supply/BAT0/charge_stop_threshold", req.stop.to_string()) {
            Ok(_) => r.push(format!("Stop set to {}%", req.stop)),
            Err(e) => r.push(format!("Stop failed: {}", e)),
        }
        match fs::write("/sys/class/power_supply/BAT0/charge_start_threshold", req.start.to_string()) {
            Ok(_) => r.push(format!("Start set to {}%", req.start)),
            Err(e) => r.push(format!("Start failed: {}", e)),
        }
        r.join("\n")
    }

    #[tool(description = "Get CPU information: governor, frequency, turbo boost status")]
    fn get_cpu_info(&self) -> String {
        let mut info = Vec::new();
        if let Ok(v) = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor") {
            info.push(format!("Governor: {}", v.trim()));
        }
        if let Ok(v) = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq") {
            if let Ok(k) = v.trim().parse::<u64>() { info.push(format!("Frequency: {} MHz", k / 1000)); }
        }
        if let Ok(v) = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_available_governors") {
            info.push(format!("Available: {}", v.trim()));
        }
        if let Ok(v) = fs::read_to_string("/sys/devices/system/cpu/intel_pstate/no_turbo") {
            info.push(format!("Turbo Boost: {}", if v.trim() == "0" { "Enabled" } else { "Disabled" }));
        }
        if info.is_empty() { "No CPU info".into() } else { info.join("\n") }
    }

    #[tool(description = "Get system memory usage")]
    fn get_memory_info(&self) -> String {
        fs::read_to_string("/proc/meminfo")
            .map(|c| c.lines().take(5).collect::<Vec<_>>().join("\n"))
            .unwrap_or_else(|e| format!("Error: {}", e))
    }

    #[tool(description = "Get system information: hostname, kernel, OS, CPU model")]
    fn get_system_info(&self) -> String {
        let mut info = Vec::new();
        if let Ok(v) = fs::read_to_string("/etc/hostname") { info.push(format!("Hostname: {}", v.trim())); }
        if let Ok(v) = fs::read_to_string("/etc/os-release") {
            for l in v.lines() {
                if l.starts_with("PRETTY_NAME=") {
                    info.push(format!("OS: {}", l.trim_start_matches("PRETTY_NAME=").trim_matches('"')));
                    break;
                }
            }
        }
        if let Ok(v) = fs::read_to_string("/proc/cpuinfo") {
            for l in v.lines() {
                if let Some((_, rest)) = l.split_once("model name") {
                    if let Some((_, name)) = rest.split_once(':') {
                        info.push(format!("CPU: {}", name.trim()));
                        break;
                    }
                }
            }
        }
        if info.is_empty() { "No system info".into() } else { info.join("\n") }
    }
}

#[tool(tool_box)]
impl ServerHandler for ThinkUtilsHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "ThinkUtils MCP Server - Monitor and control ThinkPad hardware".into()
            ),
            ..Default::default()
        }
    }
}

// -- Tauri commands --

#[derive(Debug, Serialize, Deserialize)]
pub struct McpStatus {
    pub running: bool,
    pub host: String,
    pub port: u16,
}

#[tauri::command]
pub async fn get_mcp_status(state: tauri::State<'_, McpState>) -> Result<ApiResponse<McpStatus>, String> {
    let s = state.lock().await;
    Ok(ApiResponse {
        success: true,
        data: Some(McpStatus {
            running: s.cancel_token.is_some(),
            host: s.host.clone(),
            port: s.port,
        }),
        error: None,
    })
}

#[tauri::command]
pub async fn start_mcp_server(
    state: tauri::State<'_, McpState>,
    host: String,
    port: u16,
) -> Result<ApiResponse<String>, String> {
    let mut s = state.lock().await;

    if s.cancel_token.is_some() {
        return Ok(ApiResponse {
            success: false,
            data: None,
            error: Some("MCP server is already running".into()),
        });
    }

    // Validate host
    if host != "127.0.0.1" && host != "0.0.0.0" && host != "localhost" {
        return Ok(ApiResponse {
            success: false,
            data: None,
            error: Some("Host must be 127.0.0.1, 0.0.0.0, or localhost".into()),
        });
    }

    let bind_host = if host == "localhost" { "127.0.0.1" } else { &host };
    let addr: SocketAddr = format!("{}:{}", bind_host, port).parse()
        .map_err(|e| format!("Invalid address: {}", e))?;

    let ct = CancellationToken::new();
    let ct_clone = ct.clone();

    // Start the SSE server in a background task
    tokio::spawn(async move {
        println!("[MCP] Starting SSE server on http://{}/sse", addr);

        let sse_config = rmcp::transport::sse_server::SseServerConfig {
            bind: addr,
            sse_path: "/sse".to_string(),
            post_path: "/message".to_string(),
            ct: ct_clone.clone(),
        };

        match SseServer::serve_with_config(sse_config).await {
            Ok(mut server) => {
                loop {
                    tokio::select! {
                        _ = ct_clone.cancelled() => {
                            println!("[MCP] Server stopped");
                            break;
                        }
                        transport = server.next_transport() => {
                            match transport {
                                Some(t) => {
                                    let handler = ThinkUtilsHandler;
                                    tokio::spawn(async move {
                                        if let Ok(svc) = handler.serve(t).await {
                                            let _ = svc.waiting().await;
                                        }
                                    });
                                }
                                None => break,
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("[MCP] Failed to start: {}", e);
            }
        }
    });

    s.cancel_token = Some(ct);
    s.host = host.clone();
    s.port = port;

    println!("[MCP] Server started on {}:{}", host, port);
    Ok(ApiResponse {
        success: true,
        data: Some(format!("MCP server running on http://{}:{}/sse", host, port)),
        error: None,
    })
}

#[tauri::command]
pub async fn stop_mcp_server(state: tauri::State<'_, McpState>) -> Result<ApiResponse<String>, String> {
    let mut s = state.lock().await;

    if let Some(ct) = s.cancel_token.take() {
        ct.cancel();
        println!("[MCP] Server stopped");
        Ok(ApiResponse {
            success: true,
            data: Some("MCP server stopped".into()),
            error: None,
        })
    } else {
        Ok(ApiResponse {
            success: false,
            data: None,
            error: Some("MCP server is not running".into()),
        })
    }
}
