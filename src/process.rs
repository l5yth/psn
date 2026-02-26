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

//! Process collection, mapping, filtering, and ordering.

use std::{ffi::OsString, path::Path};

use anyhow::{Result, bail};
use ratatui::style::Color;
use regex::RegexBuilder;
use sysinfo::{ProcessStatus, ProcessesToUpdate, System, Uid};

use crate::model::ProcRow;

/// Maximum allowed regex pattern length for `--regex`.
pub const MAX_REGEX_PATTERN_LEN: usize = 256;

/// Compiled filtering mode used during process row matching.
#[derive(Debug, Clone)]
pub enum FilterSpec {
    /// Case-insensitive substring filter.
    Substring(String),
    /// Case-insensitive compiled regular expression.
    Regex(regex::Regex),
}

/// Build a compiled filter from raw CLI filter input.
pub fn compile_filter(filter: Option<String>, regex_mode: bool) -> Result<Option<FilterSpec>> {
    let Some(filter_text) = filter else {
        return Ok(None);
    };

    if regex_mode {
        if filter_text.len() > MAX_REGEX_PATTERN_LEN {
            bail!(
                "regex pattern too long (max {} chars)",
                MAX_REGEX_PATTERN_LEN
            );
        }
        let regex = RegexBuilder::new(&filter_text)
            .case_insensitive(true)
            .build()?;
        return Ok(Some(FilterSpec::Regex(regex)));
    }

    Ok(Some(FilterSpec::Substring(filter_text)))
}

/// Resolve a sysinfo uid to a displayable user string.
pub fn to_user(uid: Option<&Uid>) -> String {
    if let Some(uid_value) = uid {
        let uid_text = uid_value.to_string();
        if let Ok(uid_num) = uid_text.parse::<u32>()
            && let Some(user) = users::get_user_by_uid(uid_num)
        {
            return user.name().to_string_lossy().to_string();
        }
        return uid_text;
    }

    "?".to_string()
}

