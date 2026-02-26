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
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap},
};

use crate::{app::App, process::status_dot_color, tree::display_order_with_prefix};

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

    let tree_order = display_order_with_prefix(&app.rows);
    let body = tree_order.into_iter().map(|(idx, prefix)| {
        let row = &app.rows[idx];
        let tree_name = format!("{prefix}{}", row.name);
        Row::new([
            Cell::from("●").style(Style::default().fg(status_dot_color(row.status))),
            Cell::from(row.pid.to_string()),
            Cell::from(tree_name),
            Cell::from(format!("{:?}", row.status)),
            Cell::from(row.user.as_ref()),
            Cell::from(row.cmd.as_str()),
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

    if let Some(prompt) = app.confirmation_prompt() {
        let modal = centered_rect(80, 5, size);
        frame.render_widget(Clear, modal);
        frame.render_widget(
            Paragraph::new(prompt)
                .block(Block::default().borders(Borders::ALL).title("send signal"))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
            modal,
        );
    }
}

fn centered_rect(width_percent: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .split(vertical[1]);

    horizontal[1]
}

#[cfg(test)]
mod tests {
    use super::{build_footer, build_help, build_title, render};
    use crate::{app::App, model::ProcRow, tree::display_order_with_prefix};
    use ratatui::{Terminal, backend::TestBackend};
    use std::sync::Arc;
    use sysinfo::ProcessStatus;

    fn sample_row() -> ProcRow {
        ProcRow {
            pid: 7,
            ppid: None,
            ancestor_chain: Vec::new(),
            user: Arc::from("alice"),
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

    #[test]
    fn render_draws_confirmation_overlay_when_pending() {
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).expect("terminal must initialize");
        let mut app = App::with_rows(Some("psn".to_string()), vec![sample_row()]);
        app.begin_signal_confirmation(1);

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

        assert!(text.contains("send signal"));
        assert!(text.contains("confirm sending SIGHUP (1)"));
    }

    #[test]
    fn build_tree_order_nests_children_under_parent() {
        let rows = vec![
            ProcRow {
                pid: 1,
                ppid: None,
                ancestor_chain: Vec::new(),
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                name: "parent".to_string(),
                cmd: "/bin/parent".to_string(),
            },
            ProcRow {
                pid: 2,
                ppid: Some(1),
                ancestor_chain: vec![1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                name: "child".to_string(),
                cmd: "/bin/child".to_string(),
            },
            ProcRow {
                pid: 3,
                ppid: Some(2),
                ancestor_chain: vec![2, 1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                name: "grandchild".to_string(),
                cmd: "/bin/grandchild".to_string(),
            },
        ];
        let order = display_order_with_prefix(&rows);
        assert_eq!(
            order,
            vec![
                (0, "".to_string()),
                (1, "└─".to_string()),
                (2, "  └─".to_string())
            ]
        );
    }

    #[test]
    fn build_tree_order_draws_branch_segments() {
        let rows = vec![
            ProcRow {
                pid: 1,
                ppid: None,
                ancestor_chain: Vec::new(),
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                name: "parent".to_string(),
                cmd: "/bin/parent".to_string(),
            },
            ProcRow {
                pid: 2,
                ppid: Some(1),
                ancestor_chain: vec![1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                name: "child-a".to_string(),
                cmd: "/bin/child-a".to_string(),
            },
            ProcRow {
                pid: 3,
                ppid: Some(1),
                ancestor_chain: vec![1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                name: "child-b".to_string(),
                cmd: "/bin/child-b".to_string(),
            },
            ProcRow {
                pid: 4,
                ppid: Some(2),
                ancestor_chain: vec![2, 1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                name: "grandchild".to_string(),
                cmd: "/bin/grandchild".to_string(),
            },
        ];

        let order = display_order_with_prefix(&rows);
        assert_eq!(
            order,
            vec![
                (0, "".to_string()),
                (1, "├─".to_string()),
                (3, "│ └─".to_string()),
                (2, "└─".to_string())
            ]
        );
    }

    #[test]
    fn build_tree_order_sorts_siblings_by_status_then_pid() {
        let rows = vec![
            ProcRow {
                pid: 1,
                ppid: None,
                ancestor_chain: Vec::new(),
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                name: "parent".to_string(),
                cmd: "/bin/parent".to_string(),
            },
            ProcRow {
                pid: 30,
                ppid: Some(1),
                ancestor_chain: vec![1],
                user: Arc::from("u"),
                status: ProcessStatus::Sleep,
                name: "child-sleep".to_string(),
                cmd: "/bin/child-sleep".to_string(),
            },
            ProcRow {
                pid: 40,
                ppid: Some(1),
                ancestor_chain: vec![1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                name: "child-run-high".to_string(),
                cmd: "/bin/child-run-high".to_string(),
            },
            ProcRow {
                pid: 20,
                ppid: Some(1),
                ancestor_chain: vec![1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                name: "child-run-low".to_string(),
                cmd: "/bin/child-run-low".to_string(),
            },
        ];

        let order = display_order_with_prefix(&rows);
        assert_eq!(
            order,
            vec![
                (0, "".to_string()),
                (3, "├─".to_string()),
                (2, "├─".to_string()),
                (1, "└─".to_string())
            ]
        );
    }

    #[test]
    fn build_tree_order_reattaches_to_nearest_visible_ancestor() {
        let rows = vec![
            ProcRow {
                pid: 1,
                ppid: None,
                ancestor_chain: Vec::new(),
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                name: "parent".to_string(),
                cmd: "/bin/parent".to_string(),
            },
            ProcRow {
                pid: 3,
                ppid: Some(2),
                ancestor_chain: vec![2, 1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                name: "grandchild".to_string(),
                cmd: "/bin/grandchild".to_string(),
            },
        ];
        let order = display_order_with_prefix(&rows);
        assert_eq!(order, vec![(0, "".to_string()), (1, "└─".to_string())]);
    }
}
