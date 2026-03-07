# Copyright (c) 2026 l5yth
# SPDX-License-Identifier: Apache-2.0
{
  description = "Terminal UI for process status navigation and control";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachSystem [
      "x86_64-linux"
      "aarch64-linux"
      "armv6l-linux"
      "armv7l-linux"
    ] (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in {
        packages.default = pkgs.callPackage ./packaging/nix {};
      }
    );
}
