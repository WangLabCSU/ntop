# ntop

A real-time system resource monitor for Linux, inspired by htop but focused on network and disk I/O monitoring.

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)

## Features

- **Real-time Network Monitoring**: Track RX/TX rates per network interface
- **Disk I/O Statistics**: Monitor read/write rates, IOPS, and utilization per device
- **Disk Usage Display**: View filesystem usage (like `df -h`) including NAS mounts
- **Process Monitoring**: CPU%, MEM%, disk I/O, and network connections per process
- **Interactive Filtering**: Filter processes by username or PID
- **Sorting**: Sort processes by CPU, MEM, DiskRd, DiskWr, Connections, or PID
- **Responsive UI**: 16ms event polling for smooth interaction

## Installation

### From Source

```bash
git clone https://github.com/WangLabCSU/ntop.git
cd ntop
cargo build --release
sudo cp target/release/ntop /usr/local/bin/
```

### Prerequisites

- Rust 1.70 or higher
- Linux kernel 2.6.33+ (for `/proc/diskstats` support)

## Usage

```bash
ntop
```

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `q` | Quit |
| `h`/`?` | Show help |
| `u` | Filter by username |
| `p` | Filter by PID |
| `c` | Clear filter |
| `s` | Cycle sort order |
| `↑`/`k` | Move up |
| `↓`/`j` | Move down |
| `Esc` | Clear filter / Close help |

### Sort Order

Press `s` to cycle through:
1. CPU% (default)
2. MEM%
3. DiskRd (read bytes/sec)
4. DiskWr (write bytes/sec)
5. Connections
6. PID

## Data Sources

ntop reads from the following Linux `/proc` files:

- `/proc/net/dev` - Network interface statistics
- `/proc/diskstats` - Disk I/O statistics
- `/proc/mounts` - Filesystem mount information
- `/proc/[pid]/stat` - Process CPU and memory info
- `/proc/[pid]/io` - Process I/O statistics
- `/proc/[pid]/fd/` - Process file descriptors (for connection counting)

## Architecture

```
ntop/
├── src/
│   ├── main.rs      # Application entry point and event loop
│   ├── network.rs   # Network statistics collection
│   ├── disk.rs      # Disk I/O and usage collection
│   ├── process.rs   # Process statistics collection
│   └── ui.rs        # Terminal UI rendering
├── Cargo.toml
└── README.md
```

## Platform Support

- **Linux**: Full support (tested on Ubuntu 22.04, CentOS 8)
- **macOS**: Not supported (requires `/proc` filesystem)
- **Windows**: Not supported

## Development

### Running Tests

```bash
cargo test
```

### Running with Debug Output

```bash
RUST_LOG=debug cargo run
```

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Inspired by [htop](https://htop.dev/) and [btop](https://github.com/aristocratos/btop)
- Built with [ratatui](https://github.com/ratatui-org/ratatui) for terminal UI
