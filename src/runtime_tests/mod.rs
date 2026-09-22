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

//! Unit tests for [`crate::runtime`], split by concern.
//!
//! Included into `runtime.rs` via `#[path]` rather than living inside it, so
//! these modules keep private-item access while every source file stays inside
//! the house 500-1000 LOC band. `runtime.rs` was 1563 lines, 1068 of them this
//! test module.
//!
//! Shared fixtures live here; the tests live in [`actions`] and [`filter`].

mod actions;
mod filter;

use crate::model::ProcRow;
use std::sync::Arc;
use sysinfo::ProcessStatus;

/// Stand-in for the production `await_pid_gone` callback. Defined as a
/// real `fn` so the body is covered by `noop_await_runs` and every other
/// test can reference it without instantiating its own closure.
pub(super) fn noop_await(_: i32) {}

#[test]
fn noop_await_runs() {
    noop_await(0);
}

/// Panicking `await_pid_gone` stand-in for negative tests that assert the
/// callback is never invoked. Covered by `must_not_run_panics_when_called`.
pub(super) fn must_not_run(_: i32) {
    panic!("await_pid_gone must not be called when sender fails");
}

#[test]
#[should_panic(expected = "await_pid_gone must not be called when sender fails")]
fn must_not_run_panics_when_called() {
    must_not_run(0);
}

pub(super) fn row(pid: i32, name: &str) -> ProcRow {
    ProcRow {
        pid,
        start_time: 0,
        ppid: None,
        ancestor_chain: Vec::new(),
        user: Arc::from("u"),
        status: ProcessStatus::Run,
        cpu_usage_tenths: 0,
        memory_bytes: 0,
        name: name.to_string(),
        cmd: format!("/bin/{name}"),
    }
}
