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
    Run {
        filter: Option<String>,
        regex_mode: bool,
        user_only: bool,
    },
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
    let mut args: Vec<String> = args.into_iter().map(Into::into).collect();
    if !args.is_empty() {
        args.remove(0);
    }

    let mut positionals: Vec<String> = Vec::new();
    let mut filter_from_option: Option<String> = None;
    let mut regex_from_option: Option<String> = None;
    let mut wants_help = false;
    let mut wants_version = false;
    let mut user_only = false;
    let mut saw_valid_option = false;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--" => {
                positionals.extend(iter);
                break;
            }
            "-h" | "--help" => {
                wants_help = true;
                saw_valid_option = true;
            }
            "-v" | "--version" => {
                wants_version = true;
                saw_valid_option = true;
            }
            "-r" | "--regex" => {
                saw_valid_option = true;
                if filter_from_option.is_some() || regex_from_option.is_some() {
                    bail!("cannot combine --regex with -f/--filter");
                }
                let value = iter
                    .next()
                    .ok_or_else(|| anyhow!("{arg} requires a PATTERN value"))?;
                if value.is_empty() {
                    bail!("{arg} requires a non-empty PATTERN value");
                }
                regex_from_option = Some(value);
            }
            "-u" | "--user" => {
                user_only = true;
                saw_valid_option = true;
            }
            "-f" | "--filter" => {
                saw_valid_option = true;
                let value = iter
                    .next()
                    .ok_or_else(|| anyhow!("{arg} requires a FILTER value"))?;
                if filter_from_option.is_some() {
                    bail!("FILTER specified multiple times");
                }
                if regex_from_option.is_some() {
                    bail!("cannot combine --regex with -f/--filter");
                }
                if value.is_empty() {
                    bail!("{arg} requires a non-empty FILTER value");
                }
                filter_from_option = Some(value);
            }
            _ if arg.starts_with('-') => {
                if arg.starts_with("--") {
                    bail!("unknown option: {arg}");
                }
                if saw_valid_option {
                    bail!("unknown option: {arg}");
                }
                if !positionals.is_empty() || filter_from_option.is_some() {
                    bail!("unknown option: {arg}");
                }
                positionals.push(arg);
            }
            _ => positionals.push(arg),
        }
    }

    if filter_from_option.is_some() && !positionals.is_empty() {
        bail!("cannot combine positional FILTER with -f/--filter");
    }

    if wants_help && wants_version {
        return Err(anyhow!("cannot combine --help and --version"));
    }

    if wants_help {
        if !positionals.is_empty()
            || filter_from_option.is_some()
            || regex_from_option.is_some()
            || user_only
        {
            bail!("--help does not accept FILTER");
        }
        return Ok(CliCommand::Help);
    }

    if wants_version {
        if !positionals.is_empty()
            || filter_from_option.is_some()
            || regex_from_option.is_some()
            || user_only
        {
            bail!("--version does not accept FILTER");
        }
        return Ok(CliCommand::Version);
    }

    if let Some(filter) = filter_from_option {
        return Ok(CliCommand::Run {
            filter: Some(filter),
            regex_mode: false,
            user_only,
        });
    }

    if let Some(pattern) = regex_from_option {
        if !positionals.is_empty() {
            bail!("too many PATTERN arguments for --regex");
        }
        return Ok(CliCommand::Run {
            filter: Some(pattern),
            regex_mode: true,
            user_only,
        });
    }

    if saw_valid_option && !positionals.is_empty() {
        bail!("when using options, pass FILTER with -f or --filter");
    }

    match positionals.as_slice() {
        [] => Ok(CliCommand::Run {
            filter: None,
            regex_mode: false,
            user_only,
        }),
        [filter] => Ok(CliCommand::Run {
            filter: Some(filter.clone()),
            regex_mode: false,
            user_only,
        }),
        _ => bail!("too many positional arguments; expected at most one FILTER"),
    }
}

