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

use anyhow::Result;
use psn::cli::{CliCommand, help_text, parse_args, version_text};

/// Binary entry point. Delegates to CLI parsing and runtime.
fn main() -> Result<()> {
    dispatch_command(parse_args(std::env::args())?, &mut psn::run)
}

/// Execute a parsed CLI command with an injected runtime runner.
fn dispatch_command(
    command: CliCommand,
    runner: &mut dyn FnMut(Option<String>, bool, bool) -> Result<()>,
) -> Result<()> {
    match command {
        CliCommand::Help => {
            println!("{}", help_text());
            Ok(())
        }
        CliCommand::Version => {
            println!("{}", version_text());
            Ok(())
        }
        CliCommand::Run {
            filter,
            regex_mode,
            user_only,
        } => runner(filter, regex_mode, user_only),
    }
}

#[cfg(test)]
mod tests {
    use super::dispatch_command;
    use anyhow::Result;
    use psn::cli::CliCommand;

    fn no_op_runner(_: Option<String>, _: bool, _: bool) -> Result<()> {
        Ok(())
    }

    #[test]
    fn dispatch_command_help_returns_ok() {
        let mut runner = no_op_runner;
        assert!(dispatch_command(CliCommand::Help, &mut runner).is_ok());
    }

    #[test]
    fn dispatch_command_version_returns_ok() {
        let mut runner = no_op_runner;
        assert!(dispatch_command(CliCommand::Version, &mut runner).is_ok());
    }

    #[test]
    fn dispatch_command_run_calls_runner_with_expected_values() {
        let mut called = false;
        let mut runner =
            |filter: Option<String>, regex_mode: bool, user_only: bool| -> Result<()> {
                called = true;
                assert_eq!(filter.as_deref(), Some("ssh"));
                assert!(regex_mode);
                assert!(user_only);
                Ok(())
            };

        let command = CliCommand::Run {
            filter: Some("ssh".to_string()),
            regex_mode: true,
            user_only: true,
        };

        assert!(dispatch_command(command, &mut runner).is_ok());
        assert!(called);
    }

    #[test]
    fn dispatch_command_run_works_with_no_op_runner() {
        let mut runner = no_op_runner;
        let command = CliCommand::Run {
            filter: None,
            regex_mode: false,
            user_only: false,
        };
        assert!(dispatch_command(command, &mut runner).is_ok());
    }
}
