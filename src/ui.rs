use crate::network::{NetworkStats, NetworkStatsDelta, format_bytes_per_sec as net_format_bytes_per_sec, format_bytes};
use crate::disk::{DiskIoDelta, DiskUsage, format_bytes_per_sec, format_bytes_size};
use crate::process::ProcessDelta;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table, Paragraph, Clear, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    FilterUser,
    FilterPid,
    Help,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortBy {
    Cpu,
    Mem,
    ReadIO,
    WriteIO,
    Connections,
    Pid,
}

impl SortBy {
    pub fn name(&self) -> &'static str {
        match self {
            SortBy::Cpu => "CPU%",
            SortBy::Mem => "MEM%",
            SortBy::ReadIO => "READ",
            SortBy::WriteIO => "WRITE",
            SortBy::Connections => "CONN",
            SortBy::Pid => "PID",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FocusPanel {
    Network,
    DiskIo,
    DiskUsage,
    Processes,
}

pub struct App {
    pub mode: AppMode,
    pub focus: FocusPanel,
    pub selected_index: usize,
    pub filter_user: Option<String>,
    pub filter_pid: Option<u32>,
    pub input_buffer: String,
    pub scroll_offset: usize,
    pub sort_by: SortBy,
    pub total_rx_rate: f64,
    pub total_tx_rate: f64,
    pub total_disk_read: f64,
    pub total_disk_write: f64,
    pub disk_usage_scroll: usize,
    pub disk_io_scroll: usize,
    pub network_scroll: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            mode: AppMode::Normal,
            focus: FocusPanel::Processes,
            selected_index: 0,
            filter_user: None,
            filter_pid: None,
            input_buffer: String::new(),
            scroll_offset: 0,
            sort_by: SortBy::Cpu,
            total_rx_rate: 0.0,
            total_tx_rate: 0.0,
            total_disk_read: 0.0,
            total_disk_write: 0.0,
            disk_usage_scroll: 0,
            disk_io_scroll: 0,
            network_scroll: 0,
        }
    }

    pub fn next(&mut self, total: usize) {
        if total == 0 { return; }
        self.selected_index = (self.selected_index + 1) % total;
    }

    pub fn previous(&mut self, total: usize) {
        if total == 0 { return; }
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else {
            self.selected_index = total - 1;
        }
    }

    pub fn scroll_down(&mut self, visible: usize, total: usize) {
        match self.focus {
            FocusPanel::Processes => {
                if self.selected_index < total.saturating_sub(1) {
                    self.selected_index += 1;
                }
                if self.selected_index >= self.scroll_offset + visible {
                    self.scroll_offset = self.selected_index.saturating_sub(visible - 1);
                }
            }
            FocusPanel::DiskIo => {
                self.disk_io_scroll = (self.disk_io_scroll + 1).min(total.saturating_sub(visible));
            }
            FocusPanel::DiskUsage => {
                self.disk_usage_scroll = (self.disk_usage_scroll + 1).min(total.saturating_sub(visible));
            }
            FocusPanel::Network => {
                self.network_scroll = (self.network_scroll + 1).min(total.saturating_sub(visible));
            }
        }
    }

    pub fn scroll_up(&mut self, visible: usize, _total: usize) {
        match self.focus {
            FocusPanel::Processes => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
                if self.selected_index < self.scroll_offset {
                    self.scroll_offset = self.selected_index;
                }
            }
            FocusPanel::DiskIo => {
                self.disk_io_scroll = self.disk_io_scroll.saturating_sub(1);
            }
            FocusPanel::DiskUsage => {
                self.disk_usage_scroll = self.disk_usage_scroll.saturating_sub(1);
            }
            FocusPanel::Network => {
                self.network_scroll = self.network_scroll.saturating_sub(1);
            }
        }
    }

    pub fn cycle_sort(&mut self) {
        self.sort_by = match self.sort_by {
            SortBy::Cpu => SortBy::Mem,
            SortBy::Mem => SortBy::ReadIO,
            SortBy::ReadIO => SortBy::WriteIO,
            SortBy::WriteIO => SortBy::Connections,
            SortBy::Connections => SortBy::Pid,
            SortBy::Pid => SortBy::Cpu,
        };
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            FocusPanel::Network => FocusPanel::DiskIo,
            FocusPanel::DiskIo => FocusPanel::DiskUsage,
            FocusPanel::DiskUsage => FocusPanel::Processes,
            FocusPanel::Processes => FocusPanel::Network,
        };
    }

    pub fn reset_selection(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.disk_usage_scroll = 0;
        self.disk_io_scroll = 0;
        self.network_scroll = 0;
    }
}

