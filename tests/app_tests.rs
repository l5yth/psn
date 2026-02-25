// Copyright (c) 2026 l5yth
// SPDX-License-Identifier: Apache-2.0

use nix::sys::signal::Signal;
use psn::{app::App, model::ProcRow};
use sysinfo::ProcessStatus;

fn row(pid: i32) -> ProcRow {
    ProcRow {
        pid,
        user: "u".to_string(),
        status: ProcessStatus::Run,
        name: format!("p{pid}"),
        cmd: format!("/bin/p{pid}"),
    }
}

fn dummy_sender(_: i32, _: Signal) -> Result<(), String> {
    Ok(())
}

#[test]
fn with_rows_selects_first_row_when_non_empty() {
    let app = App::with_rows(None, vec![row(1), row(2)]);
    assert_eq!(app.table_state.selected(), Some(0));
}

#[test]
fn with_rows_selects_none_when_empty() {
    let app = App::with_rows(None, vec![]);
    assert_eq!(app.table_state.selected(), None);
}

#[test]
fn filter_returns_borrowed_filter_text() {
    let app = App::with_rows(Some("abc".to_string()), vec![]);
    assert_eq!(app.filter(), Some("abc"));
}

#[test]
fn refresh_reloads_rows_and_clamps_selection() {
    let mut app = App::with_rows(Some("abc".to_string()), vec![row(1), row(2), row(3)]);
    app.table_state.select(Some(2));
    app.status = "x".to_string();

    app.refresh(vec![row(10)]);

    assert_eq!(app.rows.len(), 1);
    assert_eq!(app.table_state.selected(), Some(0));
    assert!(app.status.is_empty());
}

#[test]
fn refresh_clears_selection_when_no_rows() {
    let mut app = App::with_rows(None, vec![row(1)]);
    app.refresh(vec![]);
    assert_eq!(app.table_state.selected(), None);
}

#[test]
fn move_up_and_down_respect_bounds() {
    let mut app = App::with_rows(None, vec![row(1), row(2)]);
    app.move_up();
    assert_eq!(app.table_state.selected(), Some(0));

    app.move_down();
    assert_eq!(app.table_state.selected(), Some(1));

    app.move_down();
    assert_eq!(app.table_state.selected(), Some(1));

    app.move_up();
    assert_eq!(app.table_state.selected(), Some(0));
}

#[test]
fn move_down_selects_first_when_selection_missing() {
    let mut app = App::with_rows(None, vec![row(1)]);
    app.table_state.select(None);
    app.move_down();
    assert_eq!(app.table_state.selected(), Some(0));
}

#[test]
fn move_down_keeps_none_selection_for_empty_rows() {
    let mut app = App::with_rows(None, vec![]);
    app.table_state.select(None);
    app.move_down();
    assert_eq!(app.table_state.selected(), None);
}

#[test]
fn send_digit_updates_success_status() {
    let mut app = App::with_rows(None, vec![row(123)]);
    let mut sender = |pid, _| {
        assert_eq!(pid, 123);
        Ok(())
    };
    app.send_digit(9, &mut sender);

    assert!(app.status.contains("sent"));
    assert!(app.status.contains("123"));
}

#[test]
fn send_digit_updates_failure_status() {
    let mut app = App::with_rows(None, vec![row(456)]);
    let mut sender = |_, _| Err("denied".to_string());
    app.send_digit(1, &mut sender);

    assert!(app.status.contains("failed"));
    assert!(app.status.contains("denied"));
}

#[test]
fn send_digit_ignores_invalid_digit() {
    let mut app = App::with_rows(None, vec![row(1)]);
    app.status = "keep".to_string();
    let mut sender = dummy_sender;
    app.send_digit(0, &mut sender);
    assert_eq!(app.status, "keep");
}

#[test]
fn send_digit_ignores_when_no_selection() {
    let mut app = App::with_rows(None, vec![row(1)]);
    app.table_state.select(None);
    app.status = "keep".to_string();
    let mut sender = dummy_sender;
    app.send_digit(1, &mut sender);
    assert_eq!(app.status, "keep");
}

#[test]
fn send_digit_ignores_missing_row_for_selected_index() {
    let mut app = App::with_rows(None, vec![]);
    app.table_state.select(Some(2));
    app.status = "keep".to_string();
    let mut sender = dummy_sender;
    app.send_digit(1, &mut sender);
    assert_eq!(app.status, "keep");
}

#[test]
fn dummy_sender_returns_ok() {
    assert!(dummy_sender(1, Signal::SIGCONT).is_ok());
}
