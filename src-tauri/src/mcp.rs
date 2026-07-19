use rmcp::transport::sse_server::SseServer;
use rmcp::{model::ServerInfo, schemars, tool, ServerHandler, ServiceExt};
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// Default port for the MCP server.
///
/// Deliberately not 8765: that is sync.rs's OAuth callback port, and while the
/// MCP server held it the callback listener could not bind, so Google login
/// failed with no visible error. A test asserts the two stay different.
pub const DEFAULT_MCP_PORT: u16 = 8779;

const VALID_FAN_SPEEDS: &[&str] = &["auto", "full-speed", "0", "1", "2", "3", "4", "5", "6", "7"];

fn validate_fan_speed(speed: &str) -> Option<String> {
    if VALID_FAN_SPEEDS.contains(&speed) {
        None
    } else {
        Some(format!(
            "Invalid speed '{}'. Use: auto, full-speed, or 0-7",
            speed
        ))
    }
}

fn validate_battery_thresholds(start: u32, stop: u32) -> Option<String> {
    if start >= stop {
        Some("Start must be less than stop".into())
    } else if stop > 100 {
        Some("Thresholds must be 0-100".into())
    } else {
        None
    }
}

/// Loopback only. `0.0.0.0` is deliberately rejected: this server exposes fan and
/// battery control with no authentication, so binding it to a routable interface
/// hands hardware control to anyone on the network.
///
/// Note this does NOT close the DNS-rebinding / cross-origin CSRF vector against
/// loopback — a malicious page can still POST to 127.0.0.1 from the user's browser.
/// Closing that needs Host+Origin allowlisting, which rmcp 0.1.5 gives us no seam
/// to add (SseServer::serve_with_config builds its router internally). See task #1.
fn validate_mcp_host(host: &str) -> Option<String> {
    if host != "127.0.0.1" && host != "localhost" {
        Some("Host must be 127.0.0.1 or localhost. Binding to other interfaces is not allowed: the MCP server is unauthenticated.".into())
    } else {
        None
    }
}

