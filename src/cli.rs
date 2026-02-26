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

//! Command-line parsing and help/version text.

use anyhow::{Result, anyhow, bail};

/// Parsed command mode for process execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    /// Run the TUI with an optional process filter.
    Run { filter: Option<String> },
    /// Print usage instructions and exit.
    Help,
    /// Print version text and exit.
    Version,
}

/// Parse CLI arguments from an iterator (including argv0 as first item).
pub fn parse_args<I>(args: I) -> Result<CliCommand>
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);
    let _argv0 = args.next();

    let mut positionals: Vec<String> = Vec::new();
    let mut wants_help = false;
    let mut wants_version = false;

    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => wants_help = true,
            "-v" | "--version" => wants_version = true,
            _ if arg.starts_with('-') => bail!("unknown option: {arg}"),
            _ => positionals.push(arg),
        }
    }

    if wants_help && wants_version {
        return Err(anyhow!("cannot combine --help and --version"));
    }

    if wants_help {
        if !positionals.is_empty() {
            bail!("--help does not accept FILTER");
        }
        return Ok(CliCommand::Help);
    }

    if wants_version {
        if !positionals.is_empty() {
            bail!("--version does not accept FILTER");
        }
        return Ok(CliCommand::Version);
    }

    match positionals.len() {
        0 => Ok(CliCommand::Run { filter: None }),
        1 => Ok(CliCommand::Run {
            filter: Some(positionals.remove(0)),
        }),
        _ => bail!("too many positional arguments; expected at most one FILTER"),
    }
}

/// Render help text.
pub fn help_text() -> String {
    [
        &version_text(),
        "",
        "psn [OPTIONS] [FILTER]",
        "",
        "Terminal UI for browsing process status and sending Unix signals.",
        "",
        "Options:",
        "  -h, --help     Show usage instructions",
        "  -v, --version  Show version",
    ]
    .join("\n")
}

/// Render version text.
pub fn version_text() -> String {
    format!(
        "psn v{}\nprocess status navigator\napache v2 (c) 2026 l5yth",
        env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    use super::{CliCommand, help_text, parse_args, version_text};

    #[test]
    fn parse_args_no_args_runs_without_filter() {
        let cmd = parse_args(["psn"]).expect("parse should succeed");
        assert_eq!(cmd, CliCommand::Run { filter: None });
    }

    #[test]
    fn parse_args_single_filter_runs_with_filter() {
        let cmd = parse_args(["psn", "sshd"]).expect("parse should succeed");
        assert_eq!(
            cmd,
            CliCommand::Run {
                filter: Some("sshd".to_string())
            }
        );
    }

    #[test]
    fn parse_args_help_and_version_work() {
        assert_eq!(
            parse_args(["psn", "-h"]).expect("parse should succeed"),
            CliCommand::Help
        );
        assert_eq!(
            parse_args(["psn", "--help"]).expect("parse should succeed"),
            CliCommand::Help
        );
        assert_eq!(
            parse_args(["psn", "-v"]).expect("parse should succeed"),
            CliCommand::Version
        );
        assert_eq!(
            parse_args(["psn", "--version"]).expect("parse should succeed"),
            CliCommand::Version
        );
    }

    #[test]
    fn parse_args_unknown_flags_are_not_treated_as_filter() {
        assert!(parse_args(["psn", "-x"]).is_err());
        assert!(parse_args(["psn", "--wat"]).is_err());
    }

    #[test]
    fn parse_args_rejects_too_many_positionals() {
        assert!(parse_args(["psn", "a", "b"]).is_err());
    }

    #[test]
    fn help_text_contains_usage() {
        let text = help_text();
        assert!(text.contains("psn [OPTIONS] [FILTER]"));
        assert!(text.contains("--help"));
        assert!(text.contains("--version"));
    }

    #[test]
    fn version_text_contains_requested_lines() {
        let text = version_text();
        assert!(text.contains("psn v"));
        assert!(text.contains("process status navigator"));
        assert!(text.contains("apache v2 (c) 2026 l5yth"));
    }
}
