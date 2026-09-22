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

//! Key mapping, signal actions, navigation, and the event loop.
//!
//! Everything except interactive filter mode, which lives in `filter.rs`.

use super::{must_not_run, noop_await, row};
use crate::runtime::{
    Action, ActionResult, apply_action, map_key_event_to_action, run_event_loop, run_with_runtime,
};
use crate::{app::App, model::ProcRow};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use nix::sys::signal::Signal;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::ProcessStatus;

#[test]
fn map_key_event_to_action_maps_regular_actions() {
    assert_eq!(
        map_key_event_to_action(KeyCode::Char('q'), false, false),
        Action::Quit
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::Char('r'), false, false),
        Action::Refresh
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::Up, false, false),
        Action::MoveUp
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::Down, false, false),
        Action::MoveDown
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::PageUp, false, false),
        Action::PageUp
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::PageDown, false, false),
        Action::PageDown
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::Left, false, false),
        Action::CollapseTree
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::Right, false, false),
        Action::ExpandTree
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::Char('1'), false, false),
        Action::BeginSignalConfirmation(1)
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::Char('0'), false, false),
        Action::Noop
    );
}

#[test]
fn map_key_event_to_action_maps_pending_confirmation_actions() {
    assert_eq!(
        map_key_event_to_action(KeyCode::Enter, true, false),
        Action::ConfirmPendingSignal
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::Char('Y'), true, false),
        Action::ConfirmPendingSignal
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::Esc, true, false),
        Action::CancelPendingSignal
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::Up, true, false),
        Action::Noop
    );
}

#[test]
fn apply_action_confirm_pending_signal_refreshes_and_sends() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    app.begin_signal_confirmation(1);
    let mut refresh_calls = 0;
    let mut refresh = |_: Option<&crate::process::FilterSpec>| {
        refresh_calls += 1;
        vec![row(11, "foo")]
    };
    let mut sent = false;
    let mut sender = |pid: i32, signal: Signal| {
        sent = true;
        assert_eq!(pid, 11);
        assert_eq!(signal, Signal::SIGHUP);
        Ok(())
    };

    assert_eq!(
        apply_action(
            &mut app,
            Action::ConfirmPendingSignal,
            &mut refresh,
            &mut sender,
            &mut (noop_await as fn(i32)),
        ),
        ActionResult {
            should_quit: false,
            needs_redraw: true
        }
    );
    assert!(sent);
    assert_eq!(refresh_calls, 2);
    assert!(app.pending_confirmation.is_none());
}

#[test]
fn apply_action_confirm_pending_signal_invokes_await_pid_gone_with_signaled_pid() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    app.begin_signal_confirmation(1);
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo")];
    let mut sender = |_: i32, _: Signal| Ok(());
    let mut awaited: Option<i32> = None;
    let mut await_pid_gone = |pid: i32| {
        awaited = Some(pid);
    };

    apply_action(
        &mut app,
        Action::ConfirmPendingSignal,
        &mut refresh,
        &mut sender,
        &mut await_pid_gone,
    );

    assert_eq!(awaited, Some(11));
}

#[test]
fn apply_action_confirm_pending_signal_skips_await_when_sender_fails() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    app.begin_signal_confirmation(1);
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo")];
    let mut sender = |_: i32, _: Signal| Err("denied".to_string());

    // `must_not_run` panics if invoked; reaching the assert means the await
    // hook was correctly skipped on the sender-failure path.
    apply_action(
        &mut app,
        Action::ConfirmPendingSignal,
        &mut refresh,
        &mut sender,
        &mut (must_not_run as fn(i32)),
    );

    assert!(app.status.contains("failed"));
}

#[test]
fn apply_action_cancel_pending_signal_clears_confirmation() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    app.begin_signal_confirmation(1);
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo")];
    let mut sender = |_: i32, _: Signal| Ok(());

    assert_eq!(
        apply_action(
            &mut app,
            Action::CancelPendingSignal,
            &mut refresh,
            &mut sender,
            &mut (noop_await as fn(i32)),
        ),
        ActionResult {
            should_quit: false,
            needs_redraw: true
        }
    );
    assert!(app.pending_confirmation.is_none());
}

#[test]
fn apply_action_quit_returns_true() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo")];
    let mut sender = |_: i32, _: Signal| Ok(());
    assert_eq!(
        apply_action(
            &mut app,
            Action::Quit,
            &mut refresh,
            &mut sender,
            &mut (noop_await as fn(i32))
        ),
        ActionResult {
            should_quit: true,
            needs_redraw: false
        }
    );
}

#[test]
fn apply_action_refresh_reloads_rows() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(22, "bar")];
    let mut sender = |_: i32, _: Signal| Ok(());
    assert_eq!(
        apply_action(
            &mut app,
            Action::Refresh,
            &mut refresh,
            &mut sender,
            &mut (noop_await as fn(i32))
        ),
        ActionResult {
            should_quit: false,
            needs_redraw: true
        }
    );
    assert_eq!(app.rows[0].pid, 22);
}

