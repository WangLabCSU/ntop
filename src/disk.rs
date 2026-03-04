use anyhow::Result;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Default)]
pub struct DiskUsage {
    pub filesystem: String,
    pub size: u64,
    pub used: u64,
    pub avail: u64,
    pub use_percent: f64,
    pub mounted_on: String,
    pub device: String,
}

#[derive(Debug, Clone, Default)]
pub struct DiskIoStats {
    pub device: String,
    pub reads_completed: u64,
    pub sectors_read: u64,
    pub writes_completed: u64,
    pub sectors_written: u64,
    pub io_time_ms: u64,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct DiskIoDelta {
    pub device: String,
    pub read_bytes_sec: f64,
    pub write_bytes_sec: f64,
    pub read_iops: f64,
    pub write_iops: f64,
    pub io_util: f64,
}

pub struct DiskCollector {
    last_io_stats: HashMap<String, DiskIoStats>,
    last_time: std::time::Instant,
}

impl Default for DiskCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskCollector {
    pub fn new() -> Self {
        Self {
            last_io_stats: HashMap::new(),
            last_time: std::time::Instant::now(),
        }
    }

    pub fn read_disk_usage() -> Result<Vec<DiskUsage>> {
        let content = fs::read_to_string("/proc/mounts")?;
        let mut usage_list = Vec::new();
        let mut seen_mounts: Vec<String> = Vec::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            let filesystem = parts[0].to_string();
            let mount_point = parts[1].to_string();

            // Skip virtual filesystems
            if filesystem.starts_with("tmpfs")
                || filesystem.starts_with("devtmpfs")
                || filesystem.starts_with("cgroup")
                || filesystem.starts_with("cgmfs")
                || filesystem.starts_with("mqueue")
                || filesystem.starts_with("hugetlbfs")
                || filesystem.starts_with("debugfs")
                || filesystem.starts_with("tracefs")
                || filesystem.starts_with("securityfs")
                || filesystem.starts_with("pstore")
                || filesystem.starts_with("configfs")
                || filesystem.starts_with("fusectl")
                || filesystem.starts_with("sysfs")
                || filesystem.starts_with("proc")
                || filesystem.starts_with("devpts")
                || filesystem.starts_with("autofs")
                || filesystem.starts_with("binfmt_misc")
                || filesystem.starts_with("/dev/loop")
                || mount_point.starts_with("/sys")
                || mount_point.starts_with("/proc")
                || mount_point.starts_with("/dev/.")
                || mount_point == "/run"
                // Skip snap bind mounts
                || mount_point.contains("/snap/")
                || mount_point.contains("/var/snap/")
            {
                continue;
            }

            if seen_mounts.contains(&mount_point) {
                continue;
            }

            if let Ok(statvfs) = Self::get_statvfs(&mount_point) {
                let device_name = Self::get_device_name(&filesystem);
                let usage = DiskUsage {
                    filesystem: filesystem.clone(),
                    size: statvfs.total_bytes,
                    used: statvfs.used_bytes,
                    avail: statvfs.available_bytes,
                    use_percent: if statvfs.total_bytes > 0 {
                        (statvfs.used_bytes as f64 / statvfs.total_bytes as f64) * 100.0
                    } else {
                        0.0
                    },
                    mounted_on: mount_point.clone(),
                    device: device_name,
                };
                usage_list.push(usage);
                seen_mounts.push(mount_point);
            }
        }

        Ok(usage_list)
    }

    fn get_device_name(filesystem: &str) -> String {
        if filesystem.starts_with("/dev/") {
            filesystem.trim_start_matches("/dev/").to_string()
        } else if filesystem.contains(":") {
            let parts: Vec<&str> = filesystem.split(':').collect();
            if parts.len() > 1 {
                parts[1].trim_start_matches('/').to_string()
            } else {
                filesystem.to_string()
            }
        } else {
            filesystem.to_string()
        }
    }

    fn get_statvfs(path: &str) -> Result<StatvfsResult> {
        use std::ffi::CString;
        use std::mem::MaybeUninit;

        let c_path = CString::new(path)?;
        let mut stat: MaybeUninit<libc::statvfs> = MaybeUninit::uninit();

        let result = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
        if result != 0 {
            return Err(anyhow::anyhow!("statvfs failed for {}", path));
        }

        let stat = unsafe { stat.assume_init() };
        let block_size = stat.f_frsize;
        let total_blocks = stat.f_blocks;
        let free_blocks = stat.f_bfree;
        let avail_blocks = stat.f_bavail;

        let total_bytes = block_size * total_blocks;
        let free_bytes = block_size * free_blocks;
        let avail_bytes = block_size * avail_blocks;
        let used_bytes = total_bytes.saturating_sub(free_bytes);

        Ok(StatvfsResult {
            total_bytes,
            used_bytes,
            available_bytes: avail_bytes,
        })
    }

