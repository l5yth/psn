// Copyright (c) 2026 l5yth
// SPDX-License-Identifier: Apache-2.0

//! Domain data structures for process rows shown in the TUI.

use sysinfo::ProcessStatus;

/// A single process row rendered in the process table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcRow {
    /// Process identifier.
    pub pid: i32,
    /// Resolved user name or fallback identifier.
    pub user: String,
    /// Process status from sysinfo.
    pub status: ProcessStatus,
    /// Short process name.
    pub name: String,
    /// Full command line, when available.
    pub cmd: String,
}
