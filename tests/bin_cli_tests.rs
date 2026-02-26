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

use std::process::Command;

#[test]
fn binary_help_flag_prints_usage() {
    let output = Command::new(env!("CARGO_BIN_EXE_psn"))
        .arg("--help")
        .output()
        .expect("binary should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("usage: psn <FILTER>"));
    assert!(stdout.contains("usage: psn [OPTIONS] -r <PATTERN>"));
}

#[test]
fn binary_version_flag_prints_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_psn"))
        .arg("--version")
        .output()
        .expect("binary should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("psn v"));
    assert!(stdout.contains("process status navigator"));
}
