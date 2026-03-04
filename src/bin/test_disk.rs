use ntop::disk::DiskCollector;

fn main() {
    println!("Testing disk data collection...\n");
    
    let mut collector = DiskCollector::new();
    
    match collector.collect() {
        Ok((usage, deltas)) => {
            println!("=== Disk Usage (from /proc/mounts) ===");
            println!("Found {} filesystems:", usage.len());
            for u in &usage {
                println!("  Device: {:20} | FS: {:30} | Mount: {}", 
                    u.device, u.filesystem, u.mounted_on);
                println!("    Size: {:10} | Used: {:10} | Avail: {:10} | Use%: {:.1}%",
                    ntop::disk::format_bytes_size(u.size),
                    ntop::disk::format_bytes_size(u.used),
                    ntop::disk::format_bytes_size(u.avail),
                    u.use_percent);
            }
            
            println!("\n=== Disk I/O Stats (from /proc/diskstats) ===");
            println!("Found {} devices:", deltas.len());
            for d in &deltas {
                println!("  Device: {:15} | Read: {:12}/s | Write: {:12}/s | Util: {:.1}%",
                    d.device,
                    ntop::disk::format_bytes_per_sec(d.read_bytes_sec),
                    ntop::disk::format_bytes_per_sec(d.write_bytes_sec),
                    d.io_util);
            }
        }
        Err(e) => {
            eprintln!("Error collecting disk data: {}", e);
        }
    }
    
    // Test is_partition function
    println!("\n=== Partition Detection Test ===");
    let test_devices = vec![
        "sda", "sda1", "sda2", "sdb", "sdb10",
        "nvme0n1", "nvme0n1p1", "nvme0n1p2",
        "mmcblk0", "mmcblk0p1",
    ];
    for dev in test_devices {
        let is_part = DiskCollector::is_partition(dev);
        println!("  {:15} -> {}", dev, if is_part { "PARTITION" } else { "MAIN DEVICE" });
    }
}