    pub fn read_disk_io_stats() -> Result<Vec<DiskIoStats>> {
        let content = fs::read_to_string("/proc/diskstats")?;
        let mut stats = Vec::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 14 {
                continue;
            }

            let device = parts[2].to_string();

            if device.starts_with("loop") || device.starts_with("ram") {
                continue;
            }

            if Self::is_partition(&device) {
                continue;
            }

            stats.push(DiskIoStats {
                device,
                reads_completed: parts[3].parse().unwrap_or(0),
                sectors_read: parts[5].parse().unwrap_or(0),
                writes_completed: parts[7].parse().unwrap_or(0),
                sectors_written: parts[9].parse().unwrap_or(0),
                io_time_ms: parts[12].parse().unwrap_or(0),
            });
        }

        Ok(stats)
    }

    pub fn is_partition(device: &str) -> bool {
        // nvme partitions: nvme0n1p1, nvme0n1p2 (not nvme0n1 itself)
        if device.starts_with("nvme") {
            // nvme0n1p1 -> partition, nvme0n1 -> main device
            return device.contains('p')
                && device.matches('p').count() == 1
                && device
                    .split('p')
                    .nth(1)
                    .map(|s| {
                        s.chars()
                            .next()
                            .map(|c| c.is_ascii_digit())
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
        }
        // mmcblk partitions: mmcblk0p1
        if device.starts_with("mmcblk") {
            return device.contains('p')
                && device.matches('p').count() == 1
                && device
                    .split('p')
                    .nth(1)
                    .map(|s| {
                        s.chars()
                            .next()
                            .map(|c| c.is_ascii_digit())
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
        }
        // sdX, hdX, vdX partitions: sda1, sdb2
        let chars: Vec<char> = device.chars().collect();
        if chars.len() < 2 {
            return false;
        }
        let last_char = chars[chars.len() - 1];
        if last_char.is_ascii_digit() {
            // Check if it's a multi-digit suffix (like sda10)
            if chars.len() >= 3 {
                let second_last = chars[chars.len() - 2];
                if second_last.is_ascii_digit() {
                    return true;
                }
            }
            // Single digit suffix (sda1, sdb2)
            if device.starts_with("sd") || device.starts_with("hd") || device.starts_with("vd") {
                return true;
            }
        }
        false
    }

    pub fn collect(&mut self) -> Result<(Vec<DiskUsage>, Vec<DiskIoDelta>)> {
        let usage = Self::read_disk_usage()?;
        let current_io = Self::read_disk_io_stats()?;
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_time).as_secs_f64();
        let is_first_run = self.last_io_stats.is_empty();

        let mut deltas = Vec::new();

        // Always return all devices, with 0 rates on first run
        for current in &current_io {
            let (read_sec, write_sec, rd_iops, wr_iops, util) = if is_first_run || elapsed <= 0.0 {
                (0.0, 0.0, 0.0, 0.0, 0.0)
            } else if let Some(last) = self.last_io_stats.get(&current.device) {
                let sector_size = 512.0_f64;
                let read_bytes =
                    (current.sectors_read.saturating_sub(last.sectors_read)) as f64 * sector_size;
                let write_bytes = (current.sectors_written.saturating_sub(last.sectors_written))
                    as f64
                    * sector_size;
                let reads = current.reads_completed.saturating_sub(last.reads_completed) as f64;
                let writes = current
                    .writes_completed
                    .saturating_sub(last.writes_completed) as f64;
                let io_time = current.io_time_ms.saturating_sub(last.io_time_ms) as f64;

                (
                    read_bytes / elapsed,
                    write_bytes / elapsed,
                    reads / elapsed,
                    writes / elapsed,
                    (io_time / 1000.0 / elapsed * 100.0).min(100.0),
                )
            } else {
                (0.0, 0.0, 0.0, 0.0, 0.0)
            };

            deltas.push(DiskIoDelta {
                device: current.device.clone(),
                read_bytes_sec: read_sec,
                write_bytes_sec: write_sec,
                read_iops: rd_iops,
                write_iops: wr_iops,
                io_util: util,
            });
        }

        self.last_io_stats.clear();
        for stat in current_io {
            self.last_io_stats.insert(stat.device.clone(), stat);
        }
        self.last_time = now;

        Ok((usage, deltas))
    }
}

struct StatvfsResult {
    total_bytes: u64,
    used_bytes: u64,
    available_bytes: u64,
}

pub fn format_bytes_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1}T", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

pub fn format_bytes_per_sec(bytes_sec: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes_sec >= GB {
        format!("{:.1} GB/s", bytes_sec / GB)
    } else if bytes_sec >= MB {
        format!("{:.1} MB/s", bytes_sec / MB)
    } else if bytes_sec >= KB {
        format!("{:.1} KB/s", bytes_sec / KB)
    } else {
        format!("{:.0} B/s", bytes_sec)
    }
}

#[allow(dead_code)]
pub fn format_iops(iops: f64) -> String {
    if iops >= 1000.0 {
        format!("{:.1}k", iops / 1000.0)
    } else {
        format!("{:.0}", iops)
    }
}
