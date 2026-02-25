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

//! Application state and interaction logic.

use std::cmp::min;

use nix::sys::signal::Signal;
use ratatui::widgets::TableState;

use crate::{model::ProcRow, signal::signal_from_digit};

/// Mutable application state shared between input handling and rendering.
#[derive(Debug)]
pub struct App {
    /// Optional process filter supplied from argv.
    pub filter: Option<String>,
    /// Current table rows.
    pub rows: Vec<ProcRow>,
    /// Selected row index in the process table.
    pub table_state: TableState,
    /// Footer status message.
    pub status: String,
}

impl App {
    /// Build app state from filter and initial rows.
    pub fn with_rows(filter: Option<String>, rows: Vec<ProcRow>) -> Self {
        let mut table_state = TableState::default();
        table_state.select(if rows.is_empty() { None } else { Some(0) });

        Self {
            filter,
            rows,
            table_state,
            status: String::new(),
        }
    }

    /// Return the currently configured filter as a borrowed string.
    pub fn filter(&self) -> Option<&str> {
        self.filter.as_deref()
    }

    /// Replace row data, keep selection bounded, and clear status text.
    pub fn refresh(&mut self, rows: Vec<ProcRow>) {
        self.apply_rows(rows);
        self.status.clear();
    }

    /// Replace row data while preserving current status text.
    pub fn refresh_preserving_status(&mut self, rows: Vec<ProcRow>) {
        self.apply_rows(rows);
    }

    /// Update rows and clamp selection to valid bounds.
    fn apply_rows(&mut self, rows: Vec<ProcRow>) {
        let selected_before = self.table_state.selected().unwrap_or(0);
        self.rows = rows;

        if self.rows.is_empty() {
            self.table_state.select(None);
        } else {
            self.table_state
                .select(Some(min(selected_before, self.rows.len() - 1)));
        }
    }

    /// Move selection one row up.
    pub fn move_up(&mut self) {
        if let Some(selected) = self.table_state.selected()
            && selected > 0
        {
            self.table_state.select(Some(selected - 1));
        }
    }

    /// Move selection one row down.
    pub fn move_down(&mut self) {
        if let Some(selected) = self.table_state.selected() {
            if selected + 1 < self.rows.len() {
                self.table_state.select(Some(selected + 1));
            }
        } else if !self.rows.is_empty() {
            self.table_state.select(Some(0));
        }
    }

    /// Move selection one page up by the provided step.
    pub fn page_up(&mut self, step: usize) {
        if step == 0 {
            return;
        }

        if let Some(selected) = self.table_state.selected() {
            self.table_state.select(Some(selected.saturating_sub(step)));
        }
    }

    /// Move selection one page down by the provided step.
    pub fn page_down(&mut self, step: usize) {
        if step == 0 {
            return;
        }

        if let Some(selected) = self.table_state.selected() {
            if self.rows.is_empty() {
                self.table_state.select(None);
                return;
            }

            let last_index = self.rows.len() - 1;
            let next_index = selected.saturating_add(step);
            self.table_state.select(Some(min(next_index, last_index)));
        } else if !self.rows.is_empty() {
            self.table_state
                .select(Some(min(step - 1, self.rows.len() - 1)));
        }
    }

    /// Send a digit-mapped signal to selected process through injected sender.
    pub fn send_digit(
        &mut self,
        digit: u8,
        sender: &mut dyn FnMut(i32, Signal) -> Result<(), String>,
    ) {
        let signal = match signal_from_digit(digit) {
            Some(value) => value,
            None => return,
        };

        let selected = match self.table_state.selected() {
            Some(value) => value,
            None => return,
        };

        let row = match self.rows.get(selected) {
            Some(value) => value,
            None => return,
        };

        match sender(row.pid, signal) {
            Ok(()) => {
                self.status = format!("sent {:?} ({}) to pid {}", signal, digit, row.pid);
            }
            Err(err) => {
                self.status = format!("failed to signal pid {}: {}", row.pid, err);
            }
        }
    }
}
