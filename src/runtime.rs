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

use std::{io, time::Duration};

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use crossterm::{
    event, execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use nix::sys::signal::Signal;
use ratatui::{Terminal, prelude::CrosstermBackend};
use sysinfo::System;

use crate::{
    app::{self, App},
    model::ProcRow,
    process, signal, ui,
};

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
    /// Collapse the selected tree row.
    CollapseTree,
    /// Expand the selected tree row.
    ExpandTree,
    /// Open signal confirmation for a digit-mapped signal.
    BeginSignalConfirmation(u8),
    /// Confirm and dispatch the pending signal action.
    ConfirmPendingSignal,
    /// Cancel the pending signal action.
    CancelPendingSignal,
    /// Open the interactive `/` filter prompt.
    BeginInteractiveFilter,
    /// Append a character to the interactive filter input.
    FilterInputChar(char),
    /// Remove the last character from the interactive filter input.
    FilterInputBackspace,
    /// Confirm and apply the interactive filter.
    FilterConfirm,
    /// Discard the interactive filter and restore the previous state.
    FilterCancel,
    /// Intentionally perform no state change.
    Noop,
}

/// Outcome of applying an action to application state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionResult {
    /// Whether the event loop should exit after this action.
    pub should_quit: bool,
    /// Whether UI should be redrawn after this action.
    pub needs_redraw: bool,
}

