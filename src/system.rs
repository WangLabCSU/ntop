use anyhow::Result;
use std::fs;

#[derive(Debug, Clone, Default)]
pub struct SystemInfo {
    pub cpu_cores: usize,
    pub cpu_threads: usize,
    pub cpu_model: String,
    pub total_memory_kb: u64,
    pub used_memory_kb: u64,
    pub total_swap_kb: u64,
    pub used_swap_kb: u64,
    pub load_avg_1m: f64,
    pub load_avg_5m: f64,
    pub load_avg_15m: f64,
}

impl SystemInfo {
    pub fn collect() -> Result<Self> {
        let mut info = SystemInfo::default();

        // 读取 CPU 信息
        if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
            let mut core_ids = std::collections::HashSet::new();
            let mut physical_ids = std::collections::HashSet::new();

            for line in cpuinfo.lines() {
                if line.starts_with("processor") {
                    info.cpu_threads += 1;
                } else if line.starts_with("core id") {
                    if let Some(id) = line.split(':').nth(1) {
                        core_ids.insert(id.trim().to_string());
                    }
                } else if line.starts_with("physical id") {
                    if let Some(id) = line.split(':').nth(1) {
                        physical_ids.insert(id.trim().to_string());
                    }
                } else if line.starts_with("model name") && info.cpu_model.is_empty() {
                    if let Some(model) = line.split(':').nth(1) {
                        info.cpu_model = model.trim().to_string();
                    }
                }
            }

            // 计算核心数
            info.cpu_cores = if physical_ids.is_empty() {
                core_ids.len().max(1)
            } else {
                physical_ids.len() * core_ids.len().max(1)
            };
        }

        // 读取内存信息
        if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
            let mut total_mem: u64 = 0;
            let mut available_mem: u64 = 0;
            let mut free_mem: u64 = 0;
            let mut buffers: u64 = 0;
            let mut cached: u64 = 0;
            let mut total_swap: u64 = 0;
            let mut free_swap: u64 = 0;

            for line in meminfo.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 2 {
                    continue;
                }

                let value_kb = parts[1].parse::<u64>().unwrap_or(0);

                match parts[0] {
                    "MemTotal:" => total_mem = value_kb,
                    "MemAvailable:" => available_mem = value_kb,
                    "MemFree:" => free_mem = value_kb,
                    "Buffers:" => buffers = value_kb,
                    "Cached:" => cached = value_kb,
                    "SwapTotal:" => total_swap = value_kb,
                    "SwapFree:" => free_swap = value_kb,
                    _ => {}
                }
            }

            info.total_memory_kb = total_mem;
            // 已使用内存 = 总内存 - 可用内存（如果可用）或 总内存 - 空闲 - 缓存 - 缓冲区
            info.used_memory_kb = if available_mem > 0 {
                total_mem.saturating_sub(available_mem)
            } else {
                total_mem.saturating_sub(free_mem + buffers + cached)
            };

            info.total_swap_kb = total_swap;
            info.used_swap_kb = total_swap.saturating_sub(free_swap);
        }

        // 读取负载平均值
        if let Ok(loadavg) = fs::read_to_string("/proc/loadavg") {
            let parts: Vec<&str> = loadavg.split_whitespace().collect();
            if parts.len() >= 3 {
                info.load_avg_1m = parts[0].parse().unwrap_or(0.0);
                info.load_avg_5m = parts[1].parse().unwrap_or(0.0);
                info.load_avg_15m = parts[2].parse().unwrap_or(0.0);
            }
        }

        Ok(info)
    }

    pub fn cpu_usage_percent(&self) -> f64 {
        // Load average is already normalized per CPU
        // On Linux, loadavg of 1.0 means 100% utilization of one CPU core
        // For a multi-core system, loadavg can exceed the number of cores
        // We calculate percentage based on the ratio of load to number of cores
        if self.cpu_cores == 0 {
            return 0.0;
        }
        let usage = (self.load_avg_1m / self.cpu_cores as f64) * 100.0;
        usage.min(100.0)
    }

    pub fn memory_usage_percent(&self) -> f64 {
        if self.total_memory_kb == 0 {
            return 0.0;
        }
        (self.used_memory_kb as f64 / self.total_memory_kb as f64) * 100.0
    }

    pub fn format_memory(&self, kb: u64) -> String {
        const KB: f64 = 1.0;
        const MB: f64 = 1024.0;
        const GB: f64 = MB * 1024.0;
        const TB: f64 = GB * 1024.0;

        let bytes = kb as f64;

        if bytes >= TB {
            format!("{:.2}T", bytes / TB)
        } else if bytes >= GB {
            format!("{:.2}G", bytes / GB)
        } else if bytes >= MB {
            format!("{:.1}M", bytes / MB)
        } else {
            format!("{:.0}K", bytes / KB)
        }
    }

    pub fn header_summary(&self) -> String {
        // Show CPU usage based on loadavg
        // loadavg of 1.0 = 100% of one core
        // For multi-core systems, we show loadavg and percentage
        format!(
            "CPU: {}c/{}t L:{:.1} ({:.0}%) | MEM: {}/{} ({:.0}%)",
            self.cpu_cores,
            self.cpu_threads,
            self.load_avg_1m,
            self.cpu_usage_percent(),
            self.format_memory(self.used_memory_kb),
            self.format_memory(self.total_memory_kb),
            self.memory_usage_percent()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_memory() {
        let info = SystemInfo::default();
        assert_eq!(info.format_memory(512), "512K");
        assert_eq!(info.format_memory(1024), "1.0M");
        assert_eq!(info.format_memory(1024 * 1024), "1.00G");
        assert_eq!(info.format_memory(1024 * 1024 * 1024), "1.00T");
    }

    #[test]
    fn test_cpu_usage_percent() {
        let mut info = SystemInfo::default();
        info.cpu_cores = 4;
        info.load_avg_1m = 2.0;
        assert_eq!(info.cpu_usage_percent(), 50.0);

        info.load_avg_1m = 8.0;
        assert_eq!(info.cpu_usage_percent(), 100.0); // capped at 100%
    }

    #[test]
    fn test_memory_usage_percent() {
        let mut info = SystemInfo::default();
        info.total_memory_kb = 1000;
        info.used_memory_kb = 250;
        assert_eq!(info.memory_usage_percent(), 25.0);
    }
}
