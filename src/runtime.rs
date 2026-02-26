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

//! Input mapping and runtime action application for the TUI loop.

use std::cmp::min;

use anyhow::Result;
use crossterm::event::KeyCode;
use nix::sys::signal::Signal;

use crate::{app::App, model::ProcRow};

/// Number of rows moved by page navigation actions.
pub const PAGE_STEP: usize = 10;

/// Mapped high-level actions produced from key input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Exit the main event loop.
    Quit,
    /// Refresh process rows from the system.
    Refresh,
    /// Move selection one row up.
    MoveUp,
    /// Move selection one row down.
    MoveDown,
    /// Move selection one page up.
    PageUp,
    /// Move selection one page down.
    PageDown,
    /// Open signal confirmation for a digit-mapped signal.
    BeginSignalConfirmation(u8),
    /// Confirm and dispatch the pending signal action.
    ConfirmPendingSignal,
    /// Cancel the pending signal action.
    CancelPendingSignal,
    /// Intentionally perform no state change.
    Noop,
}

/// Map a key press to a runtime action.
pub fn map_key_event_to_action(key_code: KeyCode, pending_confirmation: bool) -> Action {
    if pending_confirmation {
        return match key_code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                Action::ConfirmPendingSignal
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Action::CancelPendingSignal,
            _ => Action::Noop,
        };
    }

    match key_code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('r') => Action::Refresh,
        KeyCode::Up => Action::MoveUp,
        KeyCode::Down => Action::MoveDown,
        KeyCode::PageUp => Action::PageUp,
        KeyCode::PageDown => Action::PageDown,
        KeyCode::Char(c) if c.is_ascii_digit() => {
            let digit = c.to_digit(10).unwrap_or_default() as u8;
            if (1..=9).contains(&digit) {
                Action::BeginSignalConfirmation(digit)
            } else {
                Action::Noop
            }
        }
        _ => Action::Noop,
    }
}

/// Apply a mapped action and return whether the event loop should exit.
pub fn apply_action(
    app: &mut App,
    action: Action,
    refresh_rows: &mut dyn FnMut() -> Vec<ProcRow>,
    sender: &mut dyn FnMut(i32, Signal) -> Result<(), String>,
) -> bool {
    match action {
        Action::Quit => true,
        Action::Refresh => {
            app.refresh(refresh_rows());
            false
        }
        Action::MoveUp => {
            app.move_up();
            false
        }
        Action::MoveDown => {
            app.move_down();
            false
        }
        Action::PageUp => {
            app.page_up(PAGE_STEP);
            false
        }
        Action::PageDown => {
            app.page_down(PAGE_STEP);
            false
        }
        Action::BeginSignalConfirmation(digit) => {
            app.begin_signal_confirmation(digit);
            false
        }
        Action::ConfirmPendingSignal => {
            refresh_with_selection_preserved(app, refresh_rows);
            if !app.pending_target_matches_current_rows() {
                app.abort_pending_target_changed();
                return false;
            }

            app.confirm_signal(sender);
            refresh_with_selection_preserved(app, refresh_rows);
            false
        }
        Action::CancelPendingSignal => {
            app.cancel_signal_confirmation();
            false
        }
        Action::Noop => false,
    }
}

