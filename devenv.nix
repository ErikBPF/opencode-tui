{ pkgs, lib, config, inputs, ... }:

{
  packages = [
    # rust toolchain: pinned by the declared devenv, not by a host install
    pkgs.cargo
    pkgs.rustc
    pkgs.rustfmt
    pkgs.clippy
    pkgs.rust-analyzer

    # workflows and glue
    pkgs.git
    pkgs.just
    pkgs.jq
    pkgs.curl
    pkgs.ripgrep
    pkgs.shellcheck
    pkgs.python3
  ];

  enterShell = ''
    echo "=== opencode-tui — Rust client for opencode serve ==="
    echo "  rust:  $(cargo --version 2>/dev/null)"
    echo "  just:  $(just --version 2>/dev/null)"
    echo "  try:   just ci | just reference | just run"
  '';

  # `devenv test` is the gate CI runs; it delegates to the justfile so the
  # recipes stay the single source of truth.
  enterTest = ''
    just ci
  '';
}
