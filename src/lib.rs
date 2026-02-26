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

//! Core library for `psn`.

pub mod app;
pub mod cli;
pub mod model;
pub mod process;
pub mod signal;
pub mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, prelude::CrosstermBackend};
use std::{cmp::min, io, time::Duration};

const PAGE_STEP: usize = 10;
use sysinfo::System;

use crate::{app::App, model::ProcRow};

/// Run the interactive TUI application.
pub fn run(filter: Option<String>, regex_mode: bool, user_only: bool) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut sys = System::new_all();
    let compiled_filter = process::compile_filter(filter.clone(), regex_mode)?;
    let initial_rows = process::refresh_rows(&mut sys, compiled_filter.as_ref(), user_only);
    let mut app = App::with_rows(filter, initial_rows);

    let run_result = (|| -> Result<()> {
        loop {
            terminal.draw(|frame| ui::render(frame, &mut app))?;

            if event::poll(Duration::from_millis(60))?
                && let Event::Key(key) = event::read()?
            {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if app.pending_confirmation.is_some() {
                    let mut refresh_rows =
                        || process::refresh_rows(&mut sys, compiled_filter.as_ref(), user_only);
                    let mut sender =
                        |pid, sig| signal::send_signal(pid, sig).map_err(|err| err.to_string());
                    if handle_pending_confirmation_input(
                        &mut app,
                        key.code,
                        &mut refresh_rows,
                        &mut sender,
                    ) {
                        continue;
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('r') => {
                        app.refresh(process::refresh_rows(
                            &mut sys,
                            compiled_filter.as_ref(),
                            user_only,
                        ));
                    }
                    KeyCode::Up => app.move_up(),
                    KeyCode::Down => app.move_down(),
                    KeyCode::PageUp => app.page_up(PAGE_STEP),
                    KeyCode::PageDown => app.page_down(PAGE_STEP),
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        let digit = c.to_digit(10).unwrap_or_default() as u8;
                        if !(1..=9).contains(&digit) {
                            continue;
                        }
                        app.begin_signal_confirmation(digit);
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    })();

    restore_terminal(terminal);
    run_result
}

fn handle_pending_confirmation_input(
    app: &mut App,
    key_code: KeyCode,
    refresh_rows: &mut dyn FnMut() -> Vec<ProcRow>,
    sender: &mut dyn FnMut(i32, nix::sys::signal::Signal) -> Result<(), String>,
) -> bool {
    match key_code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            let selected_before_refresh = app.table_state.selected().unwrap_or(0);
            app.refresh_preserving_status(refresh_rows());
            if !app.rows.is_empty() {
                app.table_state
                    .select(Some(min(selected_before_refresh, app.rows.len() - 1)));
            }

            if !app.pending_target_matches_current_rows() {
                app.abort_pending_target_changed();
                return true;
            }

            app.confirm_signal(sender);
            let selected_before_refresh = app.table_state.selected().unwrap_or(0);
            app.refresh_preserving_status(refresh_rows());
            if !app.rows.is_empty() {
                app.table_state
                    .select(Some(min(selected_before_refresh, app.rows.len() - 1)));
            }
            true
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.cancel_signal_confirmation();
            true
        }
        _ => true,
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(mut terminal: Terminal<CrosstermBackend<io::Stdout>>) {
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();
}

#[cfg(test)]
mod tests {
    use super::handle_pending_confirmation_input;
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
    fn pending_key_y_sends_when_target_still_matches() {
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

        assert!(handle_pending_confirmation_input(
            &mut app,
            KeyCode::Char('y'),
            &mut refresh,
            &mut sender
        ));
        assert!(sent);
        assert_eq!(refresh_calls, 2);
        assert!(app.pending_confirmation.is_none());
    }

    #[test]
    fn pending_key_enter_sends_when_target_still_matches() {
        let mut app = App::with_rows(None, vec![row(11, "foo")]);
        app.begin_signal_confirmation(1);

        let mut refresh = || vec![row(11, "foo")];
        let mut sent = false;
        let mut sender = |_: i32, _: Signal| {
            sent = true;
            Ok(())
        };

        assert!(handle_pending_confirmation_input(
            &mut app,
            KeyCode::Enter,
            &mut refresh,
            &mut sender
        ));
        assert!(sent);
        assert!(app.pending_confirmation.is_none());
    }

    #[test]
    fn pending_key_y_aborts_when_target_changes_after_refresh() {
        let mut app = App::with_rows(None, vec![row(11, "foo")]);
        app.begin_signal_confirmation(1);

        let mut refresh = || vec![row(22, "bar")];
        let mut sender_called = false;
        let mut sender = |_: i32, _: Signal| {
            sender_called = true;
            Ok(())
        };

        assert!(handle_pending_confirmation_input(
            &mut app,
            KeyCode::Char('y'),
            &mut refresh,
            &mut sender
        ));
        assert!(!sender_called);
        assert!(app.status.contains("aborted"));
        assert!(app.pending_confirmation.is_none());
    }

    #[test]
    fn pending_key_n_cancels_confirmation() {
        let mut app = App::with_rows(None, vec![row(11, "foo")]);
        app.begin_signal_confirmation(1);

        let mut refresh = || vec![row(11, "foo")];
        let mut sender = |_: i32, _: Signal| Ok(());

        assert!(handle_pending_confirmation_input(
            &mut app,
            KeyCode::Char('n'),
            &mut refresh,
            &mut sender
        ));
        assert!(app.pending_confirmation.is_none());
    }

    #[test]
    fn pending_other_key_is_consumed_without_changes() {
        let mut app = App::with_rows(None, vec![row(11, "foo")]);
        app.begin_signal_confirmation(1);

        let mut refresh = || vec![row(11, "foo")];
        let mut sender = |_: i32, _: Signal| Ok(());

        assert!(handle_pending_confirmation_input(
            &mut app,
            KeyCode::Up,
            &mut refresh,
            &mut sender
        ));
        assert!(app.pending_confirmation.is_some());
    }
}
