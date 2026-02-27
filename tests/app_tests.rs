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

use nix::sys::signal::Signal;
use psn::{app::App, model::ProcRow};
use std::sync::Arc;
use sysinfo::ProcessStatus;

fn row(pid: i32) -> ProcRow {
    ProcRow {
        pid,
        ppid: None,
        ancestor_chain: Vec::new(),
        user: Arc::from("u"),
        status: ProcessStatus::Run,
        cpu_usage_tenths: 0,
        memory_bytes: 0,
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
fn refresh_preserves_selected_pid_when_sort_order_changes() {
    let mut app = App::with_rows(
        None,
        vec![
            ProcRow {
                pid: 1,
                ppid: None,
                ancestor_chain: Vec::new(),
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 100,
                memory_bytes: 100,
                name: "one".to_string(),
                cmd: "/bin/one".to_string(),
            },
            ProcRow {
                pid: 2,
                ppid: None,
                ancestor_chain: Vec::new(),
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 50,
                memory_bytes: 50,
                name: "two".to_string(),
                cmd: "/bin/two".to_string(),
            },
        ],
    );
    app.table_state.select(Some(1));

    app.refresh(vec![
        ProcRow {
            pid: 1,
            ppid: None,
            ancestor_chain: Vec::new(),
            user: Arc::from("u"),
            status: ProcessStatus::Run,
            cpu_usage_tenths: 0,
            memory_bytes: 0,
            name: "one".to_string(),
            cmd: "/bin/one".to_string(),
        },
        ProcRow {
            pid: 2,
            ppid: None,
            ancestor_chain: Vec::new(),
            user: Arc::from("u"),
            status: ProcessStatus::Run,
            cpu_usage_tenths: 200,
            memory_bytes: 200,
            name: "two".to_string(),
            cmd: "/bin/two".to_string(),
        },
    ]);

    assert_eq!(app.table_state.selected(), Some(0));
    app.begin_signal_confirmation(1);
    let pending = app
        .pending_confirmation
        .expect("pending confirmation should exist");
    assert_eq!(pending.pid, 2);
}

#[test]
fn refresh_does_not_preserve_selection_for_reused_pid_with_new_identity() {
    let mut app = App::with_rows(
        None,
        vec![
            ProcRow {
                pid: 1,
                ppid: None,
                ancestor_chain: Vec::new(),
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 100,
                memory_bytes: 100,
                name: "stable".to_string(),
                cmd: "/bin/stable".to_string(),
            },
            ProcRow {
                pid: 2,
                ppid: None,
                ancestor_chain: Vec::new(),
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 50,
                memory_bytes: 50,
                name: "old".to_string(),
                cmd: "/bin/old".to_string(),
            },
        ],
    );
    app.table_state.select(Some(1));

    app.refresh(vec![
        ProcRow {
            pid: 2,
            ppid: None,
            ancestor_chain: Vec::new(),
            user: Arc::from("u"),
            status: ProcessStatus::Run,
            cpu_usage_tenths: 200,
            memory_bytes: 200,
            name: "replacement".to_string(),
            cmd: "/bin/replacement".to_string(),
        },
        ProcRow {
            pid: 1,
            ppid: None,
            ancestor_chain: Vec::new(),
            user: Arc::from("u"),
            status: ProcessStatus::Run,
            cpu_usage_tenths: 0,
            memory_bytes: 0,
            name: "stable".to_string(),
            cmd: "/bin/stable".to_string(),
        },
    ]);

    assert_eq!(app.table_state.selected(), Some(1));
    app.begin_signal_confirmation(1);
    let pending = app
        .pending_confirmation
        .expect("pending confirmation should exist");
    assert_eq!(pending.pid, 1);
    assert_eq!(pending.process_name, "stable");
}

#[test]
fn refresh_clears_selection_when_no_rows() {
    let mut app = App::with_rows(None, vec![row(1)]);
    app.refresh(vec![]);
    assert_eq!(app.table_state.selected(), None);
}

#[test]
fn refresh_preserving_status_keeps_existing_status_text() {
    let mut app = App::with_rows(None, vec![row(1)]);
    app.status = "signal sent".to_string();
    app.refresh_preserving_status(vec![row(2)]);
    assert_eq!(app.status, "signal sent");
}

#[test]
fn refresh_falls_back_to_previous_index_when_selected_pid_disappears() {
    let mut app = App::with_rows(None, vec![row(1), row(2), row(3)]);
    app.table_state.select(Some(2));

    app.refresh(vec![row(4), row(5)]);

    assert_eq!(app.table_state.selected(), Some(1));
}

#[test]
fn refresh_drops_collapsed_state_for_reused_pid_with_new_identity() {
    let mut app = App::with_rows(
        None,
        vec![
            ProcRow {
                pid: 2,
                ppid: Some(1),
                ancestor_chain: vec![1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 0,
                memory_bytes: 0,
                name: "old-parent".to_string(),
                cmd: "/bin/old-parent".to_string(),
            },
            ProcRow {
                pid: 3,
                ppid: Some(2),
                ancestor_chain: vec![2, 1],
                user: Arc::from("u"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 0,
                memory_bytes: 0,
                name: "old-child".to_string(),
                cmd: "/bin/old-child".to_string(),
            },
        ],
    );
    assert!(app.collapse_selected());
    assert!(app.collapsed_pids.contains(&2));

    app.refresh(vec![
        ProcRow {
            pid: 2,
            ppid: Some(1),
            ancestor_chain: vec![1],
            user: Arc::from("u"),
            status: ProcessStatus::Run,
            cpu_usage_tenths: 0,
            memory_bytes: 0,
            name: "new-parent".to_string(),
            cmd: "/bin/new-parent".to_string(),
        },
        ProcRow {
            pid: 4,
            ppid: Some(2),
            ancestor_chain: vec![2, 1],
            user: Arc::from("u"),
            status: ProcessStatus::Run,
            cpu_usage_tenths: 0,
            memory_bytes: 0,
            name: "new-child".to_string(),
            cmd: "/bin/new-child".to_string(),
        },
    ]);

    assert!(app.collapsed_pids.is_empty());

    app.page_down(1);
    assert_eq!(app.table_state.selected(), Some(1));
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
fn begin_signal_confirmation_sets_pending_prompt() {
    let mut app = App::with_rows(None, vec![row(123)]);

    app.begin_signal_confirmation(1);

    let prompt = app.confirmation_prompt().expect("prompt should exist");
    assert!(prompt.contains("confirm sending SIGHUP (1)"));
    assert!(prompt.contains("process p123 (123)"));
}

#[test]
fn begin_signal_confirmation_ignores_invalid_digit() {
    let mut app = App::with_rows(None, vec![row(1)]);
    app.begin_signal_confirmation(0);
    assert!(app.pending_confirmation.is_none());
}

#[test]
fn cancel_signal_confirmation_clears_pending_state() {
    let mut app = App::with_rows(None, vec![row(1)]);
    app.begin_signal_confirmation(1);
    app.cancel_signal_confirmation();
    assert!(app.pending_confirmation.is_none());
}

#[test]
fn confirm_signal_updates_success_status_and_clears_pending() {
    let mut app = App::with_rows(None, vec![row(123)]);
    app.begin_signal_confirmation(9);
    let mut sender = |pid, signal| {
        assert_eq!(pid, 123);
        assert_eq!(signal, Signal::SIGKILL);
        Ok(())
    };

    app.confirm_signal(&mut sender);

    assert!(app.status.contains("sent"));
    assert!(app.pending_confirmation.is_none());
}

#[test]
fn confirm_signal_updates_failure_status_and_clears_pending() {
    let mut app = App::with_rows(None, vec![row(123)]);
    app.begin_signal_confirmation(1);
    let mut sender = |_, _| Err("denied".to_string());

    app.confirm_signal(&mut sender);

    assert!(app.status.contains("failed"));
    assert!(app.pending_confirmation.is_none());
}

#[test]
fn confirm_signal_without_pending_is_noop() {
    let mut app = App::with_rows(None, vec![row(123)]);
    app.status = "keep".to_string();
    let mut sender = |_, _| Err("should not run".to_string());
    app.confirm_signal(&mut sender);
    assert_eq!(app.status, "keep");
}

#[test]
fn pending_target_matches_current_rows_true_for_same_name_and_pid() {
    let mut app = App::with_rows(None, vec![row(100)]);
    app.begin_signal_confirmation(1);
    assert!(app.pending_target_matches_current_rows());
}

#[test]
fn pending_target_matches_current_rows_false_when_target_changed() {
    let mut app = App::with_rows(None, vec![row(100)]);
    app.begin_signal_confirmation(1);
    app.rows = vec![row(101)];
    assert!(!app.pending_target_matches_current_rows());
}

#[test]
fn pending_target_matches_current_rows_false_without_pending() {
    let app = App::with_rows(None, vec![row(100)]);
    assert!(!app.pending_target_matches_current_rows());
}

#[test]
fn abort_pending_target_changed_sets_status_and_clears_pending() {
    let mut app = App::with_rows(None, vec![row(100)]);
    app.begin_signal_confirmation(1);
    app.abort_pending_target_changed();
    assert!(app.status.contains("aborted: process"));
    assert!(app.pending_confirmation.is_none());
}

#[test]
fn abort_pending_target_changed_without_pending_is_noop() {
    let mut app = App::with_rows(None, vec![row(100)]);
    app.status = "keep".to_string();
    app.abort_pending_target_changed();
    assert_eq!(app.status, "keep");
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

#[test]
fn page_up_and_down_use_step_and_clamp_bounds() {
    let rows = (1..=30).map(row).collect::<Vec<_>>();
    let mut app = App::with_rows(None, rows);

    app.table_state.select(Some(15));
    app.page_up(10);
    assert_eq!(app.table_state.selected(), Some(5));

    app.page_up(10);
    assert_eq!(app.table_state.selected(), Some(0));

    app.page_down(10);
    assert_eq!(app.table_state.selected(), Some(10));

    app.page_down(50);
    assert_eq!(app.table_state.selected(), Some(29));
}

#[test]
fn page_down_selects_row_when_selection_missing() {
    let rows = (1..=5).map(row).collect::<Vec<_>>();
    let mut app = App::with_rows(None, rows);
    app.table_state.select(None);

    app.page_down(3);
    assert_eq!(app.table_state.selected(), Some(2));
}

#[test]
fn page_navigation_noops_for_zero_step() {
    let rows = (1..=5).map(row).collect::<Vec<_>>();
    let mut app = App::with_rows(None, rows);
    app.table_state.select(Some(2));

    app.page_up(0);
    app.page_down(0);

    assert_eq!(app.table_state.selected(), Some(2));
}

#[test]
fn page_down_handles_huge_step_without_overflow() {
    let rows = (1..=5).map(row).collect::<Vec<_>>();
    let mut app = App::with_rows(None, rows);
    app.table_state.select(Some(1));

    app.page_down(usize::MAX);

    assert_eq!(app.table_state.selected(), Some(4));
}

#[test]
fn page_down_clears_invalid_selection_when_rows_are_empty() {
    let mut app = App::with_rows(None, vec![]);
    app.table_state.select(Some(3));

    app.page_down(1);

    assert_eq!(app.table_state.selected(), None);
}

#[test]
fn collapse_selected_hides_descendants_and_expand_selected_restores_them() {
    let mut app = App::with_rows(
        None,
        vec![
            ProcRow {
                pid: 2,
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

    assert!(app.collapse_selected());
    assert!(app.collapsed_pids.contains(&2));

    app.page_down(1);
    assert_eq!(app.table_state.selected(), Some(0));

    assert!(app.expand_selected());
    assert!(!app.collapsed_pids.contains(&2));

    app.page_down(1);
    assert_eq!(app.table_state.selected(), Some(1));
}

#[test]
fn collapse_selected_noops_for_leaf_row() {
    let mut app = App::with_rows(None, vec![row(1)]);

    assert!(!app.collapse_selected());
    assert!(app.collapsed_pids.is_empty());
}

#[test]
fn begin_signal_confirmation_uses_visible_tree_selection_index() {
    let init = ProcRow {
        pid: 1,
        ppid: None,
        ancestor_chain: Vec::new(),
        user: Arc::from("u"),
        status: ProcessStatus::Sleep,
        cpu_usage_tenths: 0,
        memory_bytes: 0,
        name: "init".to_string(),
        cmd: "/bin/init".to_string(),
    };
    let service = ProcRow {
        pid: 2,
        ppid: Some(1),
        ancestor_chain: vec![1],
        user: Arc::from("u"),
        status: ProcessStatus::Run,
        cpu_usage_tenths: 0,
        memory_bytes: 0,
        name: "service".to_string(),
        cmd: "/bin/service".to_string(),
    };

    // Backing order differs from visual tree order because pid 1 children are
    // rendered as roots and roots are sorted by status first.
    let mut app = App::with_rows(None, vec![init, service]);
    app.table_state.select(Some(0));

    app.begin_signal_confirmation(1);

    let pending = app
        .pending_confirmation
        .expect("pending confirmation should exist");
    assert_eq!(pending.pid, 2);
    assert_eq!(pending.process_name, "service");
}
