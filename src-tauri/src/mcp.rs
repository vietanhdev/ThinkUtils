use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ServerInfo;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{schemars, tool, tool_handler, tool_router, ServerHandler};
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

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

/// The path the Streamable HTTP transport is mounted at.
///
/// Exposed so the client-config snippets shown in the UI are generated from the
/// same value the router is built with. They were hardcoded to `/sse`, left over
/// from rmcp 0.1.5's SSE transport, and rmcp 2 serves nothing there — so anyone
/// pasting the displayed config got a 404 on every connection.
pub const MCP_PATH: &str = "/mcp";

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

#[tool_router]
impl ThinkUtilsHandler {
    #[tool(description = "Get ThinkPad fan status: speed (RPM), level, and status")]
    fn get_fan_status(&self) -> String {
        fs::read_to_string("/proc/acpi/ibm/fan").unwrap_or_else(|e| format!("Error: {}", e))
    }

    #[tool(
        description = "Set ThinkPad fan speed. Values: 'auto', 'full-speed', or '0' through '7'"
    )]
    fn set_fan_speed(&self, Parameters(req): Parameters<SetFanSpeedRequest>) -> String {
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
            r("charge_start_threshold"), r("charge_stop_threshold"),
        )
    }

    #[tool(description = "Set battery charge thresholds (start and stop percentages)")]
    fn set_battery_thresholds(
        &self,
        Parameters(req): Parameters<SetBatteryThresholdsRequest>,
    ) -> String {
        if let Some(err) = validate_battery_thresholds(req.start, req.stop) {
            return err;
        }
        let mut r = Vec::new();
        match fs::write(
            "/sys/class/power_supply/BAT0/charge_stop_threshold",
            req.stop.to_string(),
        ) {
            Ok(_) => r.push(format!("Stop set to {}%", req.stop)),
            Err(e) => r.push(format!("Stop failed: {}", e)),
        }
        match fs::write(
            "/sys/class/power_supply/BAT0/charge_start_threshold",
            req.start.to_string(),
        ) {
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

/// Hosts this server accepts in an inbound `Host` header.
///
/// rmcp defaults to loopback, which is what closes DNS rebinding. Naming the
/// port-qualified forms too, because a browser sends `Host: 127.0.0.1:8779`.
fn allowed_hosts(port: u16) -> Vec<String> {
    vec![
        "localhost".to_string(),
        format!("localhost:{}", port),
        "127.0.0.1".to_string(),
        format!("127.0.0.1:{}", port),
    ]
}

/// Browser origins this server accepts.
///
/// rmcp leaves `allowed_origins` EMPTY by default, and empty means Origin
/// validation is disabled entirely -- so this has to be set explicitly or the
/// cross-origin hole stays open. A Host check alone is not enough: a page can
/// fetch the loopback endpoint with mode:'no-cors', the Host header matches
/// because the request really is going to 127.0.0.1, and only the Origin reveals
/// that another site initiated it.
///
/// Listing only our own origins means a genuine MCP client -- which is not a
/// browser and sends no Origin at all -- still works, while anything originating
/// in a browser tab is rejected unless it is truly same-origin.
fn allowed_origins(port: u16) -> Vec<String> {
    vec![
        format!("http://127.0.0.1:{}", port),
        format!("http://localhost:{}", port),
    ]
}

fn build_http_config(port: u16, ct: CancellationToken) -> StreamableHttpServerConfig {
    StreamableHttpServerConfig::default()
        .with_cancellation_token(ct)
        .with_allowed_hosts(allowed_hosts(port))
        .with_allowed_origins(allowed_origins(port))
}

/// Written out rather than relying on `#[tool_router(server_handler)]`, which
/// generates a default ServerHandler and would silently drop the instructions
/// the client shows to its model.
#[tool_handler]
impl ServerHandler for ThinkUtilsHandler {
    fn get_info(&self) -> ServerInfo {
        // ServerInfo is non-exhaustive in rmcp 2, so build from Default and set
        // the one field we care about.
        let mut info = ServerInfo::default();
        info.instructions = Some(
            "ThinkUtils MCP Server - monitor and control ThinkPad hardware: fan speed and \
             status, CPU temperature and governor, battery info and charge thresholds."
                .into(),
        );
        info
    }
}

// -- Tauri commands --

#[derive(Debug, Serialize, Deserialize)]
pub struct McpStatus {
    pub running: bool,
    pub host: String,
    pub port: u16,
    /// Reported so the UI builds its client-config snippets from the path the
    /// router actually serves, rather than a second copy that can drift from it.
    pub path: String,
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
            path: MCP_PATH.to_string(),
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

    let config = build_http_config(port, ct_clone.clone());

    // Host and Origin allowlists. This is the whole reason for moving to rmcp 2:
    // 0.1.5's SSE server built its router internally and exposed no seam to add
    // either check, so a page the user visited could POST to loopback and invoke
    // tools.
    //
    // allowed_hosts defaults to loopback and closes DNS rebinding. allowed_origins
    // defaults to EMPTY, which DISABLES Origin validation entirely -- so it has to
    // be set explicitly. A Host check alone is not enough: a browser can fetch the
    // loopback endpoint with mode:'no-cors', the Host header matches, and only the
    // Origin reveals that the request came from another site.
    //
    // Listing just our own origins means a real MCP client (which sends no Origin
    // at all) still works, while anything originating in a browser tab is rejected
    // unless it is genuinely same-origin.

    // Bind BEFORE spawning. Binding inside the task left its error with nowhere
    // to go but eprintln!, while this function unconditionally reported success
    // and recorded a cancel token -- so an unusable port (already taken, or
    // privileged and EACCES) showed "Running" in the UI, and every retry was
    // refused with "already running" until the user pressed Stop.
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[MCP] Failed to bind {}: {}", addr, e);
            return Ok(ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Could not listen on {}: {}", addr, e)),
            });
        }
    };

    tokio::spawn(async move {
        println!(
            "[MCP] Starting Streamable HTTP server on http://{}/mcp",
            addr
        );

        let service = StreamableHttpService::new(
            || Ok(ThinkUtilsHandler),
            Arc::new(LocalSessionManager::default()),
            config,
        );

        let router = axum::Router::new().nest_service(MCP_PATH, service);

        let server = axum::serve(listener, router).with_graceful_shutdown(async move {
            ct_clone.cancelled().await;
            println!("[MCP] Server stopped");
        });

        if let Err(e) = server.await {
            eprintln!("[MCP] Server error: {}", e);
        }
    });

    s.cancel_token = Some(ct);
    s.host = host.clone();
    s.port = port;

    println!("[MCP] Server started on {}:{}", host, port);
    Ok(ApiResponse {
        success: true,
        data: Some(format!(
            "MCP server running on http://{}:{}/mcp",
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

    // -- Client-config endpoint --

    /// The UI and the docs tell users which URL to point their MCP client at.
    /// Both used to hardcode `/sse`, left over from rmcp 0.1.5's SSE transport,
    /// while rmcp 2 serves Streamable HTTP at `/mcp` and nothing at `/sse` — so
    /// every config copied out of the app 404'd on connect.
    ///
    /// The status payload now carries the path, and this pins that payload to
    /// the constant the router is built from.
    #[test]
    fn advertised_path_is_the_one_the_router_serves() {
        assert_eq!(MCP_PATH, "/mcp");
        assert!(
            MCP_PATH.starts_with('/'),
            "nest_service requires a leading slash"
        );
        assert_ne!(
            MCP_PATH, "/sse",
            "rmcp 2 removed the SSE server transport entirely"
        );
    }

    /// The source file must not reintroduce a second, hardcoded copy of the
    /// path. Scoped to code above the test module so this cannot match itself.
    #[test]
    fn router_path_is_not_hardcoded_alongside_the_constant() {
        let src = include_str!("mcp.rs");
        let code = src.split("#[cfg(test)]").next().unwrap();
        assert!(
            !code.contains("nest_service(\""),
            "nest_service should be given MCP_PATH, not a literal"
        );
    }

    // -- Host and Origin allowlists (the point of the rmcp 2 migration) --

    /// Any port; the allowlists are built from whatever the server binds.
    const TEST_PORT: u16 = 8779;

    /// A browser sends `Host: 127.0.0.1:8779`, not a bare `127.0.0.1`, so the
    /// port-qualified forms must be listed or every real request is rejected.
    #[test]
    fn allowed_hosts_cover_the_port_qualified_forms() {
        let hosts = allowed_hosts(TEST_PORT);
        for expected in [
            "localhost".to_string(),
            format!("localhost:{}", TEST_PORT),
            "127.0.0.1".to_string(),
            format!("127.0.0.1:{}", TEST_PORT),
        ] {
            assert!(hosts.contains(&expected), "missing host {}", expected);
        }
    }

    /// Loopback only. A routable host here would reintroduce the exposure that
    /// validate_mcp_host already refuses at bind time.
    #[test]
    fn allowed_hosts_are_loopback_only() {
        for host in allowed_hosts(TEST_PORT) {
            let bare = host.split(':').next().unwrap_or(&host).to_string();
            assert!(
                bare == "localhost" || bare == "127.0.0.1" || bare == "::1",
                "{} is not a loopback host",
                host
            );
        }
    }

    /// rmcp leaves allowed_origins empty by default, and empty DISABLES Origin
    /// validation entirely. If this list is ever emptied, a page the user visits
    /// can reach the server on loopback again -- the Host header matches, because
    /// the request really is going to 127.0.0.1. Only the Origin gives it away.
    #[test]
    fn allowed_origins_is_never_empty() {
        assert!(
            !allowed_origins(TEST_PORT).is_empty(),
            "an empty allowed_origins turns Origin validation OFF"
        );
    }

    /// rmcp matches per RFC 6454 on (scheme, host, port), so a schemeless entry
    /// silently matches nothing.
    #[test]
    fn allowed_origins_carry_a_scheme_and_the_right_port() {
        for origin in allowed_origins(TEST_PORT) {
            assert!(origin.starts_with("http://"), "{} has no scheme", origin);
            assert!(
                origin.ends_with(&format!(":{}", TEST_PORT)),
                "{} does not name the server port",
                origin
            );
        }
    }

    /// The threat this closes: a page on another site fetching loopback.
    #[test]
    fn a_foreign_origin_is_not_allowed() {
        let origins = allowed_origins(TEST_PORT);
        for hostile in [
            "https://evil.example".to_string(),
            "http://evil.example".to_string(),
            "null".to_string(),
            format!("http://evil.example:{}", TEST_PORT),
        ] {
            assert!(
                !origins.contains(&hostile),
                "{} must not be allowed",
                hostile
            );
        }
    }

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

    #[test]
    fn default_state() {
        let state = McpServerState::default();
        assert_eq!(state.host, "127.0.0.1");
        assert_eq!(state.port, 8765);
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
