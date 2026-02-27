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

//! Hidden synthetic-data TUI used for local UI development.

use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use crossterm::event::{self, Event};
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, prelude::CrosstermBackend};
use sysinfo::ProcessStatus;

use crate::{app::App, model::ProcRow, runtime::run_event_loop, ui};

const MAX_DEBUG_ROWS: usize = 21;

/// Run the hidden debug-only TUI with synthetic process rows.
pub(crate) fn run() -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut seed = initial_seed();
    let mut draw = |app: &mut App| -> Result<()> {
        terminal.draw(|frame| ui::render(frame, app))?;
        Ok(())
    };
    let mut next_event = |timeout: Duration| -> Result<Option<Event>> {
        if event::poll(timeout)? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    };
    let mut refresh_rows = || {
        seed = next_seed(seed);
        build_debug_rows(seed)
    };
    let mut sender = debug_signal_sender;
    let mut app = App::with_rows(None, refresh_rows());
    app.status = "debug tui: synthetic rows only".to_string();
    let result = run_event_loop(
        &mut app,
        &mut draw,
        &mut next_event,
        &mut refresh_rows,
        &mut sender,
    );
    restore_terminal(terminal);
    result
}

fn build_debug_rows(seed: u64) -> Vec<ProcRow> {
    let mut rng = DebugRng::new(seed);
    let statuses = debug_statuses();
    debug_assert!(statuses.len() <= MAX_DEBUG_ROWS);

    let users = ["root", "alice", "bob", "carol", "daemon"];
    let names = [
        "palette", "renderer", "watcher", "worker", "io", "sync", "cache", "input", "signal",
        "layout", "theme", "metrics", "terminal", "preview",
    ];
    let flags = [
        "--inspect",
        "--keys",
        "--colors",
        "--tree",
        "--layout",
        "--signals",
        "--focus",
        "--preview",
    ];
    let pids: Vec<i32> = statuses
        .iter()
        .enumerate()
        .map(|(index, _)| 4_000 + index as i32 * 17)
        .collect();

    statuses
        .into_iter()
        .enumerate()
        .map(|(index, status)| {
            let (ppid, ancestor_chain) = parentage(index, &pids);
            let user = Arc::<str>::from(users[rng.next_index(users.len())]);
            let name = format!("{}-{}", names[index % names.len()], status_label(&status));
            let cmd = format!(
                "/usr/bin/{} {} --slot={}",
                name,
                flags[rng.next_index(flags.len())],
                rng.next_in_range(1, 10)
            );

            ProcRow {
                pid: pids[index],
                start_time: 10_000 + index as u64,
                ppid,
                ancestor_chain,
                user,
                status,
                cpu_usage_tenths: rng.next_in_range(0, 999) as u32,
                memory_bytes: rng.next_in_range(32_768, 536_870_912),
                name,
                cmd,
            }
        })
        .collect()
}

fn debug_signal_sender(_: i32, _: nix::sys::signal::Signal) -> std::result::Result<(), String> {
    Err("debug tui: signal suppressed".to_string())
}

fn debug_statuses() -> Vec<ProcessStatus> {
    vec![
        ProcessStatus::Run,
        ProcessStatus::Sleep,
        ProcessStatus::Idle,
        ProcessStatus::Waking,
        ProcessStatus::Parked,
        ProcessStatus::Suspended,
        ProcessStatus::Stop,
        ProcessStatus::Tracing,
        ProcessStatus::UninterruptibleDiskSleep,
        ProcessStatus::LockBlocked,
        ProcessStatus::Wakekill,
        ProcessStatus::Zombie,
        ProcessStatus::Dead,
        ProcessStatus::Unknown(99),
    ]
}

fn parentage(index: usize, pids: &[i32]) -> (Option<i32>, Vec<i32>) {
    let mut ancestors = Vec::new();
    let mut current = parent_index(index);
    while let Some(parent_idx) = current {
        ancestors.push(pids[parent_idx]);
        current = parent_index(parent_idx);
    }

    (ancestors.first().copied(), ancestors)
}

fn parent_index(index: usize) -> Option<usize> {
    match index {
        0 | 1 | 4 | 8 | 12 => None,
        2 | 3 => Some(1),
        5..=7 => Some(4),
        9..=11 => Some(8),
        _ => Some(index - 1),
    }
}

fn status_label(status: &ProcessStatus) -> &'static str {
    match status {
        ProcessStatus::Run => "run",
        ProcessStatus::Sleep => "sleep",
        ProcessStatus::Idle => "idle",
        ProcessStatus::Waking => "waking",
        ProcessStatus::Parked => "parked",
        ProcessStatus::Suspended => "suspended",
        ProcessStatus::Stop => "stop",
        ProcessStatus::Tracing => "tracing",
        ProcessStatus::UninterruptibleDiskSleep => "disk",
        ProcessStatus::LockBlocked => "blocked",
        ProcessStatus::Wakekill => "wakekill",
        ProcessStatus::Zombie => "zombie",
        ProcessStatus::Dead => "dead",
        ProcessStatus::Unknown(_) => "unknown",
    }
}

fn initial_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0x5eed_u64)
}

fn next_seed(seed: u64) -> u64 {
    seed.wrapping_mul(6364136223846793005).wrapping_add(1)
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(mut terminal: Terminal<CrosstermBackend<std::io::Stdout>>) {
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();
}

struct DebugRng {
    state: u64,
}

impl DebugRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = next_seed(self.state);
        self.state
    }

    fn next_index(&mut self, len: usize) -> usize {
        (self.next_u64() % len as u64) as usize
    }

    fn next_in_range(&mut self, start: u64, end: u64) -> u64 {
        start + (self.next_u64() % (end - start))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_DEBUG_ROWS, build_debug_rows, debug_signal_sender, debug_statuses, status_label,
    };
    use nix::sys::signal::Signal;
    use std::collections::HashSet;
    use sysinfo::ProcessStatus;

    #[test]
    fn build_debug_rows_stays_within_visual_limit() {
        let rows = build_debug_rows(123);
        assert!(!rows.is_empty());
        assert!(rows.len() <= MAX_DEBUG_ROWS);
    }

    #[test]
    fn build_debug_rows_contains_every_status_variant() {
        let rows = build_debug_rows(456);
        let actual: HashSet<String> = rows
            .iter()
            .map(|row| status_label(&row.status).to_string())
            .collect();
        let expected: HashSet<String> = debug_statuses()
            .into_iter()
            .map(|status| status_label(&status).to_string())
            .collect();

        assert_eq!(actual, expected);
    }

    #[test]
    fn status_label_maps_unknown_status() {
        assert_eq!(status_label(&ProcessStatus::Unknown(7)), "unknown");
    }

    #[test]
    fn build_debug_rows_keeps_identities_stable_across_refreshes() {
        let first = build_debug_rows(123);
        let second = build_debug_rows(456);

        assert_eq!(first.len(), second.len());
        for (first_row, second_row) in first.iter().zip(second.iter()) {
            assert_eq!(first_row.pid, second_row.pid);
            assert_eq!(first_row.start_time, second_row.start_time);
            assert_eq!(first_row.status, second_row.status);
        }
    }

    #[test]
    fn debug_signal_sender_never_dispatches_real_signal() {
        let result = debug_signal_sender(std::process::id() as i32, Signal::SIGKILL);
        assert_eq!(result, Err("debug tui: signal suppressed".to_string()));
    }
}
