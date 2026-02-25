// Copyright (c) 2026 l5yth
// SPDX-License-Identifier: Apache-2.0

//! TUI rendering helpers.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

use crate::{app::App, process::status_dot_color};

/// Build the table title based on filter and process count.
pub fn build_title(filter: Option<&str>, count: usize) -> String {
    match filter {
        Some(filter_value) => format!("psn - filter: \"{}\" - {} procs", filter_value, count),
        None => format!("psn - {} procs", count),
    }
}

/// Build the static help text.
pub fn build_help(count: usize) -> String {
    format!(
        "procs: {} | up/down select | 1-9 send signal | r refresh | q quit",
        count
    )
}

/// Build the footer text with optional status suffix.
pub fn build_footer(help: &str, status: &str) -> String {
    if status.is_empty() {
        help.to_string()
    } else {
        format!("{}  -  {}", help, status)
    }
}

/// Render the full application frame.
pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let size = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(size);

    let header = Row::new([
        Cell::from(""),
        Cell::from("pid"),
        Cell::from("name"),
        Cell::from("status"),
        Cell::from("user"),
        Cell::from("command"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD));

    let body = app.rows.iter().map(|row| {
        Row::new([
            Cell::from("●").style(Style::default().fg(status_dot_color(row.status))),
            Cell::from(row.pid.to_string()),
            Cell::from(row.name.clone()),
            Cell::from(format!("{:?}", row.status)),
            Cell::from(row.user.clone()),
            Cell::from(row.cmd.clone()),
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
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(build_title(app.filter.as_deref(), app.rows.len())),
        )
        .column_spacing(1)
        .row_highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(table, chunks[0], &mut app.table_state);

    let help = build_help(app.rows.len());
    let footer = build_footer(&help, &app.status);
    frame.render_widget(
        Paragraph::new(footer).style(Style::default().fg(Color::DarkGray)),
        chunks[1],
    );
}

#[cfg(test)]
mod tests {
    use super::{build_footer, build_help, build_title, render};
    use crate::{app::App, model::ProcRow};
    use ratatui::{Terminal, backend::TestBackend};
    use sysinfo::ProcessStatus;

    fn sample_row() -> ProcRow {
        ProcRow {
            pid: 7,
            user: "alice".to_string(),
            status: ProcessStatus::Run,
            name: "psn".to_string(),
            cmd: "psn --demo".to_string(),
        }
    }

    #[test]
    fn build_title_handles_filter_and_plain_modes() {
        assert_eq!(build_title(None, 3), "psn - 3 procs");
        assert_eq!(
            build_title(Some("ssh"), 5),
            "psn - filter: \"ssh\" - 5 procs"
        );
    }

    #[test]
    fn build_help_contains_count() {
        assert!(build_help(9).contains("procs: 9"));
    }

    #[test]
    fn build_footer_handles_empty_and_non_empty_status() {
        assert_eq!(build_footer("help", ""), "help");
        assert_eq!(build_footer("help", "ok"), "help  -  ok");
    }

    #[test]
    fn render_draws_without_panic() {
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).expect("terminal must initialize");
        let mut app = App::with_rows(Some("psn".to_string()), vec![sample_row()]);

        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("render should succeed");

        let backend = terminal.backend();
        let buffer = backend.buffer().clone();
        let text: String = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("");

        assert!(text.contains("psn - filter: \"psn\" - 1 procs"));
        assert!(text.contains("procs: 1"));
    }
}
