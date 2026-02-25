// Copyright (c) 2026 l5yth
// SPDX-License-Identifier: Apache-2.0

//! Signal mapping and process signaling helpers.

use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

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

#[cfg(test)]
mod tests {
    use super::{send_signal, signal_from_digit};
    use nix::sys::signal::Signal;

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
}