pub fn draw(f: &mut Frame, app: &mut App, net_stats: &[NetworkStats], net_deltas: &[NetworkStatsDelta],
            disk_usage: &[DiskUsage], disk_deltas: &[DiskIoDelta], process_deltas: &mut [ProcessDelta]) {
    app.total_rx_rate = net_deltas.iter().map(|d| d.rx_bytes_sec).sum();
    app.total_tx_rate = net_deltas.iter().map(|d| d.tx_bytes_sec).sum();
    app.total_disk_read = disk_deltas.iter().map(|d| d.read_bytes_sec).sum();
    app.total_disk_write = disk_deltas.iter().map(|d| d.write_bytes_sec).sum();

    sort_processes(process_deltas, &app.sort_by);

    let area = f.area();
    
    // Main vertical layout
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),   // Header with totals
            Constraint::Min(0),      // Main content (flexible)
            Constraint::Length(1),   // Status bar
        ])
        .split(area);

    // Draw header
    draw_compact_header(f, app, main_chunks[0]);
    
    // Main content area - split horizontally
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),  // Left: System info (Network + Disk)
            Constraint::Percentage(60),  // Right: Processes
        ])
        .split(main_chunks[1]);

    // Left side - split vertically with better proportions
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),  // Network
            Constraint::Percentage(25),  // Disk I/O
            Constraint::Percentage(50),  // Disk Usage
        ])
        .split(content_chunks[0]);

    draw_network_scrollable(f, app, net_stats, net_deltas, left_chunks[0]);
    draw_disk_io_scrollable(f, app, disk_deltas, left_chunks[1]);
    draw_disk_usage_scrollable(f, app, disk_usage, left_chunks[2]);
    
    // Right side - processes
    draw_process_panel(f, app, process_deltas, content_chunks[1]);
    
    // Status bar
    draw_status_bar(f, app, main_chunks[2]);

    // Help overlay
    if app.mode == AppMode::Help {
        draw_help(f);
    }
}

fn sort_processes(processes: &mut [ProcessDelta], sort_by: &SortBy) {
    match sort_by {
        SortBy::Cpu => processes.sort_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap_or(std::cmp::Ordering::Equal)),
        SortBy::Mem => processes.sort_by(|a, b| b.mem_percent.partial_cmp(&a.mem_percent).unwrap_or(std::cmp::Ordering::Equal)),
        SortBy::ReadIO => processes.sort_by(|a, b| b.read_bytes_sec.partial_cmp(&a.read_bytes_sec).unwrap_or(std::cmp::Ordering::Equal)),
        SortBy::WriteIO => processes.sort_by(|a, b| b.write_bytes_sec.partial_cmp(&a.write_bytes_sec).unwrap_or(std::cmp::Ordering::Equal)),
        SortBy::Connections => processes.sort_by(|a, b| b.connections.cmp(&a.connections)),
        SortBy::Pid => processes.sort_by(|a, b| a.pid.cmp(&b.pid)),
    }
}

fn draw_compact_header(f: &mut Frame, app: &App, area: Rect) {
    let header_text = format!(
        " ntop │ ▼ {} ▲ {} │ ▼ {} ▲ {} ",
        net_format_bytes_per_sec(app.total_rx_rate),
        net_format_bytes_per_sec(app.total_tx_rate),
        format_bytes_per_sec(app.total_disk_read),
        format_bytes_per_sec(app.total_disk_write)
    );
    
    let header = Paragraph::new(header_text)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(header, area);
}

