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

use crate::{
    app::App,
    process::{FilterSpec, status_dot_color},
    tree::display_rows,
};

const COLUMN_HEADERS: [&str; 6] = ["", "pid", "name", "command", "status", "user"];

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
        "processes: {} | ↑/↓: select | ←/→: collapse/expand | 1-9: send signal | /: filter | r: refresh | q: quit",
        count
    )
}

/// Return styled spans for `text` with all occurrences of the active filter highlighted.
/// The prefix connector (tree characters) must be prepended by the caller as a plain span.
fn highlight_matches(text: &str, filter: Option<&FilterSpec>) -> Vec<Span<'static>> {
    let highlight = Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let Some(filter) = filter else {
        return vec![Span::raw(text.to_owned())];
    };

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut last = 0usize;

    match filter {
        FilterSpec::Substring {
            lowered,
            raw,
            ascii_only,
        } => {
            let text_lower = if *ascii_only {
                text.to_ascii_lowercase()
            } else {
                text.to_lowercase()
            };
            let mut pos = 0usize;
            while pos < text_lower.len() {
                match text_lower[pos..].find(lowered.as_str()) {
                    None => break,
                    Some(rel) => {
                        let start = pos + rel;
                        let end = start + raw.len();
                        if start > last {
                            spans.push(Span::raw(text[last..start].to_owned()));
                        }
                        spans.push(Span::styled(text[start..end].to_owned(), highlight));
                        last = end;
                        pos = end.max(pos + 1);
                    }
                }
            }
        }
        FilterSpec::Regex(re) => {
            for m in re.find_iter(text) {
                if m.start() > last {
                    spans.push(Span::raw(text[last..m.start()].to_owned()));
                }
                spans.push(Span::styled(text[m.start()..m.end()].to_owned(), highlight));
                last = m.end();
            }
        }
    }

    if last < text.len() {
        spans.push(Span::raw(text[last..].to_owned()));
    }

    if spans.is_empty() {
        vec![Span::raw(text.to_owned())]
    } else {
        spans
    }
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

    let header = Row::new(COLUMN_HEADERS.map(Cell::from))
        .style(Style::default().add_modifier(Modifier::BOLD));

    // Clone the active filter upfront to avoid borrow conflicts with &mut app.table_state.
    let active_filter: Option<FilterSpec> = app
        .filter_input
        .as_ref()
        .and_then(|fi| fi.compiled.clone())
        .or_else(|| app.compiled_filter.clone());

    // Title shows the interactive input text while typing, otherwise the confirmed filter.
    let title_text: Option<String> = app
        .filter_input
        .as_ref()
        .map(|fi| fi.text.clone())
        .or_else(|| app.filter.clone());

    let tree_order = display_rows(&app.rows, &app.collapsed_pids);
    let body = tree_order.into_iter().map(|display_row| {
        let row = &app.rows[display_row.row_index];
        let name = if display_row.is_collapsed {
            format!("{} [...]", row.name)
        } else {
            row.name.clone()
        };

        // Prefix (tree connectors) is plain; only the name portion is highlighted.
        let mut name_spans = vec![Span::raw(display_row.prefix.clone())];
        name_spans.extend(highlight_matches(&name, active_filter.as_ref()));

        let cmd_spans = highlight_matches(&row.cmd, active_filter.as_ref());

        Row::new([
            Cell::from("●").style(Style::default().fg(status_dot_color(row.status))),
            Cell::from(row.pid.to_string()),
            Cell::from(Line::from(name_spans)),
            Cell::from(Line::from(cmd_spans)),
            Cell::from(format!("{:?}", row.status)),
            Cell::from(row.user.as_ref()),
        ])
    });

    let widths = [
        Constraint::Length(1),
        Constraint::Length(7),
        Constraint::Min(24),
        Constraint::Min(16),
        Constraint::Length(12),
        Constraint::Length(12),
    ];

    let table = Table::new(body, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(build_title(title_text.as_deref(), app.rows.len())),
        )
        .column_spacing(1)
        .row_highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(table, chunks[0], &mut app.table_state);

    if let Some(ref fi) = app.filter_input {
        frame.render_widget(Paragraph::new(format!("/ {}█", fi.text)), chunks[1]);
    } else {
        let help = build_help(app.rows.len());
        let footer = build_footer(&help, &app.status);
        frame.render_widget(
            Paragraph::new(footer).style(Style::default().fg(Color::DarkGray)),
            chunks[1],
        );
    }

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
    use super::{COLUMN_HEADERS, build_footer, build_help, build_title, highlight_matches, render};
    use crate::{app::App, model::ProcRow, tree::display_order_with_prefix};
    use crate::{app::FilterInput, process};
    use ratatui::{Terminal, backend::TestBackend};
    use std::{collections::HashSet, sync::Arc};
    use sysinfo::ProcessStatus;

    fn sample_row() -> ProcRow {
        ProcRow {
            pid: 7,
            start_time: 0,
            ppid: None,
            ancestor_chain: Vec::new(),
            user: Arc::from("alice"),
            status: ProcessStatus::Run,
            cpu_usage_tenths: 0,
            memory_bytes: 0,
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
        assert!(build_help(9).contains("←/→: collapse/expand"));
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
                start_time: 0,
                ppid: None,
                ancestor_chain: Vec::new(),
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 100,
                memory_bytes: 0,
                name: "parent".to_string(),
                cmd: "/bin/parent".to_string(),
            },
            ProcRow {
                pid: 2,
                start_time: 0,
                ppid: Some(1),
                ancestor_chain: vec![1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 0,
                memory_bytes: 0,
                name: "child".to_string(),
                cmd: "/bin/child".to_string(),
            },
            ProcRow {
                pid: 3,
                start_time: 0,
                ppid: Some(2),
                ancestor_chain: vec![2, 1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 0,
                memory_bytes: 0,
                name: "grandchild".to_string(),
                cmd: "/bin/grandchild".to_string(),
            },
        ];
        let order = display_order_with_prefix(&rows, &HashSet::new());
        assert_eq!(
            order,
            vec![
                (0, "".to_string()),
                (1, "".to_string()),
                (2, "└─".to_string())
            ]
        );
    }

    #[test]
    fn build_tree_order_draws_branch_segments() {
        let rows = vec![
            ProcRow {
                pid: 1,
                start_time: 0,
                ppid: None,
                ancestor_chain: Vec::new(),
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 100,
                memory_bytes: 0,
                name: "parent".to_string(),
                cmd: "/bin/parent".to_string(),
            },
            ProcRow {
                pid: 2,
                start_time: 0,
                ppid: Some(1),
                ancestor_chain: vec![1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 5,
                memory_bytes: 100,
                name: "child-a".to_string(),
                cmd: "/bin/child-a".to_string(),
            },
            ProcRow {
                pid: 3,
                start_time: 0,
                ppid: Some(1),
                ancestor_chain: vec![1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 2,
                memory_bytes: 100,
                name: "child-b".to_string(),
                cmd: "/bin/child-b".to_string(),
            },
            ProcRow {
                pid: 4,
                start_time: 0,
                ppid: Some(2),
                ancestor_chain: vec![2, 1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 0,
                memory_bytes: 0,
                name: "grandchild".to_string(),
                cmd: "/bin/grandchild".to_string(),
            },
        ];

        let order = display_order_with_prefix(&rows, &HashSet::new());
        assert_eq!(
            order,
            vec![
                (0, "".to_string()),
                (1, "".to_string()),
                (3, "└─".to_string()),
                (2, "".to_string())
            ]
        );
    }

    #[test]
    fn build_tree_order_sorts_siblings_by_status_then_pid() {
        let rows = vec![
            ProcRow {
                pid: 1,
                start_time: 0,
                ppid: None,
                ancestor_chain: Vec::new(),
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 100,
                memory_bytes: 0,
                name: "parent".to_string(),
                cmd: "/bin/parent".to_string(),
            },
            ProcRow {
                pid: 30,
                start_time: 0,
                ppid: Some(1),
                ancestor_chain: vec![1],
                user: Arc::from("u"),
                status: ProcessStatus::Sleep,
                cpu_usage_tenths: 0,
                memory_bytes: 0,
                name: "child-sleep".to_string(),
                cmd: "/bin/child-sleep".to_string(),
            },
            ProcRow {
                pid: 40,
                start_time: 0,
                ppid: Some(1),
                ancestor_chain: vec![1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 10,
                memory_bytes: 100,
                name: "child-run-high".to_string(),
                cmd: "/bin/child-run-high".to_string(),
            },
            ProcRow {
                pid: 20,
                start_time: 0,
                ppid: Some(1),
                ancestor_chain: vec![1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 20,
                memory_bytes: 100,
                name: "child-run-low".to_string(),
                cmd: "/bin/child-run-low".to_string(),
            },
        ];

        let order = display_order_with_prefix(&rows, &HashSet::new());
        assert_eq!(
            order,
            vec![
                (0, "".to_string()),
                (3, "".to_string()),
                (2, "".to_string()),
                (1, "".to_string())
            ]
        );
    }

    #[test]
    fn build_tree_order_reattaches_to_nearest_visible_ancestor() {
        let rows = vec![
            ProcRow {
                pid: 1,
                start_time: 0,
                ppid: None,
                ancestor_chain: Vec::new(),
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 0,
                memory_bytes: 0,
                name: "parent".to_string(),
                cmd: "/bin/parent".to_string(),
            },
            ProcRow {
                pid: 3,
                start_time: 0,
                ppid: Some(2),
                ancestor_chain: vec![2, 1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 0,
                memory_bytes: 0,
                name: "grandchild".to_string(),
                cmd: "/bin/grandchild".to_string(),
            },
        ];
        let order = display_order_with_prefix(&rows, &HashSet::new());
        assert_eq!(order, vec![(0, "".to_string()), (1, "└─".to_string())]);
    }

    #[test]
    fn render_uses_reordered_columns() {
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).expect("terminal must initialize");
        let mut app = App::with_rows(None, vec![sample_row()]);

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
        assert_eq!(
            COLUMN_HEADERS,
            ["", "pid", "name", "command", "status", "user"]
        );
        assert!(text.contains("psn --demo"));
        assert!(text.contains("Run"));
        assert!(text.contains("alice"));
    }

    #[test]
    fn render_marks_collapsed_tree_rows() {
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).expect("terminal must initialize");
        let mut app = App::with_rows(
            None,
            vec![
                ProcRow {
                    pid: 2,
                    start_time: 0,
                    ppid: Some(1),
                    ancestor_chain: vec![1],
                    user: Arc::from("u"),
                    status: ProcessStatus::Run,
                    cpu_usage_tenths: 0,
                    memory_bytes: 0,
                    name: "service".to_string(),
                    cmd: "/bin/service".to_string(),
                },
                ProcRow {
                    pid: 3,
                    start_time: 0,
                    ppid: Some(2),
                    ancestor_chain: vec![2, 1],
                    user: Arc::from("u"),
                    status: ProcessStatus::Run,
                    cpu_usage_tenths: 0,
                    memory_bytes: 0,
                    name: "worker".to_string(),
                    cmd: "/bin/worker".to_string(),
                },
            ],
        );
        app.collapsed_pids.insert(2);

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

        assert!(text.contains("service [...]"));
        assert!(!text.contains("worker"));
    }

    #[test]
    fn highlight_matches_no_filter_returns_single_plain_span() {
        let spans = highlight_matches("hello world", None);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "hello world");
        assert_eq!(spans[0].style, ratatui::style::Style::default());
    }

    #[test]
    fn highlight_matches_substring_no_match_returns_plain_span() {
        let filter = process::compile_filter(Some("xyz".to_string()), false)
            .ok()
            .flatten();
        let spans = highlight_matches("hello world", filter.as_ref());
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "hello world");
    }

    #[test]
    fn highlight_matches_substring_single_match_returns_three_spans() {
        let filter = process::compile_filter(Some("world".to_string()), false)
            .ok()
            .flatten();
        let spans = highlight_matches("hello world!", filter.as_ref());
        // "hello " + highlighted "world" + "!"
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "hello ");
        assert_eq!(spans[1].content, "world");
        assert_eq!(spans[2].content, "!");
        // Middle span must be styled (highlighted).
        assert_ne!(spans[1].style, ratatui::style::Style::default());
    }

    #[test]
    fn highlight_matches_substring_multiple_matches() {
        let filter = process::compile_filter(Some("o".to_string()), false)
            .ok()
            .flatten();
        let spans = highlight_matches("foo bar boo", filter.as_ref());
        // "f" + "o" + "o" + " bar b" + "o" + "o"  (matches at positions 1,2,9,10)
        let highlighted: Vec<&str> = spans
            .iter()
            .filter(|s| s.style != ratatui::style::Style::default())
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(highlighted.len(), 4);
        assert!(highlighted.iter().all(|&s| s == "o"));
    }

    #[test]
    fn highlight_matches_regex_match() {
        let filter = process::compile_filter(Some("\\d+".to_string()), true)
            .ok()
            .flatten();
        let spans = highlight_matches("proc123end", filter.as_ref());
        // "proc" + highlighted "123" + "end"
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].content, "123");
        assert_ne!(spans[1].style, ratatui::style::Style::default());
    }

    #[test]
    fn highlight_matches_regex_no_match_returns_plain_span() {
        let filter = process::compile_filter(Some("\\d+".to_string()), true)
            .ok()
            .flatten();
        let spans = highlight_matches("no digits here", filter.as_ref());
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "no digits here");
    }

    #[test]
    fn highlight_matches_substring_non_ascii_filter() {
        // "café" contains é (non-ASCII), so the unicode lowercase path is taken.
        let filter = process::compile_filter(Some("café".to_string()), false)
            .ok()
            .flatten();
        let spans = highlight_matches("order café here", filter.as_ref());
        let highlighted: Vec<&str> = spans
            .iter()
            .filter(|s| s.style != ratatui::style::Style::default())
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(highlighted, vec!["café"]);
    }

    #[test]
    fn highlight_matches_empty_text_returns_plain_empty_span() {
        let filter = process::compile_filter(Some("foo".to_string()), false)
            .ok()
            .flatten();
        let spans = highlight_matches("", filter.as_ref());
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "");
    }

    #[test]
    fn render_shows_filter_prompt_footer_when_filter_input_active() {
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).expect("terminal must initialize");
        let mut app = App::with_rows(None, vec![sample_row()]);
        app.filter_input = Some(FilterInput {
            text: "psn".to_string(),
            compiled: None,
        });

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

        assert!(text.contains("/ psn"));
    }
}