/// Refresh rows while keeping selection bounded to the previous index.
fn refresh_with_selection_preserved(app: &mut App, refresh_rows: &mut dyn FnMut() -> Vec<ProcRow>) {
    let selected_before_refresh = app.table_state.selected().unwrap_or(0);
    app.refresh_preserving_status(refresh_rows());
    if !app.rows.is_empty() {
        app.table_state
            .select(Some(min(selected_before_refresh, app.rows.len() - 1)));
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, apply_action, map_key_event_to_action};
    use crate::{app::App, model::ProcRow};
    use crossterm::event::KeyCode;
    use nix::sys::signal::Signal;
    use sysinfo::ProcessStatus;

    fn row(pid: i32, name: &str) -> ProcRow {
        ProcRow {
            pid,
            user: "u".to_string(),
            status: ProcessStatus::Run,
            name: name.to_string(),
            cmd: format!("/bin/{name}"),
        }
    }

    #[test]
    fn map_key_event_to_action_maps_regular_actions() {
        assert_eq!(
            map_key_event_to_action(KeyCode::Char('q'), false),
            Action::Quit
        );
        assert_eq!(
            map_key_event_to_action(KeyCode::Char('r'), false),
            Action::Refresh
        );
        assert_eq!(map_key_event_to_action(KeyCode::Up, false), Action::MoveUp);
        assert_eq!(
            map_key_event_to_action(KeyCode::Down, false),
            Action::MoveDown
        );
        assert_eq!(
            map_key_event_to_action(KeyCode::PageUp, false),
            Action::PageUp
        );
        assert_eq!(
            map_key_event_to_action(KeyCode::PageDown, false),
            Action::PageDown
        );
        assert_eq!(
            map_key_event_to_action(KeyCode::Char('1'), false),
            Action::BeginSignalConfirmation(1)
        );
        assert_eq!(
            map_key_event_to_action(KeyCode::Char('0'), false),
            Action::Noop
        );
        assert_eq!(map_key_event_to_action(KeyCode::Left, false), Action::Noop);
    }

    #[test]
    fn map_key_event_to_action_maps_pending_confirmation_actions() {
        assert_eq!(
            map_key_event_to_action(KeyCode::Enter, true),
            Action::ConfirmPendingSignal
        );
        assert_eq!(
            map_key_event_to_action(KeyCode::Char('Y'), true),
            Action::ConfirmPendingSignal
        );
        assert_eq!(
            map_key_event_to_action(KeyCode::Esc, true),
            Action::CancelPendingSignal
        );
        assert_eq!(map_key_event_to_action(KeyCode::Up, true), Action::Noop);
    }

    #[test]
    fn apply_action_confirm_pending_signal_refreshes_and_sends() {
        let mut app = App::with_rows(None, vec![row(11, "foo")]);
        app.begin_signal_confirmation(1);
        let mut refresh_calls = 0;
        let mut refresh = || {
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

        assert!(!apply_action(
            &mut app,
            Action::ConfirmPendingSignal,
            &mut refresh,
            &mut sender
        ));
        assert!(sent);
        assert_eq!(refresh_calls, 2);
        assert!(app.pending_confirmation.is_none());
    }

    #[test]
    fn apply_action_cancel_pending_signal_clears_confirmation() {
        let mut app = App::with_rows(None, vec![row(11, "foo")]);
        app.begin_signal_confirmation(1);
        let mut refresh = || vec![row(11, "foo")];
        let mut sender = |_: i32, _: Signal| Ok(());

        assert!(!apply_action(
            &mut app,
            Action::CancelPendingSignal,
            &mut refresh,
            &mut sender
        ));
        assert!(app.pending_confirmation.is_none());
    }

    #[test]
    fn apply_action_quit_returns_true() {
        let mut app = App::with_rows(None, vec![row(11, "foo")]);
        let mut refresh = || vec![row(11, "foo")];
        let mut sender = |_: i32, _: Signal| Ok(());
        assert!(apply_action(
            &mut app,
            Action::Quit,
            &mut refresh,
            &mut sender
        ));
    }

    #[test]
    fn apply_action_refresh_reloads_rows() {
        let mut app = App::with_rows(None, vec![row(11, "foo")]);
        let mut refresh = || vec![row(22, "bar")];
        let mut sender = |_: i32, _: Signal| Ok(());
        assert!(!apply_action(
            &mut app,
            Action::Refresh,
            &mut refresh,
            &mut sender
        ));
        assert_eq!(app.rows[0].pid, 22);
    }

    #[test]
    fn apply_action_move_actions_change_selection() {
        let mut app = App::with_rows(None, vec![row(11, "foo"), row(22, "bar"), row(33, "baz")]);
        let mut refresh = || vec![row(11, "foo"), row(22, "bar"), row(33, "baz")];
        let mut sender = |_: i32, _: Signal| Ok(());

        assert!(!apply_action(
            &mut app,
            Action::MoveDown,
            &mut refresh,
            &mut sender
        ));
        assert_eq!(app.table_state.selected(), Some(1));

        assert!(!apply_action(
            &mut app,
            Action::MoveUp,
            &mut refresh,
            &mut sender
        ));
        assert_eq!(app.table_state.selected(), Some(0));
    }

    #[test]
    fn apply_action_page_actions_change_selection() {
        let rows: Vec<ProcRow> = (0..25).map(|i| row(i + 1, "p")).collect();
        let mut app = App::with_rows(None, rows.clone());
        let mut refresh = || rows.clone();
        let mut sender = |_: i32, _: Signal| Ok(());

        assert!(!apply_action(
            &mut app,
            Action::PageDown,
            &mut refresh,
            &mut sender
        ));
        assert_eq!(app.table_state.selected(), Some(10));

        assert!(!apply_action(
            &mut app,
            Action::PageUp,
            &mut refresh,
            &mut sender
        ));
        assert_eq!(app.table_state.selected(), Some(0));
    }

    #[test]
    fn apply_action_begin_signal_confirmation_sets_pending() {
        let mut app = App::with_rows(None, vec![row(11, "foo")]);
        let mut refresh = || vec![row(11, "foo")];
        let mut sender = |_: i32, _: Signal| Ok(());

        assert!(!apply_action(
            &mut app,
            Action::BeginSignalConfirmation(1),
            &mut refresh,
            &mut sender
        ));
        assert!(app.pending_confirmation.is_some());
    }

    #[test]
    fn apply_action_confirm_pending_signal_aborts_on_target_change() {
        let mut app = App::with_rows(None, vec![row(11, "foo")]);
        app.begin_signal_confirmation(1);
        let mut refresh = || vec![row(22, "bar")];
        let mut sender = |_: i32, _: Signal| Ok(());

        assert!(!apply_action(
            &mut app,
            Action::ConfirmPendingSignal,
            &mut refresh,
            &mut sender
        ));
        assert!(app.status.contains("aborted"));
    }

    #[test]
    fn apply_action_noop_is_noop() {
        let mut app = App::with_rows(None, vec![row(11, "foo")]);
        let mut refresh = || vec![row(11, "foo")];
        let mut sender = |_: i32, _: Signal| Ok(());
        let selected = app.table_state.selected();
        assert!(!apply_action(
            &mut app,
            Action::Noop,
            &mut refresh,
            &mut sender
        ));
        assert_eq!(app.table_state.selected(), selected);
    }
}
