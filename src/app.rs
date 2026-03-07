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

use std::{cmp::min, collections::HashSet};

use nix::sys::signal::Signal;
use ratatui::widgets::TableState;

use crate::{
    model::ProcRow,
    process::FilterSpec,
    signal::signal_from_digit,
    tree::{display_order_indices, display_rows},
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ProcessIdentity {
    pid: i32,
    start_time: u64,
}

/// State for the interactive `/` filter prompt.
#[derive(Debug)]
pub struct FilterInput {
    /// Raw text the user has typed so far.
    pub text: String,
    /// Compiled substring spec for `text`; `None` when `text` is empty.
    pub compiled: Option<FilterSpec>,
}

/// Mutable application state shared between input handling and rendering.
#[derive(Debug)]
pub struct App {
    /// Optional process filter supplied from argv.
    pub filter: Option<String>,
    /// Compiled form of the active CLI filter (substring or regex).
    pub compiled_filter: Option<FilterSpec>,
    /// Current table rows.
    pub rows: Vec<ProcRow>,
    /// Selected row index in the process table.
    pub table_state: TableState,
    /// Footer status message.
    pub status: String,
    /// Pending signal confirmation modal state.
    pub pending_confirmation: Option<SignalConfirmation>,
    /// Pids whose visible descendants are hidden in tree mode.
    pub collapsed_pids: HashSet<i32>,
    /// Active interactive filter prompt; `Some` while the user is typing `/`.
    pub filter_input: Option<FilterInput>,
}

/// Pending signal action that requires user confirmation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignalConfirmation {
    /// Original 1-9 key entered by the user.
    pub digit: u8,
    /// Resolved Unix signal for `digit`.
    pub signal: Signal,
    /// Target process id.
    pub pid: i32,
    /// Target process start time to guard against pid reuse.
    pub start_time: u64,
    /// Target process name.
    pub process_name: String,
}

impl App {
    /// Build app state from filter and initial rows.
    pub fn with_rows(filter: Option<String>, rows: Vec<ProcRow>) -> Self {
        let mut table_state = TableState::default();
        table_state.select(if rows.is_empty() { None } else { Some(0) });

        Self {
            filter,
            compiled_filter: None,
            rows,
            table_state,
            status: String::new(),
            pending_confirmation: None,
            collapsed_pids: HashSet::new(),
            filter_input: None,
        }
    }

    /// Return the currently configured filter as a borrowed string.
    pub fn filter(&self) -> Option<&str> {
        self.filter.as_deref()
    }

