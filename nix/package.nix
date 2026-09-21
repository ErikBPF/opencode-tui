# Build opencode-tui from this repository. The crate has no build-time
# dependency on reference/ (that tree is a read-only interface reference), so
# only Cargo.toml/Cargo.lock/src are needed.
{
  lib,
  rustPlatform,
}:

let
  root = ./..;
  excluded = [
    ".git"
    ".devenv"
    "target"
    "reference"
    "docs"
    "features"
    ".github"
    "flake.nix"
    "flake.lock"
    "devenv.nix"
    "devenv.yaml"
    "devenv.lock"
    "justfile"
    "rust-toolchain.toml"
    "README.md"
    "AGENTS.md"
    "CONTRIBUTING.md"
  ];
  src = lib.cleanSourceWith {
    src = root;
    filter =
      path: type:
      let
        base = baseNameOf path;
      in
      !(builtins.elem base excluded);
  };
in
rustPlatform.buildRustPackage {
  pname = "opencode-tui";
  version = "0.0.1";

  inherit src;

  cargoLock = {
    lockFile = ../Cargo.lock;
  };
  # The contract test needs a spawned `opencode serve`; skip it here and let
  # CI and the justfile own that gate.
  doCheck = false;

  meta = {
    description = "Rust/ratatui client for the opencode server REST+SSE API";
    license = lib.licenses.asl20;
    mainProgram = "opencode-tui";
  };
}