/// Render help text.
pub fn help_text() -> String {
    [
        &version_text(),
        "",
        "usage: psn <FILTER>",
        "usage: psn [OPTIONS] -f <FILTER>",
        "usage: psn [OPTIONS] -r <PATTERN>",
        "",
        "Terminal UI for browsing process status and sending Unix signals.",
        "",
        "Options:",
        "  -h, --help            Show usage instructions",
        "  -v, --version         Show version",
        "  -f, --filter <value>  Filter process names/commands (case insensitive string)",
        "  -r, --regex <value>   Use regex matching (regular expression pattern)",
        "  -u, --user            Show only current user's processes",
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
        assert_eq!(
            cmd,
            CliCommand::Run {
                filter: None,
                regex_mode: false,
                user_only: false
            }
        );
    }

    #[test]
    fn parse_args_single_filter_runs_with_filter() {
        let cmd = parse_args(["psn", "sshd"]).expect("parse should succeed");
        assert_eq!(
            cmd,
            CliCommand::Run {
                filter: Some("sshd".to_string()),
                regex_mode: false,
                user_only: false
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
    fn parse_args_dash_prefixed_filter_without_options_is_allowed() {
        let cmd = parse_args(["psn", "-bash"]).expect("parse should succeed");
        assert_eq!(
            cmd,
            CliCommand::Run {
                filter: Some("-bash".to_string()),
                regex_mode: false,
                user_only: false
            }
        );
    }

    #[test]
    fn parse_args_filter_flag_works() {
        let short = parse_args(["psn", "-f", "sshd"]).expect("parse should succeed");
        let long = parse_args(["psn", "--filter", "sshd"]).expect("parse should succeed");

        assert_eq!(
            short,
            CliCommand::Run {
                filter: Some("sshd".to_string()),
                regex_mode: false,
                user_only: false
            }
        );
        assert_eq!(
            long,
            CliCommand::Run {
                filter: Some("sshd".to_string()),
                regex_mode: false,
                user_only: false
            }
        );
    }

    #[test]
    fn parse_args_regex_option_works_without_filter_flag() {
        let short = parse_args(["psn", "-r", "sshd.*"]).expect("parse should succeed");
        let long = parse_args(["psn", "--regex", "sshd.*"]).expect("parse should succeed");

        assert_eq!(
            short,
            CliCommand::Run {
                filter: Some("sshd.*".to_string()),
                regex_mode: true,
                user_only: false
            }
        );
        assert_eq!(
            long,
            CliCommand::Run {
                filter: Some("sshd.*".to_string()),
                regex_mode: true,
                user_only: false
            }
        );
    }

    #[test]
    fn parse_args_regex_requires_pattern() {
        assert!(parse_args(["psn", "-r"]).is_err());
        assert!(parse_args(["psn", "--regex"]).is_err());
        assert!(parse_args(["psn", "-r", ""]).is_err());
    }

    #[test]
    fn parse_args_user_flag_works() {
        let cmd = parse_args(["psn", "-u"]).expect("parse should succeed");
        assert_eq!(
            cmd,
            CliCommand::Run {
                filter: None,
                regex_mode: false,
                user_only: true
            }
        );
    }

    #[test]
    fn parse_args_unknown_flag_errors_after_known_option() {
        assert!(parse_args(["psn", "-h", "--wat"]).is_err());
        assert!(parse_args(["psn", "-f", "sshd", "--wat"]).is_err());
        assert!(parse_args(["psn", "-u", "-wat"]).is_err());
    }

    #[test]
    fn parse_args_unknown_long_option_errors() {
        assert!(parse_args(["psn", "--wat"]).is_err());
    }

    #[test]
    fn parse_args_unknown_short_before_options_is_filter() {
        let cmd = parse_args(["psn", "-wat"]).expect("parse should succeed");
        assert_eq!(
            cmd,
            CliCommand::Run {
                filter: Some("-wat".to_string()),
                regex_mode: false,
                user_only: false
            }
        );
    }

    #[test]
    fn parse_args_rejects_positional_filter_when_options_present() {
        assert!(parse_args(["psn", "-v", "sshd"]).is_err());
        assert!(parse_args(["psn", "--help", "sshd"]).is_err());
        assert!(parse_args(["psn", "-f", "sshd", "bash"]).is_err());
        assert!(parse_args(["psn", "-r", "x", "y"]).is_err());
    }

    #[test]
    fn parse_args_rejects_missing_filter_value_for_flag() {
        assert!(parse_args(["psn", "-f"]).is_err());
        assert!(parse_args(["psn", "--filter"]).is_err());
    }

    #[test]
    fn parse_args_rejects_empty_filter_value_for_flag() {
        assert!(parse_args(["psn", "-f", ""]).is_err());
        assert!(parse_args(["psn", "--filter", ""]).is_err());
    }

    #[test]
    fn parse_args_rejects_combined_filter_and_regex() {
        assert!(parse_args(["psn", "-f", "ssh", "-r", "ssh.*"]).is_err());
        assert!(parse_args(["psn", "-r", "ssh.*", "-f", "ssh"]).is_err());
    }

    #[test]
    fn parse_args_rejects_duplicate_filter_option() {
        assert!(parse_args(["psn", "-f", "a", "--filter", "b"]).is_err());
    }

    #[test]
    fn parse_args_user_with_filter_and_regex_works() {
        assert_eq!(
            parse_args(["psn", "-u", "-f", "ssh"]).expect("parse should succeed"),
            CliCommand::Run {
                filter: Some("ssh".to_string()),
                regex_mode: false,
                user_only: true
            }
        );
        assert_eq!(
            parse_args(["psn", "-u", "-r", "^ssh(d|agent)$"]).expect("parse should succeed"),
            CliCommand::Run {
                filter: Some("^ssh(d|agent)$".to_string()),
                regex_mode: true,
                user_only: true
            }
        );
    }

    #[test]
    fn parse_args_rejects_help_and_version_together() {
        assert!(parse_args(["psn", "--help", "--version"]).is_err());
    }

    #[test]
    fn parse_args_rejects_short_unknown_after_short_unknown_filter() {
        assert!(parse_args(["psn", "-x", "-y"]).is_err());
    }

    #[test]
    fn parse_args_rejects_positional_when_user_option_present() {
        assert!(parse_args(["psn", "-u", "ssh"]).is_err());
    }

    #[test]
    fn parse_args_option_terminator_allows_dash_prefixed_filter() {
        let cmd = parse_args(["psn", "--", "--wat"]).expect("parse should succeed");
        assert_eq!(
            cmd,
            CliCommand::Run {
                filter: Some("--wat".to_string()),
                regex_mode: false,
                user_only: false
            }
        );
    }

    #[test]
    fn parse_args_rejects_too_many_positionals() {
        assert!(parse_args(["psn", "a", "b"]).is_err());
    }

    #[test]
    fn help_text_contains_usage() {
        let text = help_text();
        assert!(text.contains("usage: psn <FILTER>"));
        assert!(text.contains("usage: psn [OPTIONS] -f <FILTER>"));
        assert!(text.contains("usage: psn [OPTIONS] -r <PATTERN>"));
        assert!(text.contains("--help"));
        assert!(text.contains("--version"));
        assert!(text.contains("--filter"));
        assert!(text.contains("--regex"));
        assert!(text.contains("--user"));
    }

    #[test]
    fn version_text_contains_requested_lines() {
        let text = version_text();
        assert!(text.contains("psn v"));
        assert!(text.contains("process status navigator"));
        assert!(text.contains("apache v2 (c) 2026 l5yth"));
    }
}