fn draw_network_scrollable(f: &mut Frame, app: &mut App, stats: &[NetworkStats], deltas: &[NetworkStatsDelta], area: Rect) {
    let is_focused = matches!(app.focus, FocusPanel::Network);
    let border_style = if is_focused {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green)
    };

    let block = Block::default()
        .title(format!(" Network [{} interfaces] ", stats.len()))
        .borders(Borders::ALL)
        .border_style(border_style)
        .title_style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if stats.is_empty() {
        f.render_widget(Paragraph::new("No network interfaces").style(Style::default().fg(Color::Yellow)), inner);
        return;
    }

    let header_height = 1;
    let visible_rows = inner.height.saturating_sub(header_height) as usize;

    // Ensure scroll offset is valid
    if app.network_scroll > stats.len().saturating_sub(visible_rows) {
        app.network_scroll = stats.len().saturating_sub(visible_rows);
    }

    let rows: Vec<Row> = stats
        .iter()
        .zip(deltas.iter())
        .skip(app.network_scroll)
        .take(visible_rows)
        .map(|(s, d)| {
            Row::new(vec![
                Cell::from(Span::styled(&s.interface, Style::default().fg(Color::Cyan))),
                Cell::from(Span::styled(net_format_bytes_per_sec(d.rx_bytes_sec), Style::default().fg(Color::Green))),
                Cell::from(Span::styled(net_format_bytes_per_sec(d.tx_bytes_sec), Style::default().fg(Color::Red))),
            ])
        })
        .collect();

    let header = Row::new(vec![
        Cell::from(Span::styled("Interface", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("▼RX/s", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("▲TX/s", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))),
    ]);

    let table = Table::new(rows, [
        Constraint::Length(15),
        Constraint::Length(12),
        Constraint::Length(12),
    ])
    .header(header)
    .column_spacing(1);

    f.render_widget(table, inner);

    // Draw scrollbar if needed
    if stats.len() > visible_rows {
        let mut scrollbar_state = ScrollbarState::new(stats.len())
            .position(app.network_scroll)
            .viewport_content_length(visible_rows);
        
        let scrollbar_area = Rect {
            x: inner.x + inner.width.saturating_sub(1),
            y: inner.y + 1,
            width: 1,
            height: inner.height.saturating_sub(2),
        };
        
        f.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .thumb_style(if is_focused { Style::default().fg(Color::Green) } else { Style::default().fg(Color::DarkGray) })
                .track_style(Style::default().fg(Color::DarkGray)),
            scrollbar_area,
            &mut scrollbar_state,
        );
    }
}