/// Map a key press to a runtime action.
pub fn map_key_event_to_action(
    key_code: KeyCode,
    pending_confirmation: bool,
    in_filter_mode: bool,
) -> Action {
    if pending_confirmation {
        return match key_code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                Action::ConfirmPendingSignal
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Action::CancelPendingSignal,
            _ => Action::Noop,
        };
    }

    if in_filter_mode {
        return match key_code {
            KeyCode::Char(c) => Action::FilterInputChar(c),
            KeyCode::Backspace => Action::FilterInputBackspace,
            KeyCode::Enter => Action::FilterConfirm,
            KeyCode::Esc => Action::FilterCancel,
            // Allow scrolling through results without leaving filter mode.
            KeyCode::Up => Action::MoveUp,
            KeyCode::Down => Action::MoveDown,
            KeyCode::PageUp => Action::PageUp,
            KeyCode::PageDown => Action::PageDown,
            KeyCode::Left => Action::CollapseTree,
            KeyCode::Right => Action::ExpandTree,
            _ => Action::Noop,
        };
    }

    match key_code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('r') => Action::Refresh,
        KeyCode::Char('/') => Action::BeginInteractiveFilter,
        KeyCode::Up => Action::MoveUp,
        KeyCode::Down => Action::MoveDown,
        KeyCode::PageUp => Action::PageUp,
        KeyCode::PageDown => Action::PageDown,
        KeyCode::Left => Action::CollapseTree,
        KeyCode::Right => Action::ExpandTree,
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
    refresh_rows: &mut dyn FnMut(Option<&process::FilterSpec>) -> Vec<ProcRow>,
    sender: &mut dyn FnMut(i32, Signal) -> Result<(), String>,
) -> ActionResult {
    match action {
        Action::Quit => ActionResult {
            should_quit: true,
            needs_redraw: false,
        },
        Action::Refresh => {
            let f = app.compiled_filter.clone();
            app.refresh(refresh_rows(f.as_ref()));
            ActionResult {
                should_quit: false,
                needs_redraw: true,
            }
        }
        Action::MoveUp => {
            let selected_before = app.table_state.selected();
            app.move_up();
            ActionResult {
                should_quit: false,
                needs_redraw: app.table_state.selected() != selected_before,
            }
        }
        Action::MoveDown => {
            let selected_before = app.table_state.selected();
            app.move_down();
            ActionResult {
                should_quit: false,
                needs_redraw: app.table_state.selected() != selected_before,
            }
        }
        Action::PageUp => {
            let selected_before = app.table_state.selected();
            app.page_up(PAGE_STEP);
            ActionResult {
                should_quit: false,
                needs_redraw: app.table_state.selected() != selected_before,
            }
        }
        Action::PageDown => {
            let selected_before = app.table_state.selected();
            app.page_down(PAGE_STEP);
            ActionResult {
                should_quit: false,
                needs_redraw: app.table_state.selected() != selected_before,
            }
        }
        Action::CollapseTree => ActionResult {
            should_quit: false,
            needs_redraw: app.collapse_selected(),
        },
        Action::ExpandTree => ActionResult {
            should_quit: false,
            needs_redraw: app.expand_selected(),
        },
        Action::BeginSignalConfirmation(digit) => {
            let had_pending = app.pending_confirmation.is_some();
            app.begin_signal_confirmation(digit);
            ActionResult {
                should_quit: false,
                needs_redraw: app.pending_confirmation.is_some() != had_pending,
            }
        }
        Action::ConfirmPendingSignal => {
            refresh_with_selection_preserved(app, refresh_rows);
            if !app.pending_target_matches_current_rows() {
                app.abort_pending_target_changed();
                return ActionResult {
                    should_quit: false,
                    needs_redraw: true,
                };
            }

            app.confirm_signal(sender);
            refresh_with_selection_preserved(app, refresh_rows);
            ActionResult {
                should_quit: false,
                needs_redraw: true,
            }
        }
        Action::CancelPendingSignal => {
            let had_pending = app.pending_confirmation.is_some();
            app.cancel_signal_confirmation();
            ActionResult {
                should_quit: false,
                needs_redraw: had_pending,
            }
        }
        Action::BeginInteractiveFilter => {
            // Pre-fill with existing text only when the active filter is substring
            // (don't pre-fill regex patterns into substring mode).
            let pre_fill = match &app.compiled_filter {
                Some(process::FilterSpec::Substring { raw, .. }) => raw.clone(),
                _ => String::new(),
            };
            let compiled = process::compile_filter(
                if pre_fill.is_empty() {
                    None
                } else {
                    Some(pre_fill.clone())
                },
                false,
            )
            .ok()
            .flatten();
            app.filter_input = Some(app::FilterInput {
                text: pre_fill,
                compiled,
            });
            ActionResult {
                should_quit: false,
                needs_redraw: true,
            }
        }
        Action::FilterInputChar(c) => {
            let Some(ref mut fi) = app.filter_input else {
                return ActionResult {
                    should_quit: false,
                    needs_redraw: false,
                };
            };
            fi.text.push(c);
            // After push the text is always non-empty, so always compile.
            fi.compiled = process::compile_filter(Some(fi.text.clone()), false)
                .ok()
                .flatten();
            let f = fi.compiled.clone();
            app.refresh_preserving_status(refresh_rows(f.as_ref()));
            // Jump to first result so all matches are visible from the top.
            app.select_first();
            ActionResult {
                should_quit: false,
                needs_redraw: true,
            }
        }
        Action::FilterInputBackspace => {
            let Some(ref mut fi) = app.filter_input else {
                return ActionResult {
                    should_quit: false,
                    needs_redraw: false,
                };
            };
            fi.text.pop();
            fi.compiled = process::compile_filter(
                if fi.text.is_empty() {
                    None
                } else {
                    Some(fi.text.clone())
                },
                false,
            )
            .ok()
            .flatten();
            let f = fi.compiled.clone();
            app.refresh_preserving_status(refresh_rows(f.as_ref()));
            app.select_first();
            ActionResult {
                should_quit: false,
                needs_redraw: true,
            }
        }
        Action::FilterConfirm => {
            if let Some(fi) = app.filter_input.take() {
                // Use the already-compiled spec when available; recompile from text
                // as a fallback (e.g. when FilterInput was constructed manually).
                let compiled = fi.compiled.or_else(|| {
                    process::compile_filter(
                        if fi.text.is_empty() {
                            None
                        } else {
                            Some(fi.text.clone())
                        },
                        false,
                    )
                    .ok()
                    .flatten()
                });
                app.filter = if fi.text.is_empty() {
                    None
                } else {
                    Some(fi.text)
                };
                app.compiled_filter = compiled;
            }
            let f = app.compiled_filter.clone();
            app.refresh(refresh_rows(f.as_ref()));
            ActionResult {
                should_quit: false,
                needs_redraw: true,
            }
        }
        Action::FilterCancel => {
            let was_active = app.filter_input.take().is_some();
            if was_active {
                let f = app.compiled_filter.clone();
                app.refresh_preserving_status(refresh_rows(f.as_ref()));
            }
            ActionResult {
                should_quit: false,
                needs_redraw: was_active,
            }
        }
        Action::Noop => ActionResult {
            should_quit: false,
            needs_redraw: false,
        },
    }
}

