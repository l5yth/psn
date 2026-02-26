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

use anyhow::Result;
use psn::cli::{CliCommand, help_text, parse_args, version_text};

/// Binary entry point. Delegates to CLI parsing and runtime.
fn main() -> Result<()> {
    match parse_args(std::env::args())? {
        CliCommand::Help => {
            println!("{}", help_text());
            Ok(())
        }
        CliCommand::Version => {
            println!("{}", version_text());
            Ok(())
        }
        CliCommand::Run {
            filter,
            regex_mode,
            user_only,
        } => psn::run(filter, regex_mode, user_only),
    }
}
