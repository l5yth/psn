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

//! Terminal lifecycle: entering and leaving TUI mode.
//!
//! Split from `runtime.rs` so the *ordering* and *error policy* of the
//! lifecycle can be tested without a TTY. The policy lives in [`enter`] and
//! [`leave`], which operate on the [`TerminalOps`] trait; the only code that
//! touches a real terminal is [`CrosstermOps`] and the two thin wrappers
//! [`setup`] and [`restore`].

use std::io;

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, prelude::CrosstermBackend};

/// The concrete terminal the TUI draws into.
pub type Tui = Terminal<CrosstermBackend<io::Stdout>>;

/// The terminal mode changes the TUI lifecycle performs.
///
/// Exists so [`enter`] and [`leave`] can be exercised against a fake. Each
/// method maps to one crossterm call; implementations are expected to be thin.
pub trait TerminalOps {
    /// Put the terminal into raw mode, so keys arrive unbuffered and unechoed.
    fn enable_raw(&mut self) -> Result<()>;
    /// Return the terminal to cooked mode.
    fn disable_raw(&mut self) -> Result<()>;
    /// Switch to the alternate screen, preserving the user's scrollback.
    fn enter_alternate(&mut self) -> Result<()>;
    /// Switch back to the primary screen.
    fn leave_alternate(&mut self) -> Result<()>;
    /// Make the cursor visible again.
    fn show_cursor(&mut self) -> Result<()>;
}

/// Enter TUI mode: raw mode first, then the alternate screen.
///
/// The order matters on the failure path. Raw mode is enabled first because it
/// is the change that makes a terminal unusable if it is left behind; if
/// switching to the alternate screen then fails, this undoes raw mode before
/// returning, so a failed startup never strands the user's shell in raw mode.
/// The undo is best-effort: the original error is what the caller needs to
/// see, so a failure to restore is deliberately discarded.
pub fn enter(ops: &mut dyn TerminalOps) -> Result<()> {
    ops.enable_raw()?;
    if let Err(err) = ops.enter_alternate() {
        let _ = ops.disable_raw();
        return Err(err);
    }
    Ok(())
}

/// Leave TUI mode, undoing [`enter`] in reverse and ignoring failures.
///
/// Every step is attempted even if an earlier one fails: this runs while the
/// program is already on its way out, often on an error path, and a terminal
/// left in raw mode is worse than any error this could report. There is
/// nothing useful a caller could do with a failure here, so none is returned.
pub fn leave(ops: &mut dyn TerminalOps) {
    let _ = ops.disable_raw();
    let _ = ops.leave_alternate();
    let _ = ops.show_cursor();
}

/// [`TerminalOps`] backed by the real terminal on stdout.
pub struct CrosstermOps<'a> {
    terminal: &'a mut Tui,
}

impl TerminalOps for CrosstermOps<'_> {
    fn enable_raw(&mut self) -> Result<()> {
        enable_raw_mode()?;
        Ok(())
    }

    fn disable_raw(&mut self) -> Result<()> {
        disable_raw_mode()?;
        Ok(())
    }

    fn enter_alternate(&mut self) -> Result<()> {
        execute!(self.terminal.backend_mut(), EnterAlternateScreen)?;
        Ok(())
    }

    fn leave_alternate(&mut self) -> Result<()> {
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<()> {
        self.terminal.show_cursor()?;
        Ok(())
    }
}

/// Build a live terminal and put it into TUI mode.
pub fn setup() -> Result<Tui> {
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    enter(&mut CrosstermOps {
        terminal: &mut terminal,
    })?;
    Ok(terminal)
}

/// Take a live terminal out of TUI mode and drop it.
pub fn restore(mut terminal: Tui) {
    leave(&mut CrosstermOps {
        terminal: &mut terminal,
    });
}

#[cfg(test)]
mod tests {
    use super::{TerminalOps, enter, leave};
    use anyhow::{Result, anyhow};