fn draw_disk_io_scrollable(f: &mut Frame, app: &mut App, deltas: &[DiskIoDelta], area: Rect) {
    let is_focused = matches!(app.focus, FocusPanel::DiskIo);
    let border_style = if is_focused {
        Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Blue)
    };

    let block = Block::default()
        .title(format!(" Disk I/O [{} devices] ", deltas.len()))
        .borders(Borders::ALL)
        .border_style(border_style)
        .title_style(Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if deltas.is_empty() {
        f.render_widget(Paragraph::new("No disk devices").style(Style::default().fg(Color::Yellow)), inner);
        return;
    }

    let header_height = 1;
    let visible_rows = inner.height.saturating_sub(header_height) as usize;
    
    // Ensure scroll offset is valid
    if app.disk_io_scroll > deltas.len().saturating_sub(visible_rows) {
        app.disk_io_scroll = deltas.len().saturating_sub(visible_rows);
    }

    let rows: Vec<Row> = deltas
        .iter()
        .skip(app.disk_io_scroll)
        .take(visible_rows)
        .map(|d| {
            let util_style = if d.io_util > 80.0 {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else if d.io_util > 50.0 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Green)
            };

            Row::new(vec![
                Cell::from(Span::styled(&d.device, Style::default().fg(Color::Cyan))),
                Cell::from(Span::styled(format_bytes_per_sec(d.read_bytes_sec), Style::default().fg(Color::Blue))),
                Cell::from(Span::styled(format_bytes_per_sec(d.write_bytes_sec), Style::default().fg(Color::Magenta))),
                Cell::from(Span::styled(format!("{:.0}%", d.io_util), util_style)),
            ])
        })
        .collect();

    let header = Row::new(vec![
        Cell::from(Span::styled("Device", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("▼Read", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("▲Write", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Util", Style::default().add_modifier(Modifier::BOLD))),
    ]);

    let table = Table::new(rows, [
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(6),
    ])
    .header(header)
    .column_spacing(1);

    f.render_widget(table, inner);

    // Draw scrollbar if needed
    if deltas.len() > visible_rows {
        let mut scrollbar_state = ScrollbarState::new(deltas.len())
            .position(app.disk_io_scroll)
            .viewport_content_length(visible_rows);
        
        let scrollbar_area = Rect {
            x: inner.x + inner.width.saturating_sub(1),
            y: inner.y + 1,
            width: 1,
            height: inner.height.saturating_sub(2),
        };
        
        f.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .thumb_style(if is_focused { Style::default().fg(Color::Blue) } else { Style::default().fg(Color::DarkGray) })
                .track_style(Style::default().fg(Color::DarkGray)),
            scrollbar_area,
            &mut scrollbar_state,
        );
    }
}

fn draw_disk_usage_scrollable(f: &mut Frame, app: &mut App, usage: &[DiskUsage], area: Rect) {
    let is_focused = matches!(app.focus, FocusPanel::DiskUsage);
    let border_style = if is_focused {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Yellow)
    };

    // Filter to show only real filesystems with hierarchy
    let real_usage: Vec<&DiskUsage> = usage
        .iter()
        .filter(|d| {
            // Only show block devices and NFS mounts
            let is_block_device = d.filesystem.starts_with("/dev/") || 
                                  d.filesystem.contains(":");  // NFS
            let is_not_virtual = !d.filesystem.starts_with("tmpfs") &&
                                 !d.filesystem.starts_with("devtmpfs") &&
                                 !d.filesystem.starts_with("cgroup") &&
                                 !d.filesystem.starts_with("sysfs") &&
                                 !d.filesystem.starts_with("proc") &&
                                 !d.filesystem.starts_with("nsfs") &&
                                 !d.filesystem.starts_with("sunrpc") &&
                                 !d.filesystem.starts_with("pstore") &&
                                 !d.filesystem.starts_with("bpf") &&
                                 !d.filesystem.starts_with("configfs") &&
                                 !d.filesystem.starts_with("tracefs") &&
                                 !d.filesystem.starts_with("debugfs") &&
                                 !d.filesystem.starts_with("securityfs") &&
                                 !d.filesystem.starts_with("efivarfs") &&
                                 !d.filesystem.starts_with("fusectl") &&
                                 !d.filesystem.starts_with("mqueue") &&
                                 !d.filesystem.starts_with("hugetlbfs") &&
                                 !d.filesystem.starts_with("ramfs") &&
                                 !d.filesystem.starts_with("overlay");
            
            is_block_device && is_not_virtual && d.size > 0
        })
        .collect();

    let block = Block::default()
        .title(format!(" Disk Usage [{} filesystems] ", real_usage.len()))
        .borders(Borders::ALL)
        .border_style(border_style)
        .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if real_usage.is_empty() {
        f.render_widget(Paragraph::new("No filesystems").style(Style::default().fg(Color::Yellow)), inner);
        return;
    }

    let header_height = 1;
    let visible_rows = inner.height.saturating_sub(header_height) as usize;

    // Ensure scroll offset is valid
    if app.disk_usage_scroll > real_usage.len().saturating_sub(visible_rows) {
        app.disk_usage_scroll = real_usage.len().saturating_sub(visible_rows);
    }

    let rows: Vec<Row> = real_usage
        .iter()
        .skip(app.disk_usage_scroll)
        .take(visible_rows)
        .map(|d| {
            let use_style = if d.use_percent > 90.0 {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else if d.use_percent > 70.0 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Green)
            };

            // Show mount point as primary identifier
            let mount_display = truncate(&d.mounted_on, 20);
            let fs_display = if d.filesystem.contains(":") {
                // NFS mount - show server:path
                truncate(&d.filesystem, 18)
            } else {
                truncate(&d.device, 12)
            };

            Row::new(vec![
                Cell::from(Span::styled(mount_display, Style::default().fg(Color::Cyan))),
                Cell::from(Span::styled(fs_display, Style::default().fg(Color::Gray))),
                Cell::from(Span::styled(format_bytes_size(d.size), Style::default())),
                Cell::from(Span::styled(format_bytes_size(d.used), Style::default())),
                Cell::from(Span::styled(format_bytes_size(d.avail), Style::default())),
                Cell::from(Span::styled(format!("{:>4.0}%", d.use_percent), use_style)),
            ])
        })
        .collect();

    let header = Row::new(vec![
        Cell::from(Span::styled("Mounted On", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Source", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Size", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Used", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Avail", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Use%", Style::default().add_modifier(Modifier::BOLD))),
    ]);

    let table = Table::new(rows, [
        Constraint::Length(20),
        Constraint::Length(14),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Length(6),
    ])
    .header(header)
    .column_spacing(1);

    f.render_widget(table, inner);

    // Draw scrollbar if needed
    if real_usage.len() > visible_rows {
        let mut scrollbar_state = ScrollbarState::new(real_usage.len())
            .position(app.disk_usage_scroll)
            .viewport_content_length(visible_rows);
        
        let scrollbar_area = Rect {
            x: inner.x + inner.width.saturating_sub(1),
            y: inner.y + 1,
            width: 1,
            height: inner.height.saturating_sub(2),
        };
        
        f.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .thumb_style(if is_focused { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::DarkGray) })
                .track_style(Style::default().fg(Color::DarkGray)),
            scrollbar_area,
            &mut scrollbar_state,
        );
    }
}

