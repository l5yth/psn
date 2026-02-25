use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
};
use std::{cmp::min, io, time::Duration};
use sysinfo::{ProcessStatus, ProcessesToUpdate, System};

#[derive(Clone, Debug)]
struct ProcRow {
    pid: i32,
    user: String,
    status: ProcessStatus,
    name: String,
    cmd: String,
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(mut terminal: Terminal<CrosstermBackend<io::Stdout>>) {
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();
}

fn to_user(uid: Option<&sysinfo::Uid>) -> String {
    // sysinfo gives Uid on Linux; we resolve to username when possible.
    // If not available, show numeric-ish fallback.
    if let Some(uid) = uid {
        let s = uid.to_string();
        if let Ok(uid_num) = s.parse::<u32>() {
            if let Some(u) = users::get_user_by_uid(uid_num) {
                return u.name().to_string_lossy().to_string();
            }
        }
        return s;
    }
    "?".to_string()
}

fn matches_filter(row: &ProcRow, filter: &Option<String>) -> bool {
    match filter {
        None => true,
        Some(f) => {
            let f = f.to_lowercase();
            row.name.to_lowercase().contains(&f) || row.cmd.to_lowercase().contains(&f)
        }
    }
}

fn refresh_rows(sys: &mut System, filter: &Option<String>) -> Vec<ProcRow> {
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.refresh_cpu_all(); // helps cpu% settle
    sys.refresh_memory();

    let mut rows: Vec<ProcRow> = sys
        .processes()
        .values()
        .map(|p| {
            let pid = p.pid().as_u32() as i32;
            let user = to_user(p.user_id());
            let status = p.status();
            let name = p.name().to_string_lossy().to_string();

            let cmd_vec = p.cmd();
            let cmd = if cmd_vec.is_empty() {
                // fallback to exe if cmdline unavailable
                p.exe()
                    .map(|x| x.to_string_lossy().to_string())
                    .unwrap_or_else(|| "".to_string())
            } else {
                cmd_vec
                    .iter()
                    .map(|x| x.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ")
            };

            ProcRow {
                pid,
                user,
                status,
                name,
                cmd,
            }
        })
        .filter(|r| matches_filter(r, filter))
        .collect();

    // Keep a stable, non-resource-centric ordering.
    rows.sort_by(|a, b| {
        status_priority(a.status)
            .cmp(&status_priority(b.status))
            .then(a.name.cmp(&b.name))
            .then(a.pid.cmp(&b.pid))
    });

    rows
}

fn status_priority(status: ProcessStatus) -> u8 {
    match status {
        ProcessStatus::Run => 0,
        ProcessStatus::Sleep => 1,
        ProcessStatus::Idle => 2,
        ProcessStatus::Waking => 3,
        ProcessStatus::Parked => 4,
        ProcessStatus::Suspended => 5,
        ProcessStatus::Stop => 6,
        ProcessStatus::Tracing => 7,
        ProcessStatus::UninterruptibleDiskSleep => 8,
        ProcessStatus::LockBlocked => 9,
        ProcessStatus::Wakekill => 10,
        ProcessStatus::Zombie => 11,
        ProcessStatus::Dead => 12,
        ProcessStatus::Unknown(_) => 13,
    }
}

fn status_dot_color(status: ProcessStatus) -> Color {
    match status {
        ProcessStatus::Run => Color::Green,
        ProcessStatus::Sleep | ProcessStatus::Idle => Color::Yellow,
        ProcessStatus::Stop
        | ProcessStatus::Tracing
        | ProcessStatus::Zombie
        | ProcessStatus::Dead
        | ProcessStatus::Wakekill => Color::Red,
        _ => Color::DarkGray,
    }
}

fn signal_from_digit(d: u8) -> Option<Signal> {
    // Map 1-9 to actual Unix signal numbers.
    // nix::Signal covers common ones; for 1-9 these exist.
    match d {
        1 => Some(Signal::SIGHUP),
        2 => Some(Signal::SIGINT),
        3 => Some(Signal::SIGQUIT),
        4 => Some(Signal::SIGILL),
        5 => Some(Signal::SIGTRAP),
        6 => Some(Signal::SIGABRT),
        7 => Some(Signal::SIGBUS),
        8 => Some(Signal::SIGFPE),
        9 => Some(Signal::SIGKILL),
        _ => None,
    }
}

fn main() -> Result<()> {
    let filter = std::env::args().nth(1);

    let mut terminal = setup_terminal()?;
    let mut sys = System::new_all();

    let mut rows: Vec<ProcRow> = refresh_rows(&mut sys, &filter);
    let mut table_state = TableState::default();
    table_state.select(if rows.is_empty() { None } else { Some(0) });

    let mut status = String::new();

    let run = (|| -> Result<()> {
        loop {
            terminal.draw(|f| {
                let size = f.area();
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(2)])
                    .split(size);

                let title = match &filter {
                    Some(flt) => format!("psn — filter: \"{}\" — {} procs", flt, rows.len()),
                    None => format!("psn — {} procs", rows.len()),
                };

                let header = Row::new([
                    Cell::from(""),
                    Cell::from("pid"),
                    Cell::from("name"),
                    Cell::from("status"),
                    Cell::from("user"),
                    Cell::from("command"),
                ])
                .style(Style::default().add_modifier(Modifier::BOLD));

                let body = rows.iter().map(|r| {
                    Row::new([
                        Cell::from("●").style(Style::default().fg(status_dot_color(r.status))),
                        Cell::from(r.pid.to_string()),
                        Cell::from(r.name.clone()),
                        Cell::from(format!("{:?}", r.status)),
                        Cell::from(r.user.clone()),
                        Cell::from(r.cmd.clone()),
                    ])
                });

                let widths = [
                    Constraint::Length(1),
                    Constraint::Length(7),
                    Constraint::Length(18),
                    Constraint::Length(12),
                    Constraint::Length(12),
                    Constraint::Min(20),
                ];

                let table = Table::new(body, widths)
                    .header(header)
                    .block(Block::default().borders(Borders::ALL).title(title))
                    .column_spacing(1)
                    .row_highlight_style(
                        Style::default()
                            .add_modifier(Modifier::REVERSED)
                            .add_modifier(Modifier::BOLD),
                    );

                f.render_stateful_widget(table, chunks[0], &mut table_state);

                let help = format!(
                    "procs: {} | ↑/↓ select | 1-9: send kill signal 1-9 | r: refresh | q: quit",
                    rows.len()
                );
                let footer = if status.is_empty() {
                    help
                } else {
                    format!("{}  —  {}", help, status)
                };

                f.render_widget(
                    Paragraph::new(footer).style(Style::default().fg(Color::DarkGray)),
                    chunks[1],
                );
            })?;

            // input
            if event::poll(Duration::from_millis(60))? {
                if let Event::Key(k) = event::read()? {
                    if k.kind != KeyEventKind::Press {
                        continue;
                    }
                    match k.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('r') => {
                            rows = refresh_rows(&mut sys, &filter);
                            let sel = table_state.selected().unwrap_or(0);
                            if rows.is_empty() {
                                table_state.select(None);
                            } else {
                                table_state.select(Some(min(sel, rows.len() - 1)));
                            }
                            status.clear();
                        }
                        KeyCode::Up => {
                            if let Some(sel) = table_state.selected() {
                                if sel > 0 {
                                    table_state.select(Some(sel - 1));
                                }
                            }
                        }
                        KeyCode::Down => {
                            if let Some(sel) = table_state.selected() {
                                if !rows.is_empty() && sel + 1 < rows.len() {
                                    table_state.select(Some(sel + 1));
                                }
                            } else if !rows.is_empty() {
                                table_state.select(Some(0));
                            }
                        }
                        KeyCode::Char(c) if c.is_ascii_digit() => {
                            let d = c.to_digit(10).unwrap() as u8;
                            if (1..=9).contains(&d) {
                                if let Some(sel) = table_state.selected() {
                                    if let Some(row) = rows.get(sel) {
                                        if let Some(sig) = signal_from_digit(d) {
                                            let pid = Pid::from_raw(row.pid);
                                            match kill(pid, sig) {
                                                Ok(_) => {
                                                    status = format!(
                                                        "sent {:?} ({}) to pid {}",
                                                        sig, d, row.pid
                                                    )
                                                }
                                                Err(e) => {
                                                    status = format!(
                                                        "failed to signal pid {}: {}",
                                                        row.pid, e
                                                    )
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    })();

    restore_terminal(terminal);
    run
}
