use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use users::get_user_by_uid;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub user: String,
    pub uid: u32,
    pub connections: usize,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub read_bytes_sec: f64,
    pub write_bytes_sec: f64,
    pub cpu_percent: f64,
    pub mem_percent: f64,
    pub state: String,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct ProcessDelta {
    pub pid: u32,
    pub name: String,
    pub user: String,
    pub connections: usize,
    pub read_bytes_sec: f64,
    pub write_bytes_sec: f64,
    pub cpu_percent: f64,
    pub mem_percent: f64,
    pub state: String,
}

pub struct ProcessCollector {
    last_io: HashMap<u32, (u64, u64)>,
    last_cpu: HashMap<u32, (u64, u64, std::time::Instant)>,
    last_time: std::time::Instant,
    total_memory_kb: u64,
    clock_tick: u64,
}

impl Default for ProcessCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessCollector {
    pub fn new() -> Self {
        Self {
            last_io: HashMap::new(),
            last_cpu: HashMap::new(),
            last_time: std::time::Instant::now(),
            total_memory_kb: Self::get_total_memory(),
            clock_tick: Self::get_clock_tick(),
        }
    }

    fn get_total_memory() -> u64 {
        if let Ok(content) = fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    return line
                        .split(':')
                        .nth(1)
                        .and_then(|s| s.split_whitespace().next())
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                }
            }
        }
        0
    }

    fn get_clock_tick() -> u64 {
        unsafe { libc::sysconf(libc::_SC_CLK_TCK) as u64 }.max(1)
    }

    fn get_process_name(pid: u32) -> String {
        fs::read_to_string(format!("/proc/{}/comm", pid))
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    fn get_process_user(pid: u32) -> (String, u32) {
        if let Ok(content) = fs::read_to_string(format!("/proc/{}/status", pid)) {
            for line in content.lines() {
                if line.starts_with("Uid:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(uid) = parts[1].parse::<u32>() {
                            let user_name = get_user_by_uid(uid)
                                .map(|u| u.name().to_string_lossy().to_string())
                                .unwrap_or_else(|| uid.to_string());
                            return (user_name, uid);
                        }
                    }
                }
            }
        }
        ("unknown".to_string(), 0)
    }

    fn get_process_io(pid: u32) -> (u64, u64) {
        if let Ok(content) = fs::read_to_string(format!("/proc/{}/io", pid)) {
            let mut read_bytes = 0u64;
            let mut write_bytes = 0u64;
            for line in content.lines() {
                if line.starts_with("read_bytes:") {
                    read_bytes = line
                        .split(':')
                        .nth(1)
                        .and_then(|s| s.trim().parse().ok())
                        .unwrap_or(0);
                } else if line.starts_with("write_bytes:") {
                    write_bytes = line
                        .split(':')
                        .nth(1)
                        .and_then(|s| s.trim().parse().ok())
                        .unwrap_or(0);
                }
            }
            return (read_bytes, write_bytes);
        }
        (0, 0)
    }

    fn get_socket_connections(pid: u32) -> usize {
        let fd_path = format!("/proc/{}/fd", pid);
        if let Ok(fd_dir) = fs::read_dir(&fd_path) {
            return fd_dir
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    if let Ok(link) = fs::read_link(e.path()) {
                        let link_str = link.to_string_lossy().to_string();
                        if link_str.starts_with("socket:[") {
                            return Some(());
                        }
                    }
                    None
                })
                .count();
        }
        0
    }

    fn get_all_pids() -> Vec<u32> {
        let mut pids = Vec::new();
        if let Ok(entries) = fs::read_dir("/proc") {
            for entry in entries.flatten() {
                if let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() {
                    pids.push(pid);
                }
            }
        }
        pids
    }

    fn get_process_stat(pid: u32) -> (u64, u64, u64, String) {
        if let Ok(content) = fs::read_to_string(format!("/proc/{}/stat", pid)) {
            let parts: Vec<&str> = content.split_whitespace().collect();
            if parts.len() >= 17 {
                let utime: u64 = parts[13].parse().unwrap_or(0);
                let stime: u64 = parts[14].parse().unwrap_or(0);
                let rss: u64 = parts[23].parse().unwrap_or(0);
                // Process state is at index 2 (after pid and comm)
                let state = parts.get(2).unwrap_or(&"?").to_string();
                return (utime, stime, rss, state);
            }
        }
        (0, 0, 0, "?".to_string())
    }

    fn format_state(state: &str) -> &'static str {
        match state {
            "R" => "Running",
            "S" => "Sleeping",
            "D" => "Disk Sleep",
            "T" => "Stopped",
            "t" => "Tracing Stop",
            "X" => "Dead",
            "Z" => "Zombie",
            "P" => "Parked",
            "I" => "Idle",
            _ => "Unknown",
        }
    }

    pub fn collect(&mut self) -> Result<Vec<ProcessInfo>> {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_time).as_secs_f64();
        let mut processes = Vec::new();
        let mut current_io = HashMap::new();
        let mut current_cpu = HashMap::new();

        for pid in Self::get_all_pids() {
            let name = Self::get_process_name(pid);
            let (user, uid) = Self::get_process_user(pid);
            let (read_bytes, write_bytes) = Self::get_process_io(pid);
            let connections = Self::get_socket_connections(pid);
            let (utime, stime, rss, state_code) = Self::get_process_stat(pid);
            let state = Self::format_state(&state_code);

            let (read_sec, write_sec) = if elapsed > 0.0 {
                if let Some(&(last_read, last_write)) = self.last_io.get(&pid) {
                    (
                        (read_bytes.saturating_sub(last_read)) as f64 / elapsed,
                        (write_bytes.saturating_sub(last_write)) as f64 / elapsed,
                    )
                } else {
                    (0.0, 0.0)
                }
            } else {
                (0.0, 0.0)
            };

            let page_size_kb = 4.0_f64;
            let mem_percent = if self.total_memory_kb > 0 {
                (rss as f64 * page_size_kb / self.total_memory_kb as f64) * 100.0
            } else {
                0.0
            };

            // Calculate CPU percentage
            // utime and stime are in clock ticks
            // Formula: (delta_ticks / clock_tick) / elapsed_time * 100
            // This gives percentage of one CPU core. For multi-threaded processes,
            // this can exceed 100% (e.g., 400% means using 4 cores fully)
            let cpu_percent =
                if let Some((last_utime, last_stime, last_time)) = self.last_cpu.get(&pid) {
                    let time_elapsed = now.duration_since(*last_time).as_secs_f64();
                    if time_elapsed > 0.0 && self.clock_tick > 0 {
                        let delta_utime = utime.saturating_sub(*last_utime) as f64;
                        let delta_stime = stime.saturating_sub(*last_stime) as f64;
                        let delta_ticks = delta_utime + delta_stime;
                        // Convert ticks to seconds, then to percentage
                        let cpu_time = delta_ticks / self.clock_tick as f64;
                        (cpu_time / time_elapsed) * 100.0
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };

            current_io.insert(pid, (read_bytes, write_bytes));
            current_cpu.insert(pid, (utime, stime, now));

            processes.push(ProcessInfo {
                pid,
                name,
                user,
                uid,
                connections,
                read_bytes,
                write_bytes,
                read_bytes_sec: read_sec,
                write_bytes_sec: write_sec,
                // Cap at a reasonable max (e.g., 64 cores * 100% = 6400%)
                cpu_percent: cpu_percent.min(6400.0),
                mem_percent: mem_percent.min(100.0),
                state: state.to_string(),
            });
        }

        self.last_io = current_io;
        self.last_cpu = current_cpu;
        self.last_time = now;

        Ok(processes)
    }

    pub fn collect_delta(&mut self) -> Result<Vec<ProcessDelta>> {
        let processes = self.collect()?;

        Ok(processes
            .into_iter()
            .map(|p| ProcessDelta {
                pid: p.pid,
                name: p.name,
                user: p.user,
                connections: p.connections,
                read_bytes_sec: p.read_bytes_sec,
                write_bytes_sec: p.write_bytes_sec,
                cpu_percent: p.cpu_percent,
                mem_percent: p.mem_percent,
                state: p.state,
            })
            .collect())
    }
}
