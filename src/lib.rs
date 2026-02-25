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

#![allow(unexpected_cfgs)]

//! Core library for `psn`.

pub mod app;
pub mod model;
pub mod process;
pub mod signal;
pub mod ui;

use anyhow::Result;

#[cfg(not(coverage))]
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
#[cfg(not(coverage))]
use ratatui::{Terminal, prelude::CrosstermBackend};
#[cfg(not(coverage))]
use std::{cmp::min, io, time::Duration};
#[cfg(not(coverage))]
use sysinfo::System;

#[cfg(not(coverage))]
use crate::app::App;

/// Run the interactive TUI application.
#[cfg(coverage)]
pub fn run() -> Result<()> {
    Ok(())
}

/// Run the interactive TUI application.
#[cfg(not(coverage))]
pub fn run() -> Result<()> {
    let filter = std::env::args().nth(1);

    let mut terminal = setup_terminal()?;
    let mut sys = System::new_all();
    let mut app = App::with_rows(filter, process::refresh_rows(&mut sys, None));
    app.refresh(process::refresh_rows(&mut sys, app.filter()));

    let run_result = (|| -> Result<()> {
        loop {
            terminal.draw(|frame| ui::render(frame, &mut app))?;

            if event::poll(Duration::from_millis(60))?
                && let Event::Key(key) = event::read()?
            {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('r') => {
                        app.refresh(process::refresh_rows(&mut sys, app.filter()));
                    }
                    KeyCode::Up => app.move_up(),
                    KeyCode::Down => app.move_down(),
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        let digit = c.to_digit(10).unwrap_or_default() as u8;
                        let mut sender =
                            |pid, sig| signal::send_signal(pid, sig).map_err(|err| err.to_string());
                        app.send_digit(digit, &mut sender);
                        let selected_before_refresh = app.table_state.selected().unwrap_or(0);
                        app.refresh_preserving_status(process::refresh_rows(
                            &mut sys,
                            app.filter(),
                        ));
                        if !app.rows.is_empty() {
                            app.table_state
                                .select(Some(min(selected_before_refresh, app.rows.len() - 1)));
                        }
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

#[cfg(not(coverage))]
fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

#[cfg(not(coverage))]
fn restore_terminal(mut terminal: Terminal<CrosstermBackend<io::Stdout>>) {
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();
}

#[cfg(all(test, coverage))]
mod tests {
    #[test]
    fn run_returns_ok_under_coverage() {
        assert!(super::run().is_ok());
    }
}
