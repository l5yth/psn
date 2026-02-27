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

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    ffi::OsString,
    path::Path,
    sync::Arc,
};

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
    Substring {
        raw: String,
        lowered: String,
        ascii_only: bool,
    },
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

    Ok(Some(FilterSpec::Substring {
        lowered: filter_text.to_lowercase(),
        ascii_only: filter_text.is_ascii(),
        raw: filter_text,
    }))
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
        Some(FilterSpec::Substring {
            raw,
            lowered,
            ascii_only,
        }) => {
            contains_case_insensitive(&row.name, raw, lowered, *ascii_only)
                || contains_case_insensitive(&row.cmd, raw, lowered, *ascii_only)
        }
        Some(FilterSpec::Regex(regex)) => regex.is_match(&row.name) || regex.is_match(&row.cmd),
    }
}

/// Check whether `haystack` contains `needle` case-insensitively.
///
/// Uses an ASCII fast-path with zero allocations and falls back to Unicode
/// lowercasing when non-ASCII matching is required.
fn contains_case_insensitive(
    haystack: &str,
    needle_raw: &str,
    needle_lowered: &str,
    ascii_only: bool,
) -> bool {
    if needle_raw.is_empty() {
        return true;
    }

    if ascii_only && haystack.is_ascii() {
        let haystack_bytes = haystack.as_bytes();
        let needle_bytes = needle_raw.as_bytes();
        if needle_bytes.len() > haystack_bytes.len() {
            return false;
        }

        return haystack_bytes
            .windows(needle_bytes.len())
            .any(|window| window.eq_ignore_ascii_case(needle_bytes));
    }

    haystack.to_lowercase().contains(needle_lowered)
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

/// Compare rows by activity, resource usage, and stable identity keys.
pub fn compare_rows(a: &ProcRow, b: &ProcRow) -> Ordering {
    status_priority(a.status)
        .cmp(&status_priority(b.status))
        .then(b.cpu_usage_tenths.cmp(&a.cpu_usage_tenths))
        .then(b.memory_bytes.cmp(&a.memory_bytes))
        .then(a.name.cmp(&b.name))
        .then(a.user.as_ref().cmp(b.user.as_ref()))
        .then(a.pid.cmp(&b.pid))
}

/// Sort rows by status priority, cpu usage, memory usage, name, user, then pid.
pub fn sort_rows(rows: &mut [ProcRow]) {
    rows.sort_by(compare_rows);
}

/// Refresh process rows from sysinfo and apply optional filtering.
///
/// This intentionally refreshes only process data required by the current table
/// columns plus hidden sort keys (pid, name, command, status, user, cpu, memory).
pub fn refresh_rows(
    sys: &mut System,
    filter: Option<&FilterSpec>,
    user_only: bool,
) -> Vec<ProcRow> {
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let current_uid = if user_only {
        users::get_current_uid().to_string().parse::<Uid>().ok()
    } else {
        None
    };
    let pid_to_ppid_all: HashMap<i32, Option<i32>> = sys
        .processes()
        .values()
        .map(|process| {
            (
                process.pid().as_u32() as i32,
                process.parent().map(|value| value.as_u32() as i32),
            )
        })
        .collect();
    let mut user_cache: HashMap<String, Arc<str>> = HashMap::new();

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
            let start_time = process.start_time();
            let ppid = process.parent().map(|value| value.as_u32() as i32);
            let user = resolve_user_cached(process.user_id(), &mut user_cache);
            let status = process.status();
            let cpu_usage_tenths = (process.cpu_usage().max(0.0) * 10.0).round() as u32;
            let memory_bytes = process.memory();
            let name = process.name().to_string_lossy().to_string();
            let cmd = build_cmd(process.cmd(), process.exe());

            ProcRow {
                pid,
                start_time,
                ppid,
                ancestor_chain: build_ancestor_chain(ppid, &pid_to_ppid_all),
                user,
                status,
                cpu_usage_tenths,
                memory_bytes,
                name,
                cmd,
            }
        })
        .filter(|row| matches_filter(row, filter))
        .collect();

    sort_rows(&mut rows);
    rows
}

