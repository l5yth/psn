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

//! Interactive `/` filter mode.
//!
//! Opening, typing, backspacing, confirming and cancelling the prompt.

use super::{noop_await, row};
use crate::app::{self, App};
use crate::runtime::{Action, ActionResult, apply_action, map_key_event_to_action};
use crossterm::event::KeyCode;
use nix::sys::signal::Signal;

#[test]
fn map_key_event_to_action_maps_filter_mode_actions() {
    assert_eq!(
        map_key_event_to_action(KeyCode::Char('/'), false, false),
        Action::BeginInteractiveFilter
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::Char('a'), false, true),
        Action::FilterInputChar('a')
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::Backspace, false, true),
        Action::FilterInputBackspace
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::Enter, false, true),
        Action::FilterConfirm
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::Esc, false, true),
        Action::FilterCancel
    );
    // Normal keys are noop in filter mode.
    assert_eq!(
        map_key_event_to_action(KeyCode::Char('q'), false, true),
        Action::FilterInputChar('q')
    );
}

#[test]
fn apply_action_begin_interactive_filter_opens_prompt() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo")];
    let mut sender = |_: i32, _: Signal| Ok(());

    assert_eq!(
        apply_action(
            &mut app,
            Action::BeginInteractiveFilter,
            &mut refresh,
            &mut sender,
            &mut (noop_await as fn(i32)),
        ),
        ActionResult {
            should_quit: false,
            needs_redraw: true
        }
    );
    assert!(app.filter_input.is_some());
    assert_eq!(app.filter_input.as_ref().unwrap().text, "");
}

#[test]
fn apply_action_filter_input_char_appends_and_refilters() {
    let mut app = App::with_rows(None, vec![row(11, "foo"), row(22, "bar")]);
    app.filter_input = Some(app::FilterInput {
        text: String::new(),
        compiled: None,
    });
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo"), row(22, "bar")];
    let mut sender = |_: i32, _: Signal| Ok(());

    apply_action(
        &mut app,
        Action::FilterInputChar('f'),
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    );
    let fi = app.filter_input.as_ref().unwrap();
    assert_eq!(fi.text, "f");
    assert!(fi.compiled.is_some());
}

#[test]
fn apply_action_filter_input_backspace_removes_char() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    app.filter_input = Some(app::FilterInput {
        text: "fo".to_string(),
        compiled: None,
    });
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo")];
    let mut sender = |_: i32, _: Signal| Ok(());

    apply_action(
        &mut app,
        Action::FilterInputBackspace,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    );
    assert_eq!(app.filter_input.as_ref().unwrap().text, "f");
}

#[test]
fn apply_action_filter_confirm_commits_filter() {
    let mut app = App::with_rows(None, vec![row(11, "foo"), row(22, "bar")]);
    app.filter_input = Some(app::FilterInput {
        text: "foo".to_string(),
        compiled: None,
    });
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo")];
    let mut sender = |_: i32, _: Signal| Ok(());

    apply_action(
        &mut app,
        Action::FilterConfirm,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    );
    assert!(app.filter_input.is_none());
    assert_eq!(app.filter.as_deref(), Some("foo"));
    assert!(app.compiled_filter.is_some());
}

#[test]
fn apply_action_filter_cancel_restores_state() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    app.filter_input = Some(app::FilterInput {
        text: "bar".to_string(),
        compiled: None,
    });
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo")];
    let mut sender = |_: i32, _: Signal| Ok(());

    let result = apply_action(
        &mut app,
        Action::FilterCancel,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    );
    assert!(app.filter_input.is_none());
    assert!(result.needs_redraw);
}

#[test]
fn map_key_event_to_action_filter_mode_allows_navigation_keys() {
    assert_eq!(
        map_key_event_to_action(KeyCode::Up, false, true),
        Action::MoveUp
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::Down, false, true),
        Action::MoveDown
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::PageUp, false, true),
        Action::PageUp
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::PageDown, false, true),
        Action::PageDown
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::Left, false, true),
        Action::CollapseTree
    );
    assert_eq!(
        map_key_event_to_action(KeyCode::Right, false, true),
        Action::ExpandTree
    );
}

#[test]
fn map_key_event_to_action_filter_mode_noop_for_unknown_key() {
    assert_eq!(
        map_key_event_to_action(KeyCode::F(1), false, true),
        Action::Noop
    );
}

#[test]
fn map_key_event_to_action_noop_for_unknown_key_in_normal_mode() {
    assert_eq!(
        map_key_event_to_action(KeyCode::F(1), false, false),
        Action::Noop
    );
}

#[test]
fn apply_action_begin_interactive_filter_prefills_existing_substring() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    app.compiled_filter = crate::process::compile_filter(Some("foo".to_string()), false)
        .ok()
        .flatten();
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo")];
    let mut sender = |_: i32, _: Signal| Ok(());

    apply_action(
        &mut app,
        Action::BeginInteractiveFilter,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    );
    let fi = app.filter_input.as_ref().unwrap();
    assert_eq!(fi.text, "foo");
    assert!(fi.compiled.is_some());
}

