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
mod terminal;
pub mod tree;
pub mod ui;

#[cfg(all(feature = "debug_tui", debug_assertions))]
mod debug_tui;

use crate::runtime::run_interactive;
use anyhow::Result;

/// Run the interactive TUI application.
pub fn run(filter: Option<String>, regex_mode: bool, user_only: bool) -> Result<()> {
    let compiled_filter = process::compile_filter(filter.clone(), regex_mode)?;
    run_interactive(filter, compiled_filter, user_only)
}

/// Run the hidden synthetic-data TUI used for local development.
#[cfg(all(feature = "debug_tui", debug_assertions))]
#[doc(hidden)]
pub fn run_debug_tui() -> Result<()> {
    debug_tui::run()
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn run_returns_error_for_invalid_regex_before_terminal_setup() {
        let result = run(Some("(".to_string()), true, false);
        assert!(result.is_err());
    }

    #[cfg(all(feature = "debug_tui", debug_assertions))]
    #[test]
    fn run_debug_tui_symbol_is_available_when_feature_is_enabled() {
        let debug_runner: fn() -> anyhow::Result<()> = super::run_debug_tui;
        let _ = debug_runner;
    }
}
