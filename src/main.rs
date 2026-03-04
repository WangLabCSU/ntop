mod network;
mod disk;
mod process;
mod ui;

use std::io;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use network::NetworkCollector;
use disk::DiskCollector;
use process::ProcessCollector;
use ui::{App, AppMode, draw, draw_help};

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) -> Result<()> {
    let mut app = App::new();
    let mut net_collector = NetworkCollector::new();
    let mut disk_collector = DiskCollector::new();
    let mut proc_collector = ProcessCollector::new();

    let mut net_stats = Vec::new();
    let mut net_deltas = Vec::new();
    let mut disk_usage = Vec::new();
    let mut disk_deltas = Vec::new();
    let mut process_deltas = Vec::new();

    let mut last_data_update = Instant::now();
    let data_update_interval = Duration::from_millis(500);

    loop {
        let now = Instant::now();
        let should_update_data = now.duration_since(last_data_update) >= data_update_interval;

        while event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.mode {
                        AppMode::Normal => {
                            match key.code {
                                KeyCode::Char('q') => return Ok(()),
                                KeyCode::Char('h') | KeyCode::Char('?') => {
                                    app.mode = AppMode::Help;
                                }
                                KeyCode::Char('u') => {
                                    app.mode = AppMode::FilterUser;
                                    app.input_buffer.clear();
                                }
                                KeyCode::Char('p') => {
                                    app.mode = AppMode::FilterPid;
                                    app.input_buffer.clear();
                                }
                                KeyCode::Char('c') => {
                                    app.filter_user = None;
                                    app.filter_pid = None;
                                    app.reset_selection();
                                }
                                KeyCode::Char('s') => {
                                    app.cycle_sort();
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    let total = process_deltas.len().max(1);
                                    app.next(total);
                                    let visible = terminal.size()?.height.saturating_sub(24) as usize;
                                    app.scroll_down(visible);
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    let total = process_deltas.len().max(1);
                                    app.previous(total);
                                    app.scroll_up();
                                }
                                KeyCode::Esc => {
                                    app.filter_user = None;
                                    app.filter_pid = None;
                                    app.reset_selection();
                                }
                                _ => {}
                            }
                        }
                        AppMode::FilterUser => {
                            match key.code {
                                KeyCode::Enter => {
                                    if !app.input_buffer.is_empty() {
                                        app.filter_user = Some(app.input_buffer.clone());
                                        app.filter_pid = None;
                                    }
                                    app.mode = AppMode::Normal;
                                    app.input_buffer.clear();
                                    app.reset_selection();
                                }
                                KeyCode::Esc => {
                                    app.mode = AppMode::Normal;
                                    app.input_buffer.clear();
                                }
                                KeyCode::Backspace => {
                                    app.input_buffer.pop();
                                }
                                KeyCode::Char(c) => {
                                    app.input_buffer.push(c);
                                }
                                _ => {}
                            }
                        }
                        AppMode::FilterPid => {
                            match key.code {
                                KeyCode::Enter => {
                                    if let Ok(pid) = app.input_buffer.parse::<u32>() {
                                        app.filter_pid = Some(pid);
                                        app.filter_user = None;
                                    }
                                    app.mode = AppMode::Normal;
                                    app.input_buffer.clear();
                                    app.reset_selection();
                                }
                                KeyCode::Esc => {
                                    app.mode = AppMode::Normal;
                                    app.input_buffer.clear();
                                }
                                KeyCode::Backspace => {
                                    app.input_buffer.pop();
                                }
                                KeyCode::Char(c) if c.is_ascii_digit() => {
                                    app.input_buffer.push(c);
                                }
                                _ => {}
                            }
                        }
                        AppMode::Help => {
                            app.mode = AppMode::Normal;
                        }
                    }
                }
            }
        }

        if should_update_data {
            last_data_update = now;

            if let Ok((stats, deltas)) = net_collector.collect() {
                net_stats = stats;
                net_deltas = deltas;
            }

            if let Ok((usage, deltas)) = disk_collector.collect() {
                disk_usage = usage;
                disk_deltas = deltas;
            }

            if let Ok(deltas) = proc_collector.collect_delta() {
                process_deltas = if let Some(ref user) = app.filter_user {
                    deltas.into_iter().filter(|p| &p.user == user).collect()
                } else if let Some(pid) = app.filter_pid {
                    deltas.into_iter().filter(|p| p.pid == pid).collect()
                } else {
                    deltas
                };
            }
        }

        terminal.draw(|f| {
            draw(f, &mut app, &net_stats, &net_deltas, &disk_usage, &disk_deltas, &mut process_deltas);
            if app.mode == AppMode::Help {
                draw_help(f);
            }
        })?;
    }
}
