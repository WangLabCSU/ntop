use crate::network::{NetworkStats, NetworkStatsDelta, format_bytes_per_sec as net_format_bytes_per_sec, format_bytes};
use crate::disk::{DiskIoDelta, DiskUsage, format_iops, format_bytes_per_sec, format_bytes_size};
use crate::process::ProcessDelta;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect, Alignment},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table, Paragraph, Clear, Gauge},
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
            SortBy::Cpu => "CPU",
            SortBy::Mem => "MEM",
            SortBy::ReadIO => "DiskRd",
            SortBy::WriteIO => "DiskWr",
            SortBy::Connections => "Conns",
            SortBy::Pid => "PID",
        }
    }
}

pub struct App {
    pub mode: AppMode,
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
}

impl App {
    pub fn new() -> Self {
        Self {
            mode: AppMode::Normal,
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
        }
    }

    pub fn next(&mut self, total: usize) {
        if total == 0 {
            return;
        }
        self.selected_index = (self.selected_index + 1) % total;
    }

    pub fn previous(&mut self, total: usize) {
        if total == 0 {
            return;
        }
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else {
            self.selected_index = total - 1;
        }
    }

    pub fn scroll_down(&mut self, visible: usize) {
        if self.selected_index >= self.scroll_offset + visible {
            self.scroll_offset = self.selected_index - visible + 1;
        }
    }

    pub fn scroll_up(&mut self) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
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

    pub fn reset_selection(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }
}

