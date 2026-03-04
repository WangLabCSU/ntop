use anyhow::Result;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct NetworkStats {
    pub interface: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub rx_dropped: u64,
    pub tx_dropped: u64,
}

#[derive(Debug, Clone, Default)]
pub struct NetworkStatsDelta {
    pub interface: String,
    pub rx_bytes_sec: f64,
    pub tx_bytes_sec: f64,
    pub rx_packets_sec: f64,
    pub tx_packets_sec: f64,
}

pub struct NetworkCollector {
    last_stats: Vec<NetworkStats>,
    last_time: std::time::Instant,
}

impl NetworkCollector {
    pub fn new() -> Self {
        Self {
            last_stats: Vec::new(),
            last_time: std::time::Instant::now(),
        }
    }

    pub fn read_dev_stats() -> Result<Vec<NetworkStats>> {
        let path = Path::new("/proc/net/dev");
        let content = fs::read_to_string(path)?;

        let mut stats = Vec::new();

        for line in content.lines().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 17 {
                continue;
            }

            let interface = parts[0].trim_end_matches(':').to_string();

            if interface == "lo" {
                continue;
            }

            let stats_entry = NetworkStats {
                interface,
                rx_bytes: parts[1].parse().unwrap_or(0),
                rx_packets: parts[2].parse().unwrap_or(0),
                rx_errors: parts[3].parse().unwrap_or(0),
                rx_dropped: parts[4].parse().unwrap_or(0),
                tx_bytes: parts[9].parse().unwrap_or(0),
                tx_packets: parts[10].parse().unwrap_or(0),
                tx_errors: parts[11].parse().unwrap_or(0),
                tx_dropped: parts[12].parse().unwrap_or(0),
            };

            stats.push(stats_entry);
        }

        Ok(stats)
    }

    pub fn collect(&mut self) -> Result<(Vec<NetworkStats>, Vec<NetworkStatsDelta>)> {
        let current_stats = Self::read_dev_stats()?;
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_time).as_secs_f64();
        let is_first_run = self.last_stats.is_empty();

        let mut deltas = Vec::new();

        // Always return all interfaces, with 0 rates on first run
        for current in &current_stats {
            let (rx_sec, tx_sec, rx_pkt_sec, tx_pkt_sec) = if is_first_run || elapsed <= 0.0 {
                (0.0, 0.0, 0.0, 0.0)
            } else if let Some(last) = self
                .last_stats
                .iter()
                .find(|s| s.interface == current.interface)
            {
                (
                    (current.rx_bytes.saturating_sub(last.rx_bytes)) as f64 / elapsed,
                    (current.tx_bytes.saturating_sub(last.tx_bytes)) as f64 / elapsed,
                    (current.rx_packets.saturating_sub(last.rx_packets)) as f64 / elapsed,
                    (current.tx_packets.saturating_sub(last.tx_packets)) as f64 / elapsed,
                )
            } else {
                (0.0, 0.0, 0.0, 0.0)
            };

            deltas.push(NetworkStatsDelta {
                interface: current.interface.clone(),
                rx_bytes_sec: rx_sec,
                tx_bytes_sec: tx_sec,
                rx_packets_sec: rx_pkt_sec,
                tx_packets_sec: tx_pkt_sec,
            });
        }

        self.last_stats = current_stats.clone();
        self.last_time = now;

        Ok((current_stats, deltas))
    }
}

pub fn format_bytes(bytes: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes >= GB {
        format!("{:.2} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes / KB)
    } else {
        format!("{:.0} B", bytes)
    }
}

pub fn format_bytes_per_sec(bytes_sec: f64) -> String {
    format!("{}/s", format_bytes(bytes_sec))
}