fn build_ancestor_chain(
    ppid: Option<i32>,
    pid_to_ppid_all: &HashMap<i32, Option<i32>>,
) -> Vec<i32> {
    let mut chain: Vec<i32> = Vec::new();
    let mut seen: HashSet<i32> = HashSet::new();
    let mut current = ppid;

    while let Some(pid) = current {
        if !seen.insert(pid) {
            break;
        }
        chain.push(pid);
        current = pid_to_ppid_all.get(&pid).copied().flatten();
    }

    chain
}

/// Resolve user display text with per-refresh memoization to avoid repeated
/// uid lookups for processes owned by the same user.
fn resolve_user_cached(uid: Option<&Uid>, cache: &mut HashMap<String, Arc<str>>) -> Arc<str> {
    let Some(uid_value) = uid else {
        return Arc::<str>::from("?");
    };

    let uid_key = uid_value.to_string();
    if let Some(cached) = cache.get(&uid_key) {
        return cached.clone();
    }

    let resolved: Arc<str> = Arc::from(to_user(Some(uid_value)));
    cache.insert(uid_key, resolved.clone());
    resolved
}

#[cfg(test)]
mod tests {
    use super::{
        FilterSpec, MAX_REGEX_PATTERN_LEN, build_cmd, compare_rows, compile_filter, matches_filter,
        refresh_rows, resolve_user_cached, sort_rows, status_dot_color, status_priority, to_user,
    };
    use crate::model::ProcRow;
    use ratatui::style::Color;
    use std::{cmp::Ordering, collections::HashMap, sync::Arc};
    use std::{ffi::OsString, path::Path};
    use sysinfo::{ProcessStatus, System, Uid};