pub fn draw(f: &mut Frame, app: &mut App, net_stats: &[NetworkStats], net_deltas: &[NetworkStatsDelta],
            disk_usage: &[DiskUsage], disk_deltas: &[DiskIoDelta], process_deltas: &mut [ProcessDelta]) {
    app.total_rx_rate = net_deltas.iter().map(|d| d.rx_bytes_sec).sum();
    app.total_tx_rate = net_deltas.iter().map(|d| d.tx_bytes_sec).sum();
    app.total_disk_read = disk_deltas.iter().map(|d| d.read_bytes_sec).sum();
    app.total_disk_write = disk_deltas.iter().map(|d| d.write_bytes_sec).sum();

    sort_processes(process_deltas, &app.sort_by);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(4),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(f.area());

    draw_header(f, app, chunks[0]);
    draw_network_panel(f, net_stats, net_deltas, chunks[1]);
    draw_disk_io_panel(f, disk_deltas, chunks[2]);
    draw_disk_usage_panel(f, disk_usage, chunks[3]);
    draw_process_panel(f, app, process_deltas, chunks[4]);
    draw_status_bar(f, app, chunks[5]);
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

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ])
        .split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled("ntop", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]))
    .alignment(Alignment::Left);
    f.render_widget(title, chunks[0]);

    let net_rx = Paragraph::new(Line::from(vec![
        Span::styled("▼ NET ", Style::default().fg(Color::Green)),
        Span::styled(net_format_bytes_per_sec(app.total_rx_rate), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(net_rx, chunks[1]);

    let net_tx = Paragraph::new(Line::from(vec![
        Span::styled("▲ NET ", Style::default().fg(Color::Red)),
        Span::styled(net_format_bytes_per_sec(app.total_tx_rate), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(net_tx, chunks[2]);

    let disk_rd = Paragraph::new(Line::from(vec![
        Span::styled("▼ DISK ", Style::default().fg(Color::Blue)),
        Span::styled(format_bytes_per_sec(app.total_disk_read), Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(disk_rd, chunks[3]);

    let disk_wr = Paragraph::new(Line::from(vec![
        Span::styled("▲ DISK ", Style::default().fg(Color::Magenta)),
        Span::styled(format_bytes_per_sec(app.total_disk_write), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(disk_wr, chunks[4]);
}

fn draw_network_panel(f: &mut Frame, stats: &[NetworkStats], deltas: &[NetworkStatsDelta], area: Rect) {
    let block = Block::default()
        .title(" Network ")
        .borders(Borders::ALL)
        .title_style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if stats.is_empty() {
        return;
    }

    let rows: Vec<Row> = stats
        .iter()
        .zip(deltas.iter())
        .map(|(s, d)| {
            Row::new(vec![
                Cell::from(Span::styled(&s.interface, Style::default().fg(Color::Cyan))),
                Cell::from(Span::styled(net_format_bytes_per_sec(d.rx_bytes_sec), Style::default().fg(Color::Green))),
                Cell::from(Span::styled(net_format_bytes_per_sec(d.tx_bytes_sec), Style::default().fg(Color::Red))),
                Cell::from(Span::raw(format_bytes(s.rx_bytes as f64))),
                Cell::from(Span::raw(format_bytes(s.tx_bytes as f64))),
            ])
        })
        .collect();

    let header = Row::new(vec![
        Cell::from(Span::styled("Interface", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("▼ RX/s", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("▲ TX/s", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("RX Total", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("TX Total", Style::default().add_modifier(Modifier::BOLD))),
    ]);

    let table = Table::new(rows, [Constraint::Length(12), Constraint::Length(12), Constraint::Length(12), Constraint::Length(12), Constraint::Length(12)])
        .header(header)
        .column_spacing(2);

    f.render_widget(table, inner);
}

fn draw_disk_io_panel(f: &mut Frame, deltas: &[DiskIoDelta], area: Rect) {
    let block = Block::default()
        .title(" Disk I/O ")
        .borders(Borders::ALL)
        .title_style(Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if deltas.is_empty() {
        return;
    }

    let rows: Vec<Row> = deltas
        .iter()
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
                Cell::from(Span::styled(format_iops(d.read_iops), Style::default())),
                Cell::from(Span::styled(format_iops(d.write_iops), Style::default())),
                Cell::from(Span::styled(format!("{:.0}%", d.io_util), util_style)),
            ])
        })
        .collect();

    let header = Row::new(vec![
        Cell::from(Span::styled("Device", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("▼ Read/s", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("▲ Write/s", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Rd IOPS", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Wr IOPS", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Util%", Style::default().add_modifier(Modifier::BOLD))),
    ]);

    let table = Table::new(rows, [Constraint::Length(10), Constraint::Length(12), Constraint::Length(12), Constraint::Length(10), Constraint::Length(10), Constraint::Length(8)])
        .header(header)
        .column_spacing(2);

    f.render_widget(table, inner);
}

fn draw_disk_usage_panel(f: &mut Frame, usage: &[DiskUsage], area: Rect) {
    let block = Block::default()
        .title(" Disk Usage ")
        .borders(Borders::ALL)
        .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if usage.is_empty() {
        return;
    }

    let rows: Vec<Row> = usage
        .iter()
        .map(|d| {
            let use_style = if d.use_percent > 80.0 {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else if d.use_percent > 50.0 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Green)
            };

            let fs_display = if d.filesystem.starts_with("192.") || d.filesystem.contains(":") {
                truncate(&d.filesystem, 20)
            } else {
                truncate(&d.device, 20)
            };

            Row::new(vec![
                Cell::from(Span::styled(fs_display, Style::default().fg(Color::Cyan))),
                Cell::from(Span::styled(format_bytes_size(d.size), Style::default())),
                Cell::from(Span::styled(format_bytes_size(d.used), Style::default())),
                Cell::from(Span::styled(format_bytes_size(d.avail), Style::default())),
                Cell::from(Span::styled(format!("{:.0}%", d.use_percent), use_style)),
                Cell::from(Span::styled(truncate(&d.mounted_on, 20), Style::default().fg(Color::Gray))),
            ])
        })
        .collect();

    let header = Row::new(vec![
        Cell::from(Span::styled("Device/FS", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Size", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Used", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Avail", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Use%", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Mounted", Style::default().add_modifier(Modifier::BOLD))),
    ]);

    let table = Table::new(rows, [Constraint::Length(20), Constraint::Length(8), Constraint::Length(8), Constraint::Length(8), Constraint::Length(6), Constraint::Length(20)])
        .header(header)
        .column_spacing(2);

    f.render_widget(table, inner);
}

fn draw_process_panel(f: &mut Frame, app: &App, deltas: &[ProcessDelta], area: Rect) {
    let filter_info = if let Some(ref user) = app.filter_user {
        format!("[User: {}]", user)
    } else if let Some(pid) = app.filter_pid {
        format!("[PID: {}]", pid)
    } else {
        "[All]".to_string()
    };

    let sort_indicator = format!("Sort: {}", app.sort_by.name());
    let title = format!(" Processes {} {} ", filter_info, sort_indicator);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let visible_rows = inner.height.saturating_sub(2) as usize;

    if deltas.is_empty() {
        let msg = Paragraph::new("No processes found")
            .style(Style::default().fg(Color::Yellow));
        f.render_widget(msg, inner);
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

            let disk_rd_style = if p.read_bytes_sec > 1024.0 * 1024.0 {
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Blue)
            };

            let disk_wr_style = if p.write_bytes_sec > 1024.0 * 1024.0 {
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Magenta)
            };

            Row::new(vec![
                Cell::from(Span::styled(format!("{:>6}", p.pid), base_style)),
                Cell::from(Span::styled(truncate(&p.name, 12), base_style)),
                Cell::from(Span::styled(truncate(&p.user, 10), base_style)),
                Cell::from(Span::styled(format!("{:>5.1}", p.cpu_percent), cpu_style)),
                Cell::from(Span::styled(format!("{:>5.1}", p.mem_percent), mem_style)),
                Cell::from(Span::styled(format!("{:>4}", p.connections), base_style)),
                Cell::from(Span::styled(format_bytes_per_sec(p.read_bytes_sec), disk_rd_style)),
                Cell::from(Span::styled(format_bytes_per_sec(p.write_bytes_sec), disk_wr_style)),
            ])
        })
        .collect();

    let header = Row::new(vec![
        Cell::from(Span::styled("  PID", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Name", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("User", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("CPU%", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("MEM%", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Conns", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("▼ DiskRd", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("▲ DiskWr", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))),
    ]);

    let table = Table::new(rows, [
        Constraint::Length(8),
        Constraint::Length(13),
        Constraint::Length(11),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(12),
        Constraint::Length(12),
    ])
    .header(header)
    .column_spacing(1);

    f.render_widget(table, inner);
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let status_text = match app.mode {
        AppMode::Normal => " q:Quit | u:User | p:PID | c:Clear | s:Sort | ↑↓:Nav | h:Help ",
        AppMode::FilterUser => Box::leak(format!(" Enter user: {} [Enter=OK, Esc=Cancel]", app.input_buffer).into_boxed_str()),
        AppMode::FilterPid => Box::leak(format!(" Enter PID: {} [Enter=OK, Esc=Cancel]", app.input_buffer).into_boxed_str()),
        AppMode::Help => " Press any key to close ",
    };

    let style = match app.mode {
        AppMode::Normal => Style::default().bg(Color::DarkGray).fg(Color::White),
        AppMode::FilterUser | AppMode::FilterPid => Style::default().bg(Color::DarkGray).fg(Color::Yellow),
        AppMode::Help => Style::default().bg(Color::DarkGray).fg(Color::Green),
    };

    let paragraph = Paragraph::new(status_text).style(style);
    f.render_widget(paragraph, area);
}

pub fn draw_help(f: &mut Frame) {
    let block = Block::default()
        .title(" ntop Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

    let area = centered_rect(60, 65, f.area());
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let help_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled("ntop", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)), Span::raw(" - Network & Disk I/O Monitor")]),
        Line::from(""),
        Line::from(vec![Span::styled("Navigation", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  ↑/k    Move up    ↓/j    Move down"),
        Line::from(""),
        Line::from(vec![Span::styled("Filtering", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  u      Filter by username"),
        Line::from("  p      Filter by PID"),
        Line::from("  c      Clear filter (show all)"),
        Line::from(""),
        Line::from(vec![Span::styled("Sorting (s to cycle)", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  CPU → MEM → DiskRd → DiskWr → Conns → PID"),
        Line::from(""),
        Line::from(vec![Span::styled("General", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  h/?    Show help    q    Quit"),
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
