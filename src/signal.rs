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

//! Signal mapping and process signaling helpers.

use std::{
    thread,
    time::{Duration, Instant},
};

use nix::{
    errno::Errno,
    sys::signal::{Signal, kill},
    unistd::Pid,
};

/// Map keyboard digits 1-9 to Unix signals.
pub fn signal_from_digit(digit: u8) -> Option<Signal> {
    match digit {
        1 => Some(Signal::SIGHUP),
        2 => Some(Signal::SIGINT),
        3 => Some(Signal::SIGQUIT),
        4 => Some(Signal::SIGILL),
        5 => Some(Signal::SIGTRAP),
        6 => Some(Signal::SIGABRT),
        7 => Some(Signal::SIGBUS),
        8 => Some(Signal::SIGFPE),
        9 => Some(Signal::SIGKILL),
        _ => None,
    }
}

/// Send the provided signal to a process id.
pub fn send_signal(pid: i32, signal: Signal) -> nix::Result<()> {
    kill(Pid::from_raw(pid), signal)
}

/// Upper bound on how long we wait for a signaled process to disappear from
/// `/proc` before giving up and refreshing anyway. Sized to be short enough that
/// the event loop freeze is not perceptible for catchable signals (SIGHUP,
/// SIGINT, …) that the target may not honor, while still leaving comfortable
/// headroom for SIGKILL — which the kernel typically reaps within a few ms.
pub const SIGNAL_REAP_TIMEOUT: Duration = Duration::from_millis(100);

/// Polling interval for the existence probe inside [`wait_for_pid_gone`].
pub const SIGNAL_REAP_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Poll until `pid` is absent from the process table or `timeout` elapses.
///
/// Probes existence with `kill(pid, None)` (POSIX null-signal): `Err(ESRCH)`
/// means the pid no longer exists. Bridges the gap between `kill(2)` returning
/// (signal queued) and the kernel actually removing the entry from `/proc`,
/// so a refresh issued right after a signal send sees the updated state.
///
/// Returns `true` if the process is gone before the deadline, `false` if it is
/// still alive after `timeout`. The first probe always runs, so a `timeout` of
/// `Duration::ZERO` still returns `true` for an already-absent pid.
///
/// PID reuse: if the kernel reaps the original and recycles the pid for an
/// unrelated process inside the polling window, this returns `false`. The
/// caller must re-key on `(pid, start_time)` (see [`crate::app::App`]'s
/// `pending_target_matches_current_rows`) to detect that case after the
/// follow-up refresh.
pub fn wait_for_pid_gone(pid: i32, timeout: Duration, interval: Duration) -> bool {
    let started = Instant::now();
    loop {
        if matches!(kill(Pid::from_raw(pid), None::<Signal>), Err(Errno::ESRCH)) {
            return true;
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            return false;
        }
        let remaining = timeout - elapsed;
        thread::sleep(interval.min(remaining));
    }
}

/// Convenience wrapper around [`wait_for_pid_gone`] using the project-default
/// timeout and polling interval. The boolean outcome is intentionally
/// discarded: the caller will refresh from `/proc` next, which surfaces the
/// truth (gone vs. still-alive after a non-fatal signal) either way.
pub fn wait_for_pid_gone_default(pid: i32) {
    let _ = wait_for_pid_gone(pid, SIGNAL_REAP_TIMEOUT, SIGNAL_REAP_POLL_INTERVAL);
}

#[cfg(test)]
mod tests {
    use super::{send_signal, signal_from_digit, wait_for_pid_gone, wait_for_pid_gone_default};
    use nix::sys::signal::Signal;
    use std::{
        process::Command,
        time::{Duration, Instant},
    };

    #[test]
    fn signal_from_digit_maps_expected_range() {
        for digit in 1..=9 {
            assert!(signal_from_digit(digit).is_some());
        }
    }

    #[test]
    fn signal_from_digit_rejects_outside_range() {
        assert!(signal_from_digit(0).is_none());
        assert!(signal_from_digit(10).is_none());
        assert!(signal_from_digit(200).is_none());
    }

    #[test]
    fn send_signal_can_signal_current_process() {
        let pid = std::process::id() as i32;
        assert!(send_signal(pid, Signal::SIGCONT).is_ok());
    }

    #[test]
    fn wait_for_pid_gone_returns_true_for_nonexistent_pid() {
        assert!(wait_for_pid_gone(
            i32::MAX - 1,
            Duration::from_millis(50),
            Duration::from_millis(10),
        ));
    }

    #[test]
    fn wait_for_pid_gone_times_out_for_living_process() {
        let pid = std::process::id() as i32;
        let started = Instant::now();
        let gone = wait_for_pid_gone(pid, Duration::from_millis(50), Duration::from_millis(10));
        assert!(!gone);
        assert!(started.elapsed() >= Duration::from_millis(50));
    }

    #[test]
    fn wait_for_pid_gone_zero_timeout_polls_at_least_once() {
        assert!(wait_for_pid_gone(
            i32::MAX - 1,
            Duration::ZERO,
            Duration::ZERO,
        ));
    }

    #[test]
    fn wait_for_pid_gone_detects_child_exit() {
        let mut child = Command::new("true").spawn().expect("spawn `true`");
        let pid = child.id() as i32;
        child.wait().expect("reap child");
        assert!(wait_for_pid_gone(
            pid,
            Duration::from_millis(500),
            Duration::from_millis(10),
        ));
    }

    #[test]
    fn wait_for_pid_gone_default_returns_quickly_for_nonexistent_pid() {
        let started = Instant::now();
        wait_for_pid_gone_default(i32::MAX - 1);
        // Nonexistent pid resolves on the first probe, well below the cap.
        assert!(started.elapsed() < super::SIGNAL_REAP_TIMEOUT);
    }
}
