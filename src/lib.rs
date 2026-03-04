pub mod disk;
pub mod network;
pub mod process;
pub mod ui;
pub mod nfs;
pub mod system;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes_size() {
        assert_eq!(disk::format_bytes_size(0), "0B");
        assert_eq!(disk::format_bytes_size(512), "512B");
        assert_eq!(disk::format_bytes_size(1024), "1.0K");
        assert_eq!(disk::format_bytes_size(1024 * 1024), "1.0M");
        assert_eq!(disk::format_bytes_size(1024 * 1024 * 1024), "1.0G");
        assert_eq!(disk::format_bytes_size(1024_u64 * 1024 * 1024 * 1024), "1.0T");
    }

    #[test]
    fn test_format_bytes_per_sec() {
        assert_eq!(disk::format_bytes_per_sec(0.0), "0 B/s");
        assert_eq!(disk::format_bytes_per_sec(512.0), "512 B/s");
        assert_eq!(disk::format_bytes_per_sec(1024.0), "1.0 KB/s");
        assert_eq!(disk::format_bytes_per_sec(1024.0 * 1024.0), "1.0 MB/s");
        assert_eq!(disk::format_bytes_per_sec(1024.0 * 1024.0 * 1024.0), "1.0 GB/s");
    }

    #[test]
    fn test_format_iops() {
        assert_eq!(disk::format_iops(0.0), "0");
        assert_eq!(disk::format_iops(999.0), "999");
        assert_eq!(disk::format_iops(1000.0), "1.0k");
        assert_eq!(disk::format_iops(1500.0), "1.5k");
    }

    #[test]
    fn test_network_format_bytes() {
        assert_eq!(network::format_bytes(0.0), "0 B");
        assert_eq!(network::format_bytes(1024.0), "1.00 KB");
        assert_eq!(network::format_bytes(1024.0 * 1024.0), "1.00 MB");
        assert_eq!(network::format_bytes(1024.0 * 1024.0 * 1024.0), "1.00 GB");
    }

    #[test]
    fn test_network_format_bytes_per_sec() {
        assert_eq!(network::format_bytes_per_sec(0.0), "0 B/s");
        assert_eq!(network::format_bytes_per_sec(1024.0), "1.00 KB/s");
        assert_eq!(network::format_bytes_per_sec(1024.0 * 1024.0), "1.00 MB/s");
    }

    #[test]
    fn test_disk_is_partition() {
        // Main devices (not partitions)
        assert!(!disk::DiskCollector::is_partition("sda"));
        assert!(!disk::DiskCollector::is_partition("sdb"));
        assert!(!disk::DiskCollector::is_partition("nvme0n1"));
        assert!(!disk::DiskCollector::is_partition("nvme1n1"));
        assert!(!disk::DiskCollector::is_partition("mmcblk0"));
        
        // Partitions
        assert!(disk::DiskCollector::is_partition("sda1"));
        assert!(disk::DiskCollector::is_partition("sda2"));
        assert!(disk::DiskCollector::is_partition("sdb10"));
        assert!(disk::DiskCollector::is_partition("nvme0n1p1"));
        assert!(disk::DiskCollector::is_partition("nvme0n1p2"));
        assert!(disk::DiskCollector::is_partition("mmcblk0p1"));
    }

    #[test]
    fn test_disk_usage_calculation() {
        let usage = disk::DiskUsage {
            filesystem: "/dev/sda1".to_string(),
            size: 1024_u64 * 1024 * 1024 * 1024, // 1TB (using 1024 for exact conversion)
            used: 512_u64 * 1024 * 1024 * 1024,  // 512GB
            avail: 512_u64 * 1024 * 1024 * 1024, // 512GB
            use_percent: 50.0,
            mounted_on: "/home".to_string(),
            device: "sda1".to_string(),
        };
        
        assert_eq!(usage.use_percent, 50.0);
        assert_eq!(disk::format_bytes_size(usage.size), "1.0T");
        assert_eq!(disk::format_bytes_size(usage.used), "512.0G");
    }

    #[test]
    fn test_process_delta_calculation() {
        let delta = process::ProcessDelta {
            pid: 1234,
            name: "test_process".to_string(),
            user: "testuser".to_string(),
            connections: 5,
            read_bytes_sec: 1024.0 * 1024.0, // 1 MB/s
            write_bytes_sec: 512.0 * 1024.0,  // 512 KB/s
            cpu_percent: 25.5,
            mem_percent: 10.0,
            state: "Running".to_string(),
        };
        
        assert_eq!(delta.pid, 1234);
        assert_eq!(delta.connections, 5);
        assert_eq!(disk::format_bytes_per_sec(delta.read_bytes_sec), "1.0 MB/s");
        assert_eq!(disk::format_bytes_per_sec(delta.write_bytes_sec), "512.0 KB/s");
    }

    #[test]
    fn test_sort_by_enum() {
        use ui::SortBy;
        
        assert_eq!(SortBy::Cpu.name(), "CPU%");
        assert_eq!(SortBy::Mem.name(), "MEM%");
        assert_eq!(SortBy::ReadIO.name(), "READ");
        assert_eq!(SortBy::WriteIO.name(), "WRITE");
        assert_eq!(SortBy::Connections.name(), "CONN");
        assert_eq!(SortBy::Pid.name(), "PID");
    }

    #[test]
    fn test_app_cycle_sort() {
        use ui::{App, SortBy};
        
        let mut app = App::new();
        assert_eq!(app.sort_by, SortBy::Cpu);
        
        app.cycle_sort();
        assert_eq!(app.sort_by, SortBy::Mem);
        
        app.cycle_sort();
        assert_eq!(app.sort_by, SortBy::ReadIO);
        
        app.cycle_sort();
        assert_eq!(app.sort_by, SortBy::WriteIO);
        
        app.cycle_sort();
        assert_eq!(app.sort_by, SortBy::Connections);
        
        app.cycle_sort();
        assert_eq!(app.sort_by, SortBy::Pid);
        
        app.cycle_sort();
        assert_eq!(app.sort_by, SortBy::Cpu);
    }

    #[test]
    fn test_app_navigation() {
        use ui::App;
        
        let mut app = App::new();
        
        // Test next
        app.next(10);
        assert_eq!(app.selected_index, 1);
        
        app.next(10);
        assert_eq!(app.selected_index, 2);
        
        // Test previous
        app.previous(10);
        assert_eq!(app.selected_index, 1);
        
        // Test wrap around
        app.selected_index = 9;
        app.next(10);
        assert_eq!(app.selected_index, 0);
        
        app.previous(10);
        assert_eq!(app.selected_index, 9);
    }
}
