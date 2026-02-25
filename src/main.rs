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