/// Build command text from command parts with executable-path fallback.
pub fn build_cmd(cmd_parts: &[OsString], exe_path: Option<&Path>) -> String {
    if cmd_parts.is_empty() {
        exe_path
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default()
    } else {
        cmd_parts
            .iter()
            .map(|part| part.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Return whether a row matches an optional case-insensitive filter.
pub fn matches_filter(row: &ProcRow, filter: Option<&FilterSpec>) -> bool {
    match filter {
        None => true,
        Some(FilterSpec::Substring(raw_filter)) => {
            let lowered = raw_filter.to_lowercase();
            row.name.to_lowercase().contains(&lowered) || row.cmd.to_lowercase().contains(&lowered)
        }
        Some(FilterSpec::Regex(regex)) => regex.is_match(&row.name) || regex.is_match(&row.cmd),
    }
}

/// Priority rank used for stable row ordering by status.
pub fn status_priority(status: ProcessStatus) -> u8 {
    match status {
        ProcessStatus::Run => 0,
        ProcessStatus::Sleep => 1,
        ProcessStatus::Idle => 2,
        ProcessStatus::Waking => 3,
        ProcessStatus::Parked => 4,
        ProcessStatus::Suspended => 5,
        ProcessStatus::Stop => 6,
        ProcessStatus::Tracing => 7,
        ProcessStatus::UninterruptibleDiskSleep => 8,
        ProcessStatus::LockBlocked => 9,
        ProcessStatus::Wakekill => 10,
        ProcessStatus::Zombie => 11,
        ProcessStatus::Dead => 12,
        ProcessStatus::Unknown(_) => 13,
    }
}

/// Determine table dot color for each process status.
pub fn status_dot_color(status: ProcessStatus) -> Color {
    match status {
        ProcessStatus::Run => Color::Green,
        ProcessStatus::Sleep | ProcessStatus::Idle => Color::Yellow,
        ProcessStatus::Stop
        | ProcessStatus::Tracing
        | ProcessStatus::Zombie
        | ProcessStatus::Dead
        | ProcessStatus::Wakekill => Color::Red,
        _ => Color::DarkGray,
    }
}

/// Sort rows by status priority, process name, and pid.
pub fn sort_rows(rows: &mut [ProcRow]) {
    rows.sort_by(|a, b| {
        status_priority(a.status)
            .cmp(&status_priority(b.status))
            .then(a.name.cmp(&b.name))
            .then(a.pid.cmp(&b.pid))
    });
}

/// Refresh process rows from sysinfo and apply optional filtering.
pub fn refresh_rows(
    sys: &mut System,
    filter: Option<&FilterSpec>,
    user_only: bool,
) -> Vec<ProcRow> {
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.refresh_cpu_all();
    sys.refresh_memory();

    let current_uid = if user_only {
        users::get_current_uid().to_string().parse::<Uid>().ok()
    } else {
        None
    };

    let mut rows: Vec<ProcRow> = sys
        .processes()
        .values()
        .filter(|process| {
            if let Some(uid) = current_uid.as_ref() {
                process.user_id() == Some(uid)
            } else {
                true
            }
        })
        .map(|process| {
            let pid = process.pid().as_u32() as i32;
            let user = to_user(process.user_id());
            let status = process.status();
            let name = process.name().to_string_lossy().to_string();
            let cmd = build_cmd(process.cmd(), process.exe());

            ProcRow {
                pid,
                user,
                status,
                name,
                cmd,
            }
        })
        .filter(|row| matches_filter(row, filter))
        .collect();

    sort_rows(&mut rows);
    rows
}

#[cfg(test)]
mod tests {
    use super::{
        FilterSpec, MAX_REGEX_PATTERN_LEN, build_cmd, compile_filter, matches_filter, refresh_rows,
        sort_rows, status_dot_color, status_priority, to_user,
    };
    use crate::model::ProcRow;
    use ratatui::style::Color;
    use std::{ffi::OsString, path::Path};
    use sysinfo::{ProcessStatus, System, Uid};

    fn row(pid: i32, name: &str, status: ProcessStatus, cmd: &str) -> ProcRow {
        ProcRow {
            pid,
            user: "u".to_string(),
            status,
            name: name.to_string(),
            cmd: cmd.to_string(),
        }
    }

    #[test]
    fn to_user_handles_missing_uid() {
        assert_eq!(to_user(None), "?");
    }

    #[test]
    fn to_user_keeps_uid_text_when_name_not_resolved() {
        let uid: Uid = "4294967295".parse().expect("uid parse must succeed");
        assert_eq!(to_user(Some(&uid)), uid.to_string());
    }

    #[test]
    fn build_cmd_uses_exe_fallback_for_empty_command_parts() {
        let result = build_cmd(&[], Some(Path::new("/usr/bin/psn")));
        assert_eq!(result, "/usr/bin/psn");
    }

    #[test]
    fn build_cmd_joins_command_parts_when_present() {
        let result = build_cmd(
            &[OsString::from("psn"), OsString::from("--demo")],
            Some(Path::new("/ignored")),
        );
        assert_eq!(result, "psn --demo");
    }

    #[test]
    fn matches_filter_matches_name_or_command_case_insensitive() {
        let r = row(1, "SSHD", ProcessStatus::Run, "/usr/sbin/daemon");
        assert!(matches_filter(&r, None));
        assert!(matches_filter(
            &r,
            Some(&FilterSpec::Substring("ssh".to_string()))
        ));
        assert!(matches_filter(
            &r,
            Some(&FilterSpec::Substring("DAEMON".to_string()))
        ));
        assert!(!matches_filter(
            &r,
            Some(&FilterSpec::Substring("postgres".to_string()))
        ));
    }

    #[test]
    fn matches_filter_supports_regex_mode() {
        let r = row(1, "sshd", ProcessStatus::Run, "/usr/sbin/daemon");
        let re = compile_filter(Some("^ssh.*$".to_string()), true)
            .expect("regex should compile")
            .expect("filter should exist");
        assert!(matches_filter(&r, Some(&re)));
    }

    #[test]
    fn compile_filter_rejects_overly_long_regex() {
        let pattern = "a".repeat(MAX_REGEX_PATTERN_LEN + 1);
        assert!(compile_filter(Some(pattern), true).is_err());
    }

    #[test]
    fn status_priority_covers_all_known_variants() {
        let statuses = [
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
            ProcessStatus::Unknown(1),
        ];

        for (index, status) in statuses.iter().enumerate() {
            assert_eq!(status_priority(*status), index as u8);
        }
    }

    #[test]
    fn status_dot_color_maps_expected_groups() {
        assert_eq!(status_dot_color(ProcessStatus::Run), Color::Green);
        assert_eq!(status_dot_color(ProcessStatus::Sleep), Color::Yellow);
        assert_eq!(status_dot_color(ProcessStatus::Idle), Color::Yellow);
        assert_eq!(status_dot_color(ProcessStatus::Stop), Color::Red);
        assert_eq!(status_dot_color(ProcessStatus::Wakekill), Color::Red);
        assert_eq!(status_dot_color(ProcessStatus::Waking), Color::DarkGray);
    }

    #[test]
    fn sort_rows_uses_status_then_name_then_pid() {
        let mut rows = vec![
            row(30, "bbb", ProcessStatus::Sleep, "c1"),
            row(22, "aaa", ProcessStatus::Run, "c2"),
            row(21, "aaa", ProcessStatus::Run, "c3"),
            row(11, "aaa", ProcessStatus::Zombie, "c4"),
        ];

        sort_rows(&mut rows);

        assert_eq!(rows[0].pid, 21);
        assert_eq!(rows[1].pid, 22);
        assert_eq!(rows[2].pid, 30);
        assert_eq!(rows[3].pid, 11);
    }

    #[test]
    fn refresh_rows_returns_sorted_data() {
        let mut sys = System::new_all();
        let rows = refresh_rows(&mut sys, None, false);

        let mut sorted = rows.clone();
        sort_rows(&mut sorted);
        assert_eq!(rows, sorted);
    }

    #[test]
    fn refresh_rows_applies_filter() {
        let mut sys = System::new_all();
        let filter = FilterSpec::Substring("__psn_filter_that_should_not_exist__".to_string());
        let rows = refresh_rows(&mut sys, Some(&filter), false);
        assert!(rows.is_empty());
    }
}
