# Copyright (c) 2026 l5yth
# SPDX-License-Identifier: Apache-2.0
{ lib, rustPlatform }:

rustPlatform.buildRustPackage {
  pname = "psn";
  version = "0.1.4";

  src = lib.cleanSource ../..;

  cargoLock.lockFile = ../../Cargo.lock;

  meta = with lib; {
    description = "Terminal UI for process status navigation and control";
    homepage = "https://github.com/l5yth/psn";
    license = licenses.asl20;
    mainProgram = "psn";
    platforms = platforms.linux;
  };
}