    /// Records the calls made against it and fails the steps it is told to.
    #[derive(Default)]
    struct FakeOps {
        calls: Vec<&'static str>,
        fail_enable_raw: bool,
        fail_enter_alternate: bool,
        fail_disable_raw: bool,
        fail_leave_alternate: bool,
        fail_show_cursor: bool,
    }

    impl FakeOps {
        fn step(&mut self, name: &'static str, fail: bool) -> Result<()> {
            self.calls.push(name);
            if fail {
                return Err(anyhow!("{name} failed"));
            }
            Ok(())
        }
    }

    impl TerminalOps for FakeOps {
        fn enable_raw(&mut self) -> Result<()> {
            self.step("enable_raw", self.fail_enable_raw)
        }
        fn disable_raw(&mut self) -> Result<()> {
            self.step("disable_raw", self.fail_disable_raw)
        }
        fn enter_alternate(&mut self) -> Result<()> {
            self.step("enter_alternate", self.fail_enter_alternate)
        }
        fn leave_alternate(&mut self) -> Result<()> {
            self.step("leave_alternate", self.fail_leave_alternate)
        }
        fn show_cursor(&mut self) -> Result<()> {
            self.step("show_cursor", self.fail_show_cursor)
        }
    }

    #[test]
    fn enter_enables_raw_mode_before_the_alternate_screen() {
        let mut ops = FakeOps::default();

        assert!(enter(&mut ops).is_ok());

        assert_eq!(ops.calls, vec!["enable_raw", "enter_alternate"]);
    }

    #[test]
    fn enter_propagates_a_raw_mode_failure_without_touching_the_screen() {
        let mut ops = FakeOps {
            fail_enable_raw: true,
            ..FakeOps::default()
        };

        let err = enter(&mut ops).expect_err("raw mode failure must propagate");

        assert!(err.to_string().contains("enable_raw"));
        assert_eq!(ops.calls, vec!["enable_raw"]);
    }

    #[test]
    fn enter_undoes_raw_mode_when_the_alternate_screen_fails() {
        // The point of the rollback: a half-entered TUI must not leave the
        // user's shell in raw mode after psn exits with an error.
        let mut ops = FakeOps {
            fail_enter_alternate: true,
            ..FakeOps::default()
        };

        let err = enter(&mut ops).expect_err("alternate screen failure must propagate");

        assert!(err.to_string().contains("enter_alternate"));
        assert_eq!(
            ops.calls,
            vec!["enable_raw", "enter_alternate", "disable_raw"]
        );
    }

    #[test]
    fn enter_reports_the_original_error_even_if_the_rollback_also_fails() {
        let mut ops = FakeOps {
            fail_enter_alternate: true,
            fail_disable_raw: true,
            ..FakeOps::default()
        };

        let err = enter(&mut ops).expect_err("alternate screen failure must propagate");

        // The rollback failure is swallowed; the caller sees the real cause.
        assert!(err.to_string().contains("enter_alternate"));
        assert_eq!(
            ops.calls,
            vec!["enable_raw", "enter_alternate", "disable_raw"]
        );
    }

    #[test]
    fn leave_undoes_the_lifecycle_in_reverse() {
        let mut ops = FakeOps::default();

        leave(&mut ops);

        assert_eq!(
            ops.calls,
            vec!["disable_raw", "leave_alternate", "show_cursor"]
        );
    }

    #[test]
    fn leave_attempts_every_step_even_when_all_of_them_fail() {
        // Teardown runs while the program is already exiting, so one failing
        // step must not skip the rest: a terminal stuck in raw mode is worse
        // than any error this could report.
        let mut ops = FakeOps {
            fail_disable_raw: true,
            fail_leave_alternate: true,
            fail_show_cursor: true,
            ..FakeOps::default()
        };

        leave(&mut ops);

        assert_eq!(
            ops.calls,
            vec!["disable_raw", "leave_alternate", "show_cursor"]
        );
    }
}
