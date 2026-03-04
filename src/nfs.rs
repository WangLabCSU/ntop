use anyhow::Result;
use std::fs;

#[derive(Debug, Clone, Default)]
pub struct NfsMount {
    pub server: String,
    pub path: String,
    pub mount_point: String,
    pub fs_type: String,
}

#[derive(Debug, Clone, Default)]
pub struct NfsStats {
    pub mount: NfsMount,
    pub read_ops: u64,
    pub write_ops: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub struct NfsStatsDelta {
    pub mount_point: String,
    pub read_ops_sec: f64,
    pub write_ops_sec: f64,
    pub read_bytes_sec: f64,
    pub write_bytes_sec: f64,
}

pub struct NfsCollector {
    last_stats: Vec<NfsStats>,
    last_time: std::time::Instant,
}

impl NfsCollector {
    pub fn new() -> Self {
        Self {
            last_stats: Vec::new(),
            last_time: std::time::Instant::now(),
        }
    }

    /// 读取 NFS 挂载点信息
    pub fn read_nfs_mounts() -> Result<Vec<NfsMount>> {
        let content = fs::read_to_string("/proc/mounts")?;
        let mut mounts = Vec::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }

            let fs_type = parts[2];
            if fs_type.starts_with("nfs") {
                let device = parts[0];
                let mount_point = parts[1];

                // 解析 server:path 格式
                let device_parts: Vec<&str> = device.splitn(2, ':').collect();
                if device_parts.len() == 2 {
                    mounts.push(NfsMount {
                        server: device_parts[0].to_string(),
                        path: device_parts[1].to_string(),
                        mount_point: mount_point.to_string(),
                        fs_type: fs_type.to_string(),
                    });
                }
            }
        }

        Ok(mounts)
    }

    /// 读取 NFS 统计信息（从 /proc/net/rpc/nfs）
    pub fn read_nfs_stats(mounts: Vec<NfsMount>) -> Result<Vec<NfsStats>> {
        let mut stats = Vec::new();

        // 读取 /proc/net/rpc/nfs 获取总体统计
        let nfs_content = fs::read_to_string("/proc/net/rpc/nfs").unwrap_or_default();
        let mut total_read_ops: u64 = 0;
        let mut total_write_ops: u64 = 0;

        for line in nfs_content.lines() {
            if line.starts_with("proc4 ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() > 25 {
                    // proc4 格式: proc4 <ops> <null> <getattr> <setattr> <lookup> <access> <readlink> <read> <write> ...
                    // read 操作通常在第 8 个位置 (索引 8)
                    // write 操作通常在第 9 个位置 (索引 9)
                    total_read_ops = parts.get(8).and_then(|s| s.parse().ok()).unwrap_or(0);
                    total_write_ops = parts.get(9).and_then(|s| s.parse().ok()).unwrap_or(0);
                }
            }
        }

        // 为每个挂载点创建统计（目前 Linux 不提供每个挂载点的 NFS 统计，只有总体统计）
        // 我们将总体统计分配给每个挂载点，或者如果有多个挂载点，则平均分配
        let mount_count = mounts.len() as u64;
        let per_mount_read = if mount_count > 0 {
            total_read_ops / mount_count
        } else {
            0
        };
        let per_mount_write = if mount_count > 0 {
            total_write_ops / mount_count
        } else {
            0
        };

        // 估算字节数（NFS 通常使用较大的块大小）
        let avg_read_size: u64 = 64 * 1024; // 64KB 平均读取大小
        let avg_write_size: u64 = 64 * 1024; // 64KB 平均写入大小

        for mount in mounts {
            stats.push(NfsStats {
                mount: mount.clone(),
                read_ops: per_mount_read,
                write_ops: per_mount_write,
                read_bytes: per_mount_read * avg_read_size,
                write_bytes: per_mount_write * avg_write_size,
            });
        }

        Ok(stats)
    }

    pub fn collect(&mut self) -> Result<(Vec<NfsStats>, Vec<NfsStatsDelta>)> {
        let mounts = Self::read_nfs_mounts()?;
        let current_stats = Self::read_nfs_stats(mounts)?;
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_time).as_secs_f64();
        let is_first_run = self.last_stats.is_empty();

        let mut deltas = Vec::new();

        for current in &current_stats {
            let (read_ops_sec, write_ops_sec, read_bytes_sec, write_bytes_sec) =
                if is_first_run || elapsed <= 0.0 {
                    (0.0, 0.0, 0.0, 0.0)
                } else if let Some(last) = self
                    .last_stats
                    .iter()
                    .find(|s| s.mount.mount_point == current.mount.mount_point)
                {
                    (
                        (current.read_ops.saturating_sub(last.read_ops)) as f64 / elapsed,
                        (current.write_ops.saturating_sub(last.write_ops)) as f64 / elapsed,
                        (current.read_bytes.saturating_sub(last.read_bytes)) as f64 / elapsed,
                        (current.write_bytes.saturating_sub(last.write_bytes)) as f64 / elapsed,
                    )
                } else {
                    (0.0, 0.0, 0.0, 0.0)
                };

            deltas.push(NfsStatsDelta {
                mount_point: current.mount.mount_point.clone(),
                read_ops_sec,
                write_ops_sec,
                read_bytes_sec,
                write_bytes_sec,
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

pub fn format_ops_per_sec(ops_sec: f64) -> String {
    format!("{:.1} op/s", ops_sec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes_per_sec() {
        assert_eq!(format_bytes_per_sec(1024.0), "1.00 KB/s");
        assert_eq!(format_bytes_per_sec(1024.0 * 1024.0), "1.00 MB/s");
    }

    #[test]
    fn test_format_ops_per_sec() {
        assert_eq!(format_ops_per_sec(10.5), "10.5 op/s");
        assert_eq!(format_ops_per_sec(0.0), "0.0 op/s");
    }
}