#[test]
fn apply_action_move_actions_change_selection() {
    let mut app = App::with_rows(None, vec![row(11, "foo"), row(22, "bar"), row(33, "baz")]);
    let rows = vec![row(11, "foo"), row(22, "bar"), row(33, "baz")];
    let mut refresh = |_: Option<&crate::process::FilterSpec>| rows.clone();
    let mut sender = |_: i32, _: Signal| Ok(());
    // Exercise the refresh closure so its body is covered.
    apply_action(
        &mut app,
        Action::Refresh,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    );

    assert_eq!(
        apply_action(
            &mut app,
            Action::MoveDown,
            &mut refresh,
            &mut sender,
            &mut (noop_await as fn(i32))
        ),
        ActionResult {
            should_quit: false,
            needs_redraw: true
        }
    );
    assert_eq!(app.table_state.selected(), Some(1));

    assert_eq!(
        apply_action(
            &mut app,
            Action::MoveUp,
            &mut refresh,
            &mut sender,
            &mut (noop_await as fn(i32))
        ),
        ActionResult {
            should_quit: false,
            needs_redraw: true
        }
    );
    assert_eq!(app.table_state.selected(), Some(0));
}

#[test]
fn apply_action_page_actions_change_selection() {
    let rows: Vec<ProcRow> = (0..25).map(|i| row(i + 1, "p")).collect();
    let mut app = App::with_rows(None, rows.clone());
    let mut refresh = |_: Option<&crate::process::FilterSpec>| rows.clone();
    let mut sender = |_: i32, _: Signal| Ok(());

    assert_eq!(
        apply_action(
            &mut app,
            Action::PageDown,
            &mut refresh,
            &mut sender,
            &mut (noop_await as fn(i32))
        ),
        ActionResult {
            should_quit: false,
            needs_redraw: true
        }
    );
    assert_eq!(app.table_state.selected(), Some(10));

    assert_eq!(
        apply_action(
            &mut app,
            Action::PageUp,
            &mut refresh,
            &mut sender,
            &mut (noop_await as fn(i32))
        ),
        ActionResult {
            should_quit: false,
            needs_redraw: true
        }
    );
    assert_eq!(app.table_state.selected(), Some(0));
}

#[test]
fn apply_action_tree_actions_toggle_collapsed_state() {
    let rows = vec![
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
    ];
    let mut app = App::with_rows(None, rows.clone());
    let mut refresh = |_: Option<&crate::process::FilterSpec>| rows.clone();
    let mut sender = |_: i32, _: Signal| Ok(());

    assert_eq!(
        apply_action(
            &mut app,
            Action::CollapseTree,
            &mut refresh,
            &mut sender,
            &mut (noop_await as fn(i32))
        ),
        ActionResult {
            should_quit: false,
            needs_redraw: true
        }
    );
    assert!(app.collapsed_pids.contains(&2));

    assert_eq!(
        apply_action(
            &mut app,
            Action::ExpandTree,
            &mut refresh,
            &mut sender,
            &mut (noop_await as fn(i32))
        ),
        ActionResult {
            should_quit: false,
            needs_redraw: true
        }
    );
    assert!(!app.collapsed_pids.contains(&2));
}

#[test]
fn apply_action_begin_signal_confirmation_sets_pending() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo")];
    let mut sender = |_: i32, _: Signal| Ok(());

    assert_eq!(
        apply_action(
            &mut app,
            Action::BeginSignalConfirmation(1),
            &mut refresh,
            &mut sender,
            &mut (noop_await as fn(i32)),
        ),
        ActionResult {
            should_quit: false,
            needs_redraw: true
        }
    );
    assert!(app.pending_confirmation.is_some());
}

#[test]
fn apply_action_confirm_pending_signal_aborts_on_target_change() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    app.begin_signal_confirmation(1);
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(22, "bar")];
    let mut sender = |_: i32, _: Signal| Ok(());

    assert_eq!(
        apply_action(
            &mut app,
            Action::ConfirmPendingSignal,
            &mut refresh,
            &mut sender,
            &mut (noop_await as fn(i32)),
        ),
        ActionResult {
            should_quit: false,
            needs_redraw: true
        }
    );
    assert!(app.status.contains("aborted"));
}

#[test]
fn apply_action_noop_is_noop() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo")];
    let mut sender = |_: i32, _: Signal| Ok(());
    let selected = app.table_state.selected();
    assert_eq!(
        apply_action(
            &mut app,
            Action::Noop,
            &mut refresh,
            &mut sender,
            &mut (noop_await as fn(i32))
        ),
        ActionResult {
            should_quit: false,
            needs_redraw: false
        }
    );
    assert_eq!(app.table_state.selected(), selected);
}

#[test]
fn run_event_loop_redraws_on_resize_and_exits_on_q() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    let mut draw_calls = 0;
    let mut draw = |_: &mut App| -> anyhow::Result<()> {
        draw_calls += 1;
        Ok(())
    };

    let mut events = vec![
        Event::Resize(100, 20),
        Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
    ]
    .into_iter();
    let mut next_event =
        |_timeout: Duration| -> anyhow::Result<Option<Event>> { Ok(events.next()) };
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo")];
    let mut sender = |_: i32, _: Signal| Ok(());

    run_event_loop(
        &mut app,
        &mut draw,
        &mut next_event,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    )
    .expect("loop should terminate cleanly");

    assert!(draw_calls >= 2);
}