#[test]
fn apply_action_filter_input_char_noop_when_not_in_filter_mode() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(22, "bar")];
    let mut sender = |_: i32, _: Signal| Ok(());

    let result = apply_action(
        &mut app,
        Action::FilterInputChar('x'),
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    );
    assert!(!result.needs_redraw);
    // Rows must not change since filter mode is not active.
    assert_eq!(app.rows[0].pid, 11);
}

#[test]
fn apply_action_filter_input_backspace_noop_when_not_in_filter_mode() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(22, "bar")];
    let mut sender = |_: i32, _: Signal| Ok(());

    let result = apply_action(
        &mut app,
        Action::FilterInputBackspace,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    );
    assert!(!result.needs_redraw);
    assert_eq!(app.rows[0].pid, 11);
}

#[test]
fn apply_action_filter_input_backspace_clears_compiled_when_text_becomes_empty() {
    let mut app = App::with_rows(None, vec![row(11, "foo"), row(22, "bar")]);
    app.filter_input = Some(app::FilterInput {
        text: "f".to_string(),
        compiled: crate::process::compile_filter(Some("f".to_string()), false)
            .ok()
            .flatten(),
    });
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo"), row(22, "bar")];
    let mut sender = |_: i32, _: Signal| Ok(());

    apply_action(
        &mut app,
        Action::FilterInputBackspace,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    );
    let fi = app.filter_input.as_ref().unwrap();
    assert_eq!(fi.text, "");
    assert!(fi.compiled.is_none());
}

#[test]
fn apply_action_filter_confirm_noop_when_not_in_filter_mode() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo")];
    let mut sender = |_: i32, _: Signal| Ok(());

    apply_action(
        &mut app,
        Action::FilterConfirm,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    );
    // filter_input was None, compiled_filter stays None, rows refresh with None filter.
    assert!(app.filter_input.is_none());
    assert!(app.compiled_filter.is_none());
}

#[test]
fn apply_action_filter_confirm_with_empty_text_clears_filter() {
    let mut app = App::with_rows(None, vec![row(11, "foo"), row(22, "bar")]);
    app.filter_input = Some(app::FilterInput {
        text: String::new(),
        compiled: None,
    });
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo"), row(22, "bar")];
    let mut sender = |_: i32, _: Signal| Ok(());

    apply_action(
        &mut app,
        Action::FilterConfirm,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    );
    assert!(app.filter_input.is_none());
    assert!(app.filter.is_none());
    assert!(app.compiled_filter.is_none());
}

#[test]
fn apply_action_filter_cancel_noop_when_not_active() {
    let mut app = App::with_rows(None, vec![row(11, "foo")]);
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(22, "bar")];
    let mut sender = |_: i32, _: Signal| Ok(());

    let result = apply_action(
        &mut app,
        Action::FilterCancel,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    );
    assert!(!result.needs_redraw);
    // Rows must not change since there was nothing to cancel.
    assert_eq!(app.rows[0].pid, 11);
}

#[test]
fn apply_action_begin_interactive_filter_applies_prefill_immediately() {
    // Start with two rows; refresh returns only foo when given a substring filter.
    let mut app = App::with_rows(None, vec![row(11, "foo"), row(22, "bar")]);
    app.compiled_filter = crate::process::compile_filter(Some("foo".to_string()), false)
        .ok()
        .flatten();
    // Simulate the live process list: only foo matches the pre-filled filter.
    let mut refresh = |f: Option<&crate::process::FilterSpec>| {
        if f.is_some() {
            vec![row(11, "foo")]
        } else {
            vec![row(11, "foo"), row(22, "bar")]
        }
    };
    let mut sender = |_: i32, _: Signal| Ok(());

    apply_action(
        &mut app,
        Action::BeginInteractiveFilter,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    );
    // The row list must already reflect the pre-filled filter.
    assert_eq!(app.rows.len(), 1);
    assert_eq!(app.rows[0].pid, 11);
}

#[test]
fn apply_action_filter_cancel_resets_selection_to_first() {
    let mut app = App::with_rows(None, vec![row(11, "foo"), row(22, "bar")]);
    app.filter_input = Some(app::FilterInput {
        text: "x".to_string(),
        compiled: None,
    });
    app.table_state.select(Some(1)); // selection somewhere other than first
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo"), row(22, "bar")];
    let mut sender = |_: i32, _: Signal| Ok(());

    apply_action(
        &mut app,
        Action::FilterCancel,
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    );
    assert_eq!(app.table_state.selected(), Some(0));
}

#[test]
fn apply_action_filter_input_char_resets_selection_to_first() {
    let mut app = App::with_rows(None, vec![row(11, "foo"), row(22, "bar")]);
    app.filter_input = Some(app::FilterInput {
        text: String::new(),
        compiled: None,
    });
    app.table_state.select(Some(1)); // pre-select last row
    let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo"), row(22, "bar")];
    let mut sender = |_: i32, _: Signal| Ok(());

    apply_action(
        &mut app,
        Action::FilterInputChar('f'),
        &mut refresh,
        &mut sender,
        &mut (noop_await as fn(i32)),
    );
    assert_eq!(app.table_state.selected(), Some(0));
}
