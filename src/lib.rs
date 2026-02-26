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
pub mod runtime;
pub mod signal;
pub mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, prelude::CrosstermBackend};
use std::{io, time::Duration};

use sysinfo::System;

use crate::{
    app::App,
    runtime::{apply_action, map_key_event_to_action},
};

/// Run the interactive TUI application.
pub fn run(filter: Option<String>, regex_mode: bool, user_only: bool) -> Result<()> {
    let compiled_filter = process::compile_filter(filter.clone(), regex_mode)?;

    let mut terminal = setup_terminal()?;
    let mut sys = System::new_all();
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

                let action = map_key_event_to_action(key.code, app.pending_confirmation.is_some());
                let mut refresh_rows =
                    || process::refresh_rows(&mut sys, compiled_filter.as_ref(), user_only);
                let mut sender =
                    |pid, sig| signal::send_signal(pid, sig).map_err(|err| err.to_string());
                if apply_action(&mut app, action, &mut refresh_rows, &mut sender) {
                    break;
                }
            }
        }

        Ok(())
    })();

    restore_terminal(terminal);
    run_result
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