fn resolve_bind_host(host: &str) -> &str {
    if host == "localhost" {
        "127.0.0.1"
    } else {
        host
    }
}

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
            port: DEFAULT_MCP_PORT,
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
        fs::read_to_string("/proc/acpi/ibm/fan").unwrap_or_else(|e| format!("Error: {}", e))
    }

    #[tool(
        description = "Set ThinkPad fan speed. Values: 'auto', 'full-speed', or '0' through '7'"
    )]
    fn set_fan_speed(&self, #[tool(aggr)] req: SetFanSpeedRequest) -> String {
        if let Some(err) = validate_fan_speed(&req.speed) {
            return err;
        }
        let command = format!("level {}", req.speed);
        if fs::write("/proc/acpi/ibm/fan", &command).is_ok() {
            return format!("Fan speed set to: {}", req.speed);
        }
        if let Some(helper) = crate::fan_control::helper_path() {
            match Command::new("pkexec").arg(helper).arg(&command).output() {
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
        if temps.is_empty() {
            "No thermal zones found".into()
        } else {
            temps.join("\n")
        }
    }

    #[tool(description = "Get battery information: status, capacity, health, charge thresholds")]
    fn get_battery_info(&self) -> String {
        let bat = "/sys/class/power_supply/BAT0";
        if !std::path::Path::new(bat).exists() {
            return "No battery found".into();
        }
        let r = |f: &str| {
            fs::read_to_string(format!("{}/{}", bat, f))
                .map(|s| s.trim().to_string())
                .unwrap_or("N/A".into())
        };
        format!(
            "Status: {}\nCapacity: {}%\nCycle Count: {}\nTechnology: {}\nStart Threshold: {}%\nStop Threshold: {}%",
            r("status"), r("capacity"), r("cycle_count"), r("technology"),
            r("charge_control_start_threshold"), r("charge_control_end_threshold"),
        )
    }

    #[tool(description = "Set battery charge thresholds (start and stop percentages)")]
    fn set_battery_thresholds(&self, #[tool(aggr)] req: SetBatteryThresholdsRequest) -> String {
        if let Some(err) = validate_battery_thresholds(req.start, req.stop) {
            return err;
        }
        // Resolved rather than hardcoded: the attribute names differ between the
        // generic kernel API and thinkpad_acpi's older spelling, and this module
        // used to name a different pair than battery.rs.
        let Some((start_path, stop_path)) = crate::battery::threshold_paths() else {
            return "This machine exposes no battery charge threshold controls.".to_string();
        };

        let mut r = Vec::new();
        match fs::write(&stop_path, req.stop.to_string()) {
            Ok(_) => r.push(format!("Stop set to {}%", req.stop)),
            Err(e) => r.push(format!("Stop failed: {}", e)),
        }
        match fs::write(&start_path, req.start.to_string()) {
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
            if let Ok(k) = v.trim().parse::<u64>() {
                info.push(format!("Frequency: {} MHz", k / 1000));
            }
        }
        if let Ok(v) =
            fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_available_governors")
        {
            info.push(format!("Available: {}", v.trim()));
        }
        if let Ok(v) = fs::read_to_string("/sys/devices/system/cpu/intel_pstate/no_turbo") {
            info.push(format!(
                "Turbo Boost: {}",
                if v.trim() == "0" {
                    "Enabled"
                } else {
                    "Disabled"
                }
            ));
        }
        if info.is_empty() {
            "No CPU info".into()
        } else {
            info.join("\n")
        }
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
        if let Ok(v) = fs::read_to_string("/etc/hostname") {
            info.push(format!("Hostname: {}", v.trim()));
        }
        if let Ok(v) = fs::read_to_string("/etc/os-release") {
            for l in v.lines() {
                if l.starts_with("PRETTY_NAME=") {
                    info.push(format!(
                        "OS: {}",
                        l.trim_start_matches("PRETTY_NAME=").trim_matches('"')
                    ));
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
        if info.is_empty() {
            "No system info".into()
        } else {
            info.join("\n")
        }
    }
}

#[tool(tool_box)]
impl ServerHandler for ThinkUtilsHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "ThinkUtils MCP Server - Monitor and control ThinkPad hardware".into(),
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
pub async fn get_mcp_status(
    state: tauri::State<'_, McpState>,
) -> Result<ApiResponse<McpStatus>, String> {
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

    if let Some(err) = validate_mcp_host(&host) {
        return Ok(ApiResponse {
            success: false,
            data: None,
            error: Some(err),
        });
    }

    let bind_host = resolve_bind_host(&host);
    let addr: SocketAddr = format!("{}:{}", bind_host, port)
        .parse()
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
            Ok(mut server) => loop {
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
            },
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
        data: Some(format!(
            "MCP server running on http://{}:{}/sse",
            host, port
        )),
        error: None,
    })
}

#[tauri::command]
pub async fn stop_mcp_server(
    state: tauri::State<'_, McpState>,
) -> Result<ApiResponse<String>, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    // -- Fan speed validation --

    #[test]
    fn valid_fan_speeds_accepted() {
        for speed in VALID_FAN_SPEEDS {
            assert!(
                validate_fan_speed(speed).is_none(),
                "expected '{}' to be valid",
                speed
            );
        }
    }

    #[test]
    fn invalid_fan_speeds_rejected() {
        for speed in &["8", "-1", "turbo", "", "Auto", "FULL-SPEED"] {
            let err = validate_fan_speed(speed);
            assert!(err.is_some(), "expected '{}' to be invalid", speed);
            assert!(err.unwrap().contains(speed));
        }
    }

    // -- Battery threshold validation --

    #[test]
    fn valid_battery_thresholds() {
        assert!(validate_battery_thresholds(40, 80).is_none());
        assert!(validate_battery_thresholds(0, 100).is_none());
        assert!(validate_battery_thresholds(0, 1).is_none());
    }

    #[test]
    fn battery_start_must_be_less_than_stop() {
        assert_eq!(
            validate_battery_thresholds(80, 80).unwrap(),
            "Start must be less than stop"
        );
        assert_eq!(
            validate_battery_thresholds(90, 80).unwrap(),
            "Start must be less than stop"
        );
    }

    #[test]
    fn battery_stop_must_be_at_most_100() {
        assert_eq!(
            validate_battery_thresholds(50, 101).unwrap(),
            "Thresholds must be 0-100"
        );
    }

    // -- MCP host validation --

    #[test]
    fn valid_mcp_hosts() {
        assert!(validate_mcp_host("127.0.0.1").is_none());
        assert!(validate_mcp_host("localhost").is_none());
    }

    #[test]
    fn invalid_mcp_hosts_rejected() {
        for host in &["192.168.1.1", "10.0.0.1", "example.com", ""] {
            assert!(
                validate_mcp_host(host).is_some(),
                "expected '{}' to be rejected",
                host
            );
        }
    }

    /// The MCP server is unauthenticated and exposes fan/battery control, so it
    /// must never be bindable to a routable interface. Regression guard.
    #[test]
    fn wildcard_bind_is_rejected() {
        assert!(validate_mcp_host("0.0.0.0").is_some());
        assert!(validate_mcp_host("::").is_some());
    }

    // -- Bind host resolution --

    #[test]
    fn localhost_resolves_to_ip() {
        assert_eq!(resolve_bind_host("localhost"), "127.0.0.1");
    }

    #[test]
    fn ip_hosts_pass_through() {
        assert_eq!(resolve_bind_host("127.0.0.1"), "127.0.0.1");
        assert_eq!(resolve_bind_host("0.0.0.0"), "0.0.0.0");
    }

    // -- McpServerState defaults --

    /// The MCP server and the OAuth callback listener cannot both bind the same
    /// port, and the failure is silent: with MCP running, the callback server
    /// fails to bind and Google login simply never completes. They used to share
    /// 8765.
    #[test]
    fn mcp_port_does_not_collide_with_the_oauth_callback() {
        assert_ne!(
            DEFAULT_MCP_PORT,
            crate::sync::OAUTH_CALLBACK_PORT,
            "MCP and the OAuth callback would fight over the same port"
        );
    }

    #[test]
    fn default_state() {
        let state = McpServerState::default();
        assert_eq!(state.host, "127.0.0.1");
        assert_eq!(state.port, DEFAULT_MCP_PORT);
        assert!(state.cancel_token.is_none());
    }

    // -- ApiResponse serialization --

    #[test]
    fn api_response_success_serialization() {
        let resp = ApiResponse {
            success: true,
            data: Some("ok".to_string()),
            error: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["data"], "ok");
        assert!(json["error"].is_null());
    }

    #[test]
    fn api_response_error_serialization() {
        let resp: ApiResponse<String> = ApiResponse {
            success: false,
            data: None,
            error: Some("something broke".into()),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["success"], false);
        assert!(json["data"].is_null());
        assert_eq!(json["error"], "something broke");
    }
}