/// Run the interactive loop using injectable draw and event hooks.
pub fn run_event_loop(
    app: &mut App,
    draw: &mut dyn FnMut(&mut App) -> Result<()>,
    next_event: &mut dyn FnMut(Duration) -> Result<Option<Event>>,
    refresh_rows: &mut dyn FnMut(Option<&process::FilterSpec>) -> Vec<ProcRow>,
    sender: &mut dyn FnMut(i32, Signal) -> Result<(), String>,
) -> Result<()> {
    let mut needs_redraw = true;

    loop {
        if needs_redraw {
            draw(app)?;
            needs_redraw = false;
        }

        if let Some(event) = next_event(Duration::from_millis(250))? {
            match event {
                Event::Resize(_, _) => {
                    needs_redraw = true;
                }
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    let action = map_key_event_to_action(
                        key.code,
                        app.pending_confirmation.is_some(),
                        app.filter_input.is_some(),
                    );
                    let outcome = apply_action(app, action, refresh_rows, sender);
                    if outcome.should_quit {
                        break;
                    }
                    needs_redraw |= outcome.needs_redraw;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

/// Run the interactive terminal session with concrete TUI/system dependencies.
pub fn run_interactive(
    filter: Option<String>,
    compiled_filter: Option<process::FilterSpec>,
    user_only: bool,
) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut sys = System::new_all();

    let mut draw = |app: &mut App| -> Result<()> {
        terminal.draw(|frame| ui::render(frame, app))?;
        Ok(())
    };
    let mut next_event = |timeout| -> Result<Option<Event>> {
        if event::poll(timeout)? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    };
    let mut refresh_rows =
        |filter: Option<&process::FilterSpec>| process::refresh_rows(&mut sys, filter, user_only);
    let mut sender = |pid, sig| signal::send_signal(pid, sig).map_err(|err| err.to_string());
    let result = run_with_runtime(
        filter,
        compiled_filter,
        &mut draw,
        &mut next_event,
        &mut refresh_rows,
        &mut sender,
    );
    restore_terminal(terminal);
    result
}

fn run_with_runtime(
    filter: Option<String>,
    compiled_filter: Option<process::FilterSpec>,
    draw: &mut dyn FnMut(&mut App) -> Result<()>,
    next_event: &mut dyn FnMut(Duration) -> Result<Option<Event>>,
    refresh_rows: &mut dyn FnMut(Option<&process::FilterSpec>) -> Vec<ProcRow>,
    sender: &mut dyn FnMut(i32, Signal) -> Result<(), String>,
) -> Result<()> {
    let initial_rows = refresh_rows(compiled_filter.as_ref());
    let mut app = App::with_rows(filter, initial_rows);
    app.compiled_filter = compiled_filter;
    run_event_loop(&mut app, draw, next_event, refresh_rows, sender)
}

/// Configure terminal raw mode and alternate screen for TUI rendering.
fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

/// Restore terminal state after TUI execution, ignoring restoration failures.
fn restore_terminal(mut terminal: Terminal<CrosstermBackend<io::Stdout>>) {
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();
}

/// Refresh rows while keeping selection bounded to the previous index.
fn refresh_with_selection_preserved(
    app: &mut App,
    refresh_rows: &mut dyn FnMut(Option<&process::FilterSpec>) -> Vec<ProcRow>,
) {
    let f = app.compiled_filter.clone();
    app.refresh_preserving_status(refresh_rows(f.as_ref()));
}

#[cfg(test)]
mod tests {
    use super::{
        Action, ActionResult, apply_action, map_key_event_to_action, run_event_loop,
        run_with_runtime,
    };
    use crate::{
        app::{self, App},
        model::ProcRow,
    };
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use nix::sys::signal::Signal;
    use std::sync::Arc;
    use std::time::Duration;
    use sysinfo::ProcessStatus;

    fn row(pid: i32, name: &str) -> ProcRow {
        ProcRow {
            pid,
            start_time: 0,
            ppid: None,
            ancestor_chain: Vec::new(),
            user: Arc::from("u"),
            status: ProcessStatus::Run,
            cpu_usage_tenths: 0,
            memory_bytes: 0,
            name: name.to_string(),
            cmd: format!("/bin/{name}"),
        }
    }

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
                &mut sender
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
                &mut sender
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
            apply_action(&mut app, Action::Quit, &mut refresh, &mut sender),
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
            apply_action(&mut app, Action::Refresh, &mut refresh, &mut sender),
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
        apply_action(&mut app, Action::Refresh, &mut refresh, &mut sender);

        assert_eq!(
            apply_action(&mut app, Action::MoveDown, &mut refresh, &mut sender),
            ActionResult {
                should_quit: false,
                needs_redraw: true
            }
        );
        assert_eq!(app.table_state.selected(), Some(1));

        assert_eq!(
            apply_action(&mut app, Action::MoveUp, &mut refresh, &mut sender),
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
            apply_action(&mut app, Action::PageDown, &mut refresh, &mut sender),
            ActionResult {
                should_quit: false,
                needs_redraw: true
            }
        );
        assert_eq!(app.table_state.selected(), Some(10));

        assert_eq!(
            apply_action(&mut app, Action::PageUp, &mut refresh, &mut sender),
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
            apply_action(&mut app, Action::CollapseTree, &mut refresh, &mut sender),
            ActionResult {
                should_quit: false,
                needs_redraw: true
            }
        );
        assert!(app.collapsed_pids.contains(&2));

        assert_eq!(
            apply_action(&mut app, Action::ExpandTree, &mut refresh, &mut sender),
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
                &mut sender
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
                &mut sender
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
            apply_action(&mut app, Action::Noop, &mut refresh, &mut sender),
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
        let release =
            KeyEvent::new_with_kind(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Release);
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
        )
        .expect("runtime should terminate cleanly");

        assert_eq!(refresh_calls, 1);
        assert!(draw_calls >= 1);
    }

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
                &mut sender
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
        let mut refresh =
            |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo"), row(22, "bar")];
        let mut sender = |_: i32, _: Signal| Ok(());

        apply_action(
            &mut app,
            Action::FilterInputChar('f'),
            &mut refresh,
            &mut sender,
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

        apply_action(&mut app, Action::FilterConfirm, &mut refresh, &mut sender);
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

        let result = apply_action(&mut app, Action::FilterCancel, &mut refresh, &mut sender);
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
        let mut refresh =
            |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo"), row(22, "bar")];
        let mut sender = |_: i32, _: Signal| Ok(());

        apply_action(
            &mut app,
            Action::FilterInputBackspace,
            &mut refresh,
            &mut sender,
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

        apply_action(&mut app, Action::FilterConfirm, &mut refresh, &mut sender);
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
        let mut refresh =
            |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo"), row(22, "bar")];
        let mut sender = |_: i32, _: Signal| Ok(());

        apply_action(&mut app, Action::FilterConfirm, &mut refresh, &mut sender);
        assert!(app.filter_input.is_none());
        assert!(app.filter.is_none());
        assert!(app.compiled_filter.is_none());
    }

    #[test]
    fn apply_action_filter_cancel_noop_when_not_active() {
        let mut app = App::with_rows(None, vec![row(11, "foo")]);
        let mut refresh = |_: Option<&crate::process::FilterSpec>| vec![row(22, "bar")];
        let mut sender = |_: i32, _: Signal| Ok(());

        let result = apply_action(&mut app, Action::FilterCancel, &mut refresh, &mut sender);
        assert!(!result.needs_redraw);
        // Rows must not change since there was nothing to cancel.
        assert_eq!(app.rows[0].pid, 11);
    }

    #[test]
    fn apply_action_filter_input_char_resets_selection_to_first() {
        let mut app = App::with_rows(None, vec![row(11, "foo"), row(22, "bar")]);
        app.filter_input = Some(app::FilterInput {
            text: String::new(),
            compiled: None,
        });
        app.table_state.select(Some(1)); // pre-select last row
        let mut refresh =
            |_: Option<&crate::process::FilterSpec>| vec![row(11, "foo"), row(22, "bar")];
        let mut sender = |_: i32, _: Signal| Ok(());

        apply_action(
            &mut app,
            Action::FilterInputChar('f'),
            &mut refresh,
            &mut sender,
        );
        assert_eq!(app.table_state.selected(), Some(0));
    }
}