#[test]
fn run_event_loop_updates_redraw_state_for_non_quit_key_action() {
    let mut app = App::with_rows(None, vec![row(11, "foo"), row(12, "bar")]);
    let mut draw_calls = 0;
    let mut draw = |_: &mut App| -> anyhow::Result<()> {
        draw_calls += 1;
        Ok(())
    };

    let rows = vec![row(11, "foo"), row(12, "bar")];
    let mut events = vec![
        // 'r' ensures the refresh closure body is executed.
        Event::Key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)),
        Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
    ]
    .into_iter();
    let mut next_event =
        |_timeout: Duration| -> anyhow::Result<Option<Event>> { Ok(events.next()) };
    let mut refresh = |_: Option<&crate::process::FilterSpec>| rows.clone();
    let mut sender = |_: i32, _: Signal| Ok(());

    run_event_loop(
        &mut app,
        &mut draw,
        &mut next_event,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    )
    .expect("loop should terminate cleanly");

    assert_eq!(app.table_state.selected(), Some(1));
    assert!(draw_calls >= 2);
}

#[test]
fn run_event_loop_ignores_non_press_key_events() {
    let mut app = App::with_rows(None, vec![row(11, "foo"), row(12, "bar")]);
    let mut draw = |_: &mut App| -> anyhow::Result<()> { Ok(()) };
    let rows = vec![row(11, "foo"), row(12, "bar")];
    let release = KeyEvent::new_with_kind(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Release);
    let mut events = vec![
        // 'r' ensures the refresh closure body is executed.
        Event::Key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)),
        Event::Key(release),
        Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
    ]
    .into_iter();
    let mut next_event =
        |_timeout: Duration| -> anyhow::Result<Option<Event>> { Ok(events.next()) };
    let mut refresh = |_: Option<&crate::process::FilterSpec>| rows.clone();
    let mut sender = |_: i32, _: Signal| Ok(());

    run_event_loop(
        &mut app,
        &mut draw,
        &mut next_event,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    )
    .expect("loop should terminate cleanly");

    assert_eq!(app.table_state.selected(), Some(0));
}

#[test]
fn run_event_loop_ignores_non_key_events() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    let mut draw = |_: &mut App| -> anyhow::Result<()> { Ok(()) };
    let mut events = vec![
        Event::FocusGained,
        Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
    ]
    .into_iter();
    let mut next_event =
        |_timeout: Duration| -> anyhow::Result<Option<Event>> { Ok(events.next()) };
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo")];
    let mut sender = |_: i32, _: Signal| Ok(());

    run_event_loop(
        &mut app,
        &mut draw,
        &mut next_event,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    )
    .expect("loop should terminate cleanly");
}

#[test]
fn run_event_loop_propagates_draw_errors() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    let mut draw = |_: &mut App| -> anyhow::Result<()> { Err(anyhow::anyhow!("draw failed")) };
    let mut next_event = |_timeout: Duration| -> anyhow::Result<Option<Event>> { Ok(None) };
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo")];
    let mut sender = |_: i32, _: Signal| Ok(());

    let result = run_event_loop(
        &mut app,
        &mut draw,
        &mut next_event,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    );
    assert!(result.is_err());
}

#[test]
fn run_event_loop_propagates_event_errors() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    let mut draw = |_: &mut App| -> anyhow::Result<()> { Ok(()) };
    let mut next_event = |_timeout: Duration| -> anyhow::Result<Option<Event>> {
        Err(anyhow::anyhow!("event failed"))
    };
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo")];
    let mut sender = |_: i32, _: Signal| Ok(());

    let result = run_event_loop(
        &mut app,
        &mut draw,
        &mut next_event,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    );
    assert!(result.is_err());
}

#[test]
fn run_with_runtime_initializes_rows_and_runs_loop() {
    let mut draw_calls = 0;
    let mut draw = |_: &mut App| -> anyhow::Result<()> {
        draw_calls += 1;
        Ok(())
    };
    let mut events = vec![Event::Key(KeyEvent::new(
        KeyCode::Char('q'),
        KeyModifiers::NONE,
    ))]
    .into_iter();
    let mut next_event =
        |_timeout: Duration| -> anyhow::Result<Option<Event>> { Ok(events.next()) };
    let mut refresh_calls = 0;
    let mut refresh = |_: Option<&crate::process::FilterSpec>| {
        refresh_calls += 1;
        vec![row(11, "foo")]
    };
    let mut sender = |_: i32, _: Signal| Ok(());

    run_with_runtime(
        Some("foo".to_string()),
        None,
        &mut draw,
        &mut next_event,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    )
    .expect("runtime should terminate cleanly");

    assert_eq!(refresh_calls, 1);
    assert!(draw_calls >= 1);
}