fn draw_process_panel(f: &mut Frame, app: &App, deltas: &[ProcessDelta], area: Rect) {
    let is_focused = matches!(app.focus, FocusPanel::Processes);
    let border_style = if is_focused {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let filter_info = if let Some(ref user) = app.filter_user {
        format!("[User:{}]", user)
    } else if let Some(pid) = app.filter_pid {
        format!("[PID:{}]", pid)
    } else {
        "[All]".to_string()
    };

    let title = format!(" Processes {} Sort:{} ", filter_info, app.sort_by.name());

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style)
        .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let header_height = 1;
    let visible_rows = inner.height.saturating_sub(header_height) as usize;

    if deltas.is_empty() {
        f.render_widget(Paragraph::new("No processes found").style(Style::default().fg(Color::Yellow)), inner);
        return;
    }

    let rows: Vec<Row> = deltas
        .iter()
        .enumerate()
        .skip(app.scroll_offset)
        .take(visible_rows)
        .map(|(i, p)| {
            let is_selected = i == app.selected_index;
            let base_style = if is_selected {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let cpu_style = if p.cpu_percent > 80.0 {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else if p.cpu_percent > 50.0 {
                Style::default().fg(Color::Yellow)
            } else {
                base_style
            };

            let mem_style = if p.mem_percent > 50.0 {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else if p.mem_percent > 20.0 {
                Style::default().fg(Color::Yellow)
            } else {
                base_style
            };

            Row::new(vec![
                Cell::from(Span::styled(format!("{:>6}", p.pid), base_style)),
                Cell::from(Span::styled(truncate(&p.name, 12), base_style)),
                Cell::from(Span::styled(truncate(&p.user, 8), base_style)),
                Cell::from(Span::styled(format!("{:>5.1}", p.cpu_percent), cpu_style)),
                Cell::from(Span::styled(format!("{:>5.1}", p.mem_percent), mem_style)),
                Cell::from(Span::styled(format!("{:>3}", p.connections), base_style)),
                Cell::from(Span::styled(format_bytes_per_sec(p.read_bytes_sec), Style::default().fg(Color::Blue))),
                Cell::from(Span::styled(format_bytes_per_sec(p.write_bytes_sec), Style::default().fg(Color::Magenta))),
            ])
        })
        .collect();

    let header = Row::new(vec![
        Cell::from(Span::styled("PID", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Name", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("User", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("CPU%", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("MEM%", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Con", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("▼Read", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("▲Write", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))),
    ]);

    let table = Table::new(rows, [
        Constraint::Length(7),
        Constraint::Length(12),
        Constraint::Length(9),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(4),
        Constraint::Length(10),
        Constraint::Length(10),
    ])
    .header(header)
    .column_spacing(1);

    f.render_widget(table, inner);

    // Draw scrollbar for processes
    if deltas.len() > visible_rows {
        let mut scrollbar_state = ScrollbarState::new(deltas.len())
            .position(app.scroll_offset)
            .viewport_content_length(visible_rows);
        
        let scrollbar_area = Rect {
            x: inner.x + inner.width.saturating_sub(1),
            y: inner.y + 1,
            width: 1,
            height: inner.height.saturating_sub(2),
        };
        
        f.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .thumb_style(if is_focused { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) })
                .track_style(Style::default().fg(Color::DarkGray)),
            scrollbar_area,
            &mut scrollbar_state,
        );
    }
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let focus_indicator = format!("[{:?}]", app.focus);
    let status_text = match app.mode {
        AppMode::Normal => format!("{} q:Quit Tab:Focus ↑↓:Scroll u:User p:PID c:Clear s:Sort h:Help", focus_indicator),
        AppMode::FilterUser => format!(" User: {} [Enter=OK, Esc=Cancel]", app.input_buffer),
        AppMode::FilterPid => format!(" PID: {} [Enter=OK, Esc=Cancel]", app.input_buffer),
        AppMode::Help => " Press any key to close ".to_string(),
    };

    let style = match app.mode {
        AppMode::Normal => Style::default().bg(Color::DarkGray).fg(Color::White),
        AppMode::FilterUser | AppMode::FilterPid => Style::default().bg(Color::DarkGray).fg(Color::Yellow),
        AppMode::Help => Style::default().bg(Color::DarkGray).fg(Color::Green),
    };

    f.render_widget(Paragraph::new(status_text).style(style), area);
}

pub fn draw_help(f: &mut Frame) {
    let block = Block::default()
        .title(" ntop Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

    let area = centered_rect(60, 70, f.area());
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let help_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled("ntop", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)), Span::raw(" - Network & Disk I/O Monitor")]),
        Line::from(""),
        Line::from(vec![Span::styled("Focus Panels (Tab to cycle)", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  Tab       Cycle focus: Network → Disk I/O → Disk Usage → Processes"),
        Line::from("  ↑/↓       Scroll the focused panel"),
        Line::from(""),
        Line::from(vec![Span::styled("Navigation (Processes)", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  ↑/k       Move up        ↓/j    Move down"),
        Line::from(""),
        Line::from(vec![Span::styled("Filtering", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  u         Filter by username"),
        Line::from("  p         Filter by PID"),
        Line::from("  c         Clear filter"),
        Line::from(""),
        Line::from(vec![Span::styled("Sorting", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  s         Cycle: CPU% → MEM% → READ → WRITE → CONN → PID"),
        Line::from(""),
        Line::from(vec![Span::styled("General", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  h/?       Show help"),
        Line::from("  q         Quit"),
        Line::from(""),
        Line::from(Span::styled("  Press any key to close", Style::default().fg(Color::Yellow))),
    ];

    f.render_widget(Paragraph::new(help_text), inner);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        format!("{}…", s.chars().take(max_len.saturating_sub(1)).collect::<String>())
    } else {
        s.to_string()
    }
}