    fn row(pid: i32, name: &str, status: ProcessStatus, cmd: &str) -> ProcRow {
        ProcRow {
            pid,
            start_time: 0,
            ppid: None,
            ancestor_chain: Vec::new(),
            user: Arc::from("u"),
            status,
            cpu_usage_tenths: 0,
            memory_bytes: 0,
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
            Some(&FilterSpec::Substring {
                raw: "ssh".to_string(),
                lowered: "ssh".to_string(),
                ascii_only: true,
            })
        ));
        assert!(matches_filter(
            &r,
            Some(&FilterSpec::Substring {
                raw: "DAEMON".to_string(),
                lowered: "daemon".to_string(),
                ascii_only: true,
            })
        ));
        assert!(!matches_filter(
            &r,
            Some(&FilterSpec::Substring {
                raw: "postgres".to_string(),
                lowered: "postgres".to_string(),
                ascii_only: true,
            })
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
    fn compile_filter_accepts_none_and_substring_and_regex() {
        assert!(
            compile_filter(None, false)
                .expect("none should parse")
                .is_none()
        );

        let substring = compile_filter(Some("ssh".to_string()), false)
            .expect("substring should parse")
            .expect("substring filter should exist");
        assert!(matches!(substring, FilterSpec::Substring { .. }));

        let regex = compile_filter(Some("^ssh$".to_string()), true)
            .expect("regex should parse")
            .expect("regex filter should exist");
        assert!(matches!(regex, FilterSpec::Regex(_)));
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
    fn sort_rows_uses_status_then_name_then_pid_when_resource_keys_match() {
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
    fn sort_rows_uses_cpu_memory_name_user_then_pid_tie_breakers() {
        let mut rows = vec![
            ProcRow {
                pid: 42,
                start_time: 1,
                ppid: None,
                ancestor_chain: Vec::new(),
                user: std::sync::Arc::from("z"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 25,
                memory_bytes: 200,
                name: "bbb".to_string(),
                cmd: "z".to_string(),
            },
            ProcRow {
                pid: 50,
                start_time: 2,
                ppid: None,
                ancestor_chain: Vec::new(),
                user: std::sync::Arc::from("a"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 25,
                memory_bytes: 200,
                name: "bbb".to_string(),
                cmd: "x".to_string(),
            },
            ProcRow {
                pid: 60,
                start_time: 3,
                ppid: None,
                ancestor_chain: Vec::new(),
                user: std::sync::Arc::from("a"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 30,
                memory_bytes: 100,
                name: "ccc".to_string(),
                cmd: "y".to_string(),
            },
            ProcRow {
                pid: 40,
                start_time: 4,
                ppid: None,
                ancestor_chain: Vec::new(),
                user: std::sync::Arc::from("a"),
                status: ProcessStatus::Run,
                cpu_usage_tenths: 25,
                memory_bytes: 300,
                name: "ccc".to_string(),
                cmd: "w".to_string(),
            },
        ];

        sort_rows(&mut rows);
        assert_eq!(rows[0].pid, 60);
        assert_eq!(rows[1].pid, 40);
        assert_eq!(rows[2].pid, 50);
        assert_eq!(rows[3].pid, 42);
    }

    #[test]
    fn compare_rows_orders_higher_cpu_and_memory_first() {
        let high_cpu = ProcRow {
            pid: 1,
            start_time: 1,
            ppid: None,
            ancestor_chain: Vec::new(),
            user: Arc::from("u"),
            status: ProcessStatus::Run,
            cpu_usage_tenths: 90,
            memory_bytes: 10,
            name: "a".to_string(),
            cmd: "a".to_string(),
        };
        let high_mem = ProcRow {
            pid: 2,
            start_time: 2,
            ppid: None,
            ancestor_chain: Vec::new(),
            user: Arc::from("u"),
            status: ProcessStatus::Run,
            cpu_usage_tenths: 80,
            memory_bytes: 999,
            name: "b".to_string(),
            cmd: "b".to_string(),
        };

        assert_eq!(compare_rows(&high_cpu, &high_mem), Ordering::Less);
        assert_eq!(compare_rows(&high_mem, &high_cpu), Ordering::Greater);
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
        let filter = FilterSpec::Substring {
            raw: "__psn_filter_that_should_not_exist__".to_string(),
            lowered: "__psn_filter_that_should_not_exist__".to_string(),
            ascii_only: true,
        };
        let rows = refresh_rows(&mut sys, Some(&filter), false);
        assert!(rows.is_empty());
    }

    #[test]
    fn refresh_rows_user_only_applies_current_uid_branch() {
        let mut sys = System::new_all();
        let _rows = refresh_rows(&mut sys, None, true);
    }

    #[test]
    fn matches_filter_handles_empty_and_non_ascii_substring() {
        let r = row(1, "Ångström", ProcessStatus::Run, "/usr/bin/ångström");
        assert!(matches_filter(
            &r,
            Some(&FilterSpec::Substring {
                raw: "".to_string(),
                lowered: "".to_string(),
                ascii_only: true,
            })
        ));
        assert!(matches_filter(
            &r,
            Some(&FilterSpec::Substring {
                raw: "ång".to_string(),
                lowered: "ång".to_string(),
                ascii_only: false,
            })
        ));
    }

    #[test]
    fn resolve_user_cached_handles_missing_uid() {
        let mut cache = HashMap::new();
        assert_eq!(&*resolve_user_cached(None, &mut cache), "?");
    }

    #[test]
    fn build_ancestor_chain_breaks_on_cycle() {
        let mut pid_to_ppid_all = HashMap::new();
        pid_to_ppid_all.insert(2, Some(3));
        pid_to_ppid_all.insert(3, Some(2));

        let chain = super::build_ancestor_chain(Some(2), &pid_to_ppid_all);
        assert_eq!(chain, vec![2, 3]);
    }
}
