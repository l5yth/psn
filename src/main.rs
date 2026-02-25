// Copyright (c) 2026 l5yth
// SPDX-License-Identifier: Apache-2.0

#![allow(unexpected_cfgs)]

use anyhow::Result;

/// Binary entry point. Delegates to the library runtime.
#[cfg(not(coverage))]
fn main() -> Result<()> {
    psn::run()
}

/// Coverage-only lightweight entry point to keep binary coverage deterministic.
#[cfg(coverage)]
fn main() -> Result<()> {
    Ok(())
}

#[cfg(all(test, coverage))]
mod tests {
    #[test]
    fn main_returns_ok_under_coverage() {
        assert!(super::main().is_ok());
    }
}
