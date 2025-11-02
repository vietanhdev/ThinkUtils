use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::thread;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemMonitor {
    pub cpu: CpuStats,
    pub memory: MemoryStats,
    pub disk: Vec<DiskStats>,
    pub network: Vec<NetworkStats>,
    pub processes: Vec<ProcessInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CpuStats {
    pub usage_percent: f64,
    pub cores: Vec<CoreStats>,
    pub load_avg: LoadAverage,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoreStats {
    pub core_id: usize,
    pub usage_percent: f64,
    pub frequency: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoadAverage {
    pub one_min: f64,
    pub five_min: f64,
    pub fifteen_min: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub usage_percent: f64,
    pub swap_total: u64,
    pub swap_used: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiskStats {
    pub device: String,
    pub mount_point: String,
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub usage_percent: f64,
    pub filesystem: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkStats {
    pub interface: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f64,
    pub memory_mb: f64,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[tauri::command]
pub fn get_system_monitor() -> ApiResponse<SystemMonitor> {
    match collect_system_stats() {
        Ok(monitor) => ApiResponse {
            success: true,
            data: Some(monitor),
            error: None,
        },
        Err(e) => ApiResponse {
            success: false,
            data: None,
            error: Some(e),
        },
    }
}

fn collect_system_stats() -> Result<SystemMonitor, String> {
    Ok(SystemMonitor {
        cpu: get_cpu_stats()?,
        memory: get_memory_stats()?,
        disk: get_disk_stats()?,
        network: get_network_stats()?,
        processes: get_top_processes()?,
    })
}

fn get_cpu_stats() -> Result<CpuStats, String> {
    // Take first snapshot
    let stat1 = fs::read_to_string("/proc/stat")
        .map_err(|e| format!("Failed to read /proc/stat: {}", e))?;
    
    let snapshot1 = parse_cpu_snapshot(&stat1)?;
    
    // Wait 200ms for measurable difference
    thread::sleep(Duration::from_millis(200));
    
    // Take second snapshot
    let stat2 = fs::read_to_string("/proc/stat")
        .map_err(|e| format!("Failed to read /proc/stat: {}", e))?;
    
    let snapshot2 = parse_cpu_snapshot(&stat2)?;
    
    // Calculate usage from deltas
    let total_usage = calculate_cpu_usage_from_snapshots(&snapshot1.total, &snapshot2.total);
    
    let mut cores = Vec::new();
    for (i, (core1, core2)) in snapshot1.cores.iter().zip(snapshot2.cores.iter()).enumerate() {
        let usage = calculate_cpu_usage_from_snapshots(core1, core2);
        let freq = get_core_frequency(i);
        cores.push(CoreStats {
            core_id: i,
            usage_percent: usage,
            frequency: freq,
        });
    }
    
    let load_avg = get_load_average()?;
    
    Ok(CpuStats {
        usage_percent: total_usage,
        cores,
        load_avg,
    })
}

#[derive(Debug)]
struct CpuSnapshot {
    total: CpuTimes,
    cores: Vec<CpuTimes>,
}

#[derive(Debug)]
struct CpuTimes {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
}

fn parse_cpu_snapshot(stat: &str) -> Result<CpuSnapshot, String> {
    let mut total = None;
    let mut cores = Vec::new();
    
    for line in stat.lines() {
        if line.starts_with("cpu ") {
            total = Some(parse_cpu_line(line)?);
        } else if line.starts_with("cpu") && line.len() > 3 {
            if let Some(core_id) = line.strip_prefix("cpu") {
                if core_id.split_whitespace().next().unwrap_or("").parse::<usize>().is_ok() {
                    cores.push(parse_cpu_line(line)?);
                }
            }
        }
    }
    
    Ok(CpuSnapshot {
        total: total.ok_or("No total CPU line found")?,
        cores,
    })
}

fn parse_cpu_line(line: &str) -> Result<CpuTimes, String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 8 {
        return Err("Invalid CPU line format".to_string());
    }
    
    Ok(CpuTimes {
        user: parts[1].parse().unwrap_or(0),
        nice: parts[2].parse().unwrap_or(0),
        system: parts[3].parse().unwrap_or(0),
        idle: parts[4].parse().unwrap_or(0),
        iowait: parts[5].parse().unwrap_or(0),
        irq: parts[6].parse().unwrap_or(0),
        softirq: parts[7].parse().unwrap_or(0),
        steal: if parts.len() > 8 { parts[8].parse().unwrap_or(0) } else { 0 },
    })
}

fn calculate_cpu_usage_from_snapshots(before: &CpuTimes, after: &CpuTimes) -> f64 {
    let idle_delta = after.idle.saturating_sub(before.idle) + after.iowait.saturating_sub(before.iowait);
    let total_delta = 
        after.user.saturating_sub(before.user) +
        after.nice.saturating_sub(before.nice) +
        after.system.saturating_sub(before.system) +
        after.idle.saturating_sub(before.idle) +
        after.iowait.saturating_sub(before.iowait) +
        after.irq.saturating_sub(before.irq) +
        after.softirq.saturating_sub(before.softirq) +
        after.steal.saturating_sub(before.steal);
    
    if total_delta == 0 {
        return 0.0;
    }
    
    let active_delta = total_delta - idle_delta;
    (active_delta as f64 / total_delta as f64) * 100.0
}

fn get_core_frequency(core_id: usize) -> u64 {
    let path = format!("/sys/devices/system/cpu/cpu{}/cpufreq/scaling_cur_freq", core_id);
    fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0)
}

fn get_load_average() -> Result<LoadAverage, String> {
    let loadavg = fs::read_to_string("/proc/loadavg")
        .map_err(|e| format!("Failed to read /proc/loadavg: {}", e))?;
    
    let parts: Vec<&str> = loadavg.split_whitespace().collect();
    if parts.len() < 3 {
        return Err("Invalid loadavg format".to_string());
    }
    
    Ok(LoadAverage {
        one_min: parts[0].parse().unwrap_or(0.0),
        five_min: parts[1].parse().unwrap_or(0.0),
        fifteen_min: parts[2].parse().unwrap_or(0.0),
    })
}

fn get_memory_stats() -> Result<MemoryStats, String> {
    let meminfo = fs::read_to_string("/proc/meminfo")
        .map_err(|e| format!("Failed to read /proc/meminfo: {}", e))?;
    
    let mut mem_map: HashMap<String, u64> = HashMap::new();
    
    for line in meminfo.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let value_str = value.trim().split_whitespace().next().unwrap_or("0");
            if let Ok(val) = value_str.parse::<u64>() {
                mem_map.insert(key.to_string(), val);
            }
        }
    }
    
    let total = mem_map.get("MemTotal").copied().unwrap_or(0);
    let available = mem_map.get("MemAvailable").copied().unwrap_or(0);
    let used = total.saturating_sub(available);
    let usage_percent = if total > 0 {
        (used as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    
    let swap_total = mem_map.get("SwapTotal").copied().unwrap_or(0);
    let swap_free = mem_map.get("SwapFree").copied().unwrap_or(0);
    let swap_used = swap_total.saturating_sub(swap_free);
    
    Ok(MemoryStats {
        total,
        used,
        available,
        usage_percent,
        swap_total,
        swap_used,
    })
}

fn get_disk_stats() -> Result<Vec<DiskStats>, String> {
    let output = Command::new("df")
        .args(&["-B1", "-T", "-x", "tmpfs", "-x", "devtmpfs"])
        .output()
        .map_err(|e| format!("Failed to run df: {}", e))?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut disks = Vec::new();
    
    for (i, line) in stdout.lines().enumerate() {
        if i == 0 {
            continue; // Skip header
        }
        
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 7 {
            continue;
        }
        
        let device = parts[0].to_string();
        let filesystem = parts[1].to_string();
        let total: u64 = parts[2].parse().unwrap_or(0);
        let used: u64 = parts[3].parse().unwrap_or(0);
        let available: u64 = parts[4].parse().unwrap_or(0);
        let usage_str = parts[5].trim_end_matches('%');
        let usage_percent: f64 = usage_str.parse().unwrap_or(0.0);
        let mount_point = parts[6].to_string();
        
        disks.push(DiskStats {
            device,
            mount_point,
            total,
            used,
            available,
            usage_percent,
            filesystem,
        });
    }
    
    Ok(disks)
}

fn get_network_stats() -> Result<Vec<NetworkStats>, String> {
    let net_dev = fs::read_to_string("/proc/net/dev")
        .map_err(|e| format!("Failed to read /proc/net/dev: {}", e))?;
    
    let mut interfaces = Vec::new();
    
    for (i, line) in net_dev.lines().enumerate() {
        if i < 2 {
            continue; // Skip headers
        }
        
        if let Some((iface, stats)) = line.split_once(':') {
            let iface = iface.trim().to_string();
            
            // Skip loopback
            if iface == "lo" {
                continue;
            }
            
            let parts: Vec<&str> = stats.split_whitespace().collect();
            if parts.len() < 10 {
                continue;
            }
            
            interfaces.push(NetworkStats {
                interface: iface,
                rx_bytes: parts[0].parse().unwrap_or(0),
                tx_bytes: parts[8].parse().unwrap_or(0),
                rx_packets: parts[1].parse().unwrap_or(0),
                tx_packets: parts[9].parse().unwrap_or(0),
            });
        }
    }
    
    Ok(interfaces)
}

fn get_top_processes() -> Result<Vec<ProcessInfo>, String> {
    let output = Command::new("ps")
        .args(&["aux", "--sort=-%cpu"])
        .output()
        .map_err(|e| format!("Failed to run ps: {}", e))?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut processes = Vec::new();
    
    for (i, line) in stdout.lines().enumerate() {
        if i == 0 || processes.len() >= 10 {
            continue; // Skip header or limit to top 10
        }
        
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 11 {
            continue;
        }
        
        let pid: u32 = parts[1].parse().unwrap_or(0);
        let cpu_percent: f64 = parts[2].parse().unwrap_or(0.0);
        let mem_percent: f64 = parts[3].parse().unwrap_or(0.0);
        let status = parts[7].to_string();
        let name = parts[10].to_string();
        
        // Calculate memory in MB (approximate)
        let memory_mb = mem_percent * 0.01 * get_total_memory_mb();
        
        processes.push(ProcessInfo {
            pid,
            name,
            cpu_percent,
            memory_mb,
            status,
        });
    }
    
    Ok(processes)
}

fn get_total_memory_mb() -> f64 {
    fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|line| line.starts_with("MemTotal:"))
                .and_then(|line| {
                    line.split_whitespace()
                        .nth(1)
                        .and_then(|kb| kb.parse::<f64>().ok())
                        .map(|kb| kb / 1024.0)
                })
        })
        .unwrap_or(8192.0)
}
