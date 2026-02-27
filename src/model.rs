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

//! Domain data structures for process rows shown in the TUI.

use std::sync::Arc;

use sysinfo::ProcessStatus;

/// A single process row rendered in the process table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcRow {
    /// Process identifier.
    pub pid: i32,
    /// Process start time as reported by sysinfo, used to disambiguate pid reuse.
    pub start_time: u64,
    /// Parent process identifier when available.
    pub ppid: Option<i32>,
    /// Ancestor pid chain from immediate parent upward.
    pub ancestor_chain: Vec<i32>,
    /// Resolved user name or fallback identifier.
    pub user: Arc<str>,
    /// Process status from sysinfo.
    pub status: ProcessStatus,
    /// CPU usage in tenths of a percent, used for hidden sort ordering.
    pub cpu_usage_tenths: u32,
    /// Resident memory usage in bytes, used for hidden sort ordering.
    pub memory_bytes: u64,
    /// Short process name.
    pub name: String,
    /// Full command line, when available.
    pub cmd: String,
}
