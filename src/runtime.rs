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

use std::time::Duration;

use anyhow::Result;
use crossterm::event;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use nix::sys::signal::Signal;
use sysinfo::System;

use crate::{
    app::{self, App},
    model::ProcRow,
    process, signal, terminal, ui,
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
    await_pid_gone: &mut dyn FnMut(i32),
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

            if let Some(pid) = app.confirm_signal(sender) {
                // Bridge the async-kill / proc-cleanup race so the upcoming
                // refresh sees the dying process as gone instead of stale.
                await_pid_gone(pid);
            }
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
            let f = compiled.clone();
            app.filter_input = Some(app::FilterInput {
                text: pre_fill,
                compiled,
            });
            // Apply the pre-filled filter immediately so the row list matches
            // what the footer shows without waiting for the first keystroke.
            app.refresh_preserving_status(refresh_rows(f.as_ref()));
            app.select_first();
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
                app.select_first();
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
    await_pid_gone: &mut dyn FnMut(i32),
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
                    let outcome = apply_action(app, action, refresh_rows, sender, await_pid_gone);
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
    let mut terminal = terminal::setup()?;
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
        &mut (signal::wait_for_pid_gone_default as fn(i32)),
    );
    terminal::restore(terminal);
    result
}

fn run_with_runtime(
    filter: Option<String>,
    compiled_filter: Option<process::FilterSpec>,
    draw: &mut dyn FnMut(&mut App) -> Result<()>,
    next_event: &mut dyn FnMut(Duration) -> Result<Option<Event>>,
    refresh_rows: &mut dyn FnMut(Option<&process::FilterSpec>) -> Vec<ProcRow>,
    sender: &mut dyn FnMut(i32, Signal) -> Result<(), String>,
    await_pid_gone: &mut dyn FnMut(i32),
) -> Result<()> {
    let initial_rows = refresh_rows(compiled_filter.as_ref());
    let mut app = App::with_rows(filter, initial_rows);
    app.compiled_filter = compiled_filter;
    run_event_loop(
        &mut app,
        draw,
        next_event,
        refresh_rows,
        sender,
        await_pid_gone,
    )
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
#[path = "runtime_tests/mod.rs"]
mod tests;
