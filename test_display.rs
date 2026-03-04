// Quick test to verify data collection
use std::fs;

fn main() {
    println!("=== Testing Data Collection ===\n");
    
    // Test Network
    println!("1. Network Interfaces:");
    if let Ok(content) = fs::read_to_string("/proc/net/dev") {
        for line in content.lines().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if !parts.is_empty() {
                let iface = parts[0].trim_end_matches(':');
                if !iface.is_empty() && iface != "lo" {
                    println!("   - {}", iface);
                }
            }
        }
    }
    
    // Test Disk I/O
    println!("\n2. Disk I/O Devices (from /proc/diskstats):");
    if let Ok(content) = fs::read_to_string("/proc/diskstats") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let device = parts[2];
                // Skip partitions, only show main devices
                if !device.starts_with("loop") && 
                   !device.starts_with("ram") &&
                   !is_partition(device) {
                    println!("   - {}", device);
                }
            }
        }
    }
    
    // Test Disk Usage
    println!("\n3. Disk Usage (from /proc/mounts):");
    if let Ok(content) = fs::read_to_string("/proc/mounts") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let fs = parts[0];
                let mount = parts[1];
                // Only show real filesystems
                if (fs.starts_with("/dev/") || fs.contains(":")) &&
                   !fs.starts_with("/dev/loop") &&
                   !mount.starts_with("/sys") &&
                   !mount.starts_with("/proc") &&
                   !mount.starts_with("/run") &&
                   mount != "/dev" {
                    println!("   - {} -> {}", fs, mount);
                }
            }
        }
    }
}

fn is_partition(device: &str) -> bool {
    // nvme partitions: nvme0n1p1
    if device.starts_with("nvme") {
        return device.contains('p') && 
               device.matches('p').count() == 1 &&
               device.split('p').nth(1).map(|s| s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)).unwrap_or(false);
    }
    // sdX, hdX partitions: sda1, sdb2
    let chars: Vec<char> = device.chars().collect();
    if chars.len() >= 2 {
        let last = chars[chars.len() - 1];
        if last.is_ascii_digit() && 
           (device.starts_with("sd") || device.starts_with("hd") || device.starts_with("vd")) {
            return true;
        }
    }
    false
}