    /// Return the compiled filter that should be used for row matching and highlighting.
    /// Prefers the interactive filter input when active, falls back to the CLI filter.
    pub fn active_filter(&self) -> Option<&FilterSpec> {
        self.filter_input
            .as_ref()
            .and_then(|fi| fi.compiled.as_ref())
            .or(self.compiled_filter.as_ref())
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
        let selected_identity = self.selected_row().map(ProcessIdentity::from_row);
        let collapsed_identities: HashSet<ProcessIdentity> = self
            .rows
            .iter()
            .filter(|row| self.collapsed_pids.contains(&row.pid))
            .map(ProcessIdentity::from_row)
            .collect();
        self.rows = rows;
        self.collapsed_pids = self
            .rows
            .iter()
            .filter(|row| collapsed_identities.contains(&ProcessIdentity::from_row(row)))
            .map(|row| row.pid)
            .collect();

        let visible_count = self.visible_row_count();
        if visible_count == 0 {
            self.table_state.select(None);
        } else if let Some(identity) = selected_identity
            && let Some(index) = display_order_indices(&self.rows, &self.collapsed_pids)
                .iter()
                .position(|row_index| ProcessIdentity::from_row(&self.rows[*row_index]) == identity)
        {
            self.table_state.select(Some(index));
        } else {
            self.table_state
                .select(Some(min(selected_before, visible_count - 1)));
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
        let visible_count = self.visible_row_count();
        if let Some(selected) = self.table_state.selected() {
            if selected + 1 < visible_count {
                self.table_state.select(Some(selected + 1));
            }
        } else if visible_count > 0 {
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
            let visible_count = self.visible_row_count();
            if visible_count == 0 {
                self.table_state.select(None);
                return;
            }

            let last_index = visible_count - 1;
            let next_index = selected.saturating_add(step);
            self.table_state.select(Some(min(next_index, last_index)));
        } else {
            let visible_count = self.visible_row_count();
            if visible_count == 0 {
                return;
            }
            self.table_state
                .select(Some(min(step - 1, visible_count - 1)));
        }
    }

    /// Collapse the selected row when it currently shows descendants.
    pub fn collapse_selected(&mut self) -> bool {
        let Some(display_row) = self.selected_display_row() else {
            return false;
        };
        if !display_row.has_children || display_row.is_collapsed {
            return false;
        }

        let pid = self.rows[display_row.row_index].pid;
        self.collapsed_pids.insert(pid)
    }

    /// Expand the selected row when it currently hides descendants.
    pub fn expand_selected(&mut self) -> bool {
        let Some(display_row) = self.selected_display_row() else {
            return false;
        };
        if !display_row.is_collapsed {
            return false;
        }

        let pid = self.rows[display_row.row_index].pid;
        self.collapsed_pids.remove(&pid)
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

        let row = match self.selected_row() {
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

    /// Prepare a confirmation modal for a digit-mapped signal.
    pub fn begin_signal_confirmation(&mut self, digit: u8) {
        let signal = match signal_from_digit(digit) {
            Some(value) => value,
            None => return,
        };

        let row = match self.selected_row() {
            Some(value) => value,
            None => return,
        };

        self.pending_confirmation = Some(SignalConfirmation {
            digit,
            signal,
            pid: row.pid,
            start_time: row.start_time,
            process_name: row.name.clone(),
        });
    }

    /// Cancel any active signal confirmation modal.
    pub fn cancel_signal_confirmation(&mut self) {
        self.pending_confirmation = None;
    }

    /// Confirm and execute a pending signal action.
    pub fn confirm_signal(&mut self, sender: &mut dyn FnMut(i32, Signal) -> Result<(), String>) {
        let Some(pending) = self.pending_confirmation.take() else {
            return;
        };

        match sender(pending.pid, pending.signal) {
            Ok(()) => {
                self.status = format!(
                    "sent {:?} ({}) to pid {}",
                    pending.signal, pending.digit, pending.pid
                );
            }
            Err(err) => {
                self.status = format!("failed to signal pid {}: {}", pending.pid, err);
            }
        }
    }

    /// Check whether the pending confirmation target still matches current rows.
    pub fn pending_target_matches_current_rows(&self) -> bool {
        let Some(pending) = self.pending_confirmation.as_ref() else {
            return false;
        };

        self.rows
            .iter()
            .any(|row| row.pid == pending.pid && row.start_time == pending.start_time)
    }

    /// Abort pending confirmation because the target no longer matches current rows.
    pub fn abort_pending_target_changed(&mut self) {
        let Some(pending) = self.pending_confirmation.take() else {
            return;
        };

        self.status = format!(
            "aborted: process {} ({}) no longer matches confirmation target",
            pending.process_name, pending.pid
        );
    }

    /// Build the confirmation prompt text for the current pending action.
    pub fn confirmation_prompt(&self) -> Option<String> {
        self.pending_confirmation.as_ref().map(|pending| {
            format!(
                "confirm sending {:?} ({}) to process {} ({}) (y/n)",
                pending.signal, pending.digit, pending.process_name, pending.pid
            )
        })
    }

    fn selected_row(&self) -> Option<&ProcRow> {
        let selected_display_index = self.table_state.selected()?;
        let display_to_data = display_order_indices(&self.rows, &self.collapsed_pids);
        let row_index = *display_to_data.get(selected_display_index)?;
        self.rows.get(row_index)
    }

    fn selected_display_row(&self) -> Option<crate::tree::DisplayRow> {
        let selected_display_index = self.table_state.selected()?;
        display_rows(&self.rows, &self.collapsed_pids)
            .get(selected_display_index)
            .cloned()
    }

    fn visible_row_count(&self) -> usize {
        display_order_indices(&self.rows, &self.collapsed_pids).len()
    }
}

impl ProcessIdentity {
    fn from_row(row: &ProcRow) -> Self {
        Self {
            pid: row.pid,
            start_time: row.start_time,
        }
    }
}
