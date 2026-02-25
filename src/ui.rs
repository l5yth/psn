/*
   Copyright (C) 2026 l5yth

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
*/

//! TUI rendering helpers.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

use crate::{app::App, process::status_dot_color};

/// Build the table title based on filter and process count.
pub fn build_title(filter: Option<&str>, _count: usize) -> String {
    match filter {
        Some(filter_value) => format!("process status - filter: \"{}\"", filter_value),
        None => "process status".to_string(),
    }
}

/// Build the static help text.
pub fn build_help(count: usize) -> String {
    format!(
        "processes: {} | ↑/↓: select | 1-9: send signal (1-9) | r: refresh | q: quit",
        count
    )
}

/// Build the footer text with optional status suffix.
pub fn build_footer(help: &str, status: &str) -> String {
    if status.is_empty() {
        help.to_string()
    } else {
        format!("{}  —  {}", help, status)
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
        assert_eq!(build_title(None, 3), "process status");
        assert_eq!(
            build_title(Some("ssh"), 5),
            "process status - filter: \"ssh\""
        );
    }

    #[test]
    fn build_help_contains_count() {
        assert!(build_help(9).contains("processes: 9"));
    }

    #[test]
    fn build_footer_handles_empty_and_non_empty_status() {
        assert_eq!(build_footer("help", ""), "help");
        assert_eq!(build_footer("help", "ok"), "help  —  ok");
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

        assert!(text.contains("process status - filter: \"psn\""));
        assert!(text.contains("processes: 1"));
    }
}
