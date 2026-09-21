# opencode-tui workflows. Recipes are the single source of truth: CI, devenv's
# `enterTest`, and humans all call into this file.
default:
    @just --list

root := justfile_directory()
reference_dir := "reference/opencode"
reference_repo := "https://github.com/anomalyco/opencode"
reference_tag := "v1.18.30"

# Use the ambient cargo when the devenv shell is active, otherwise enter it, so
# `just x` works the same from inside or outside the shell.
cargo := if `command -v cargo 2>/dev/null || true` != "" { "cargo" } else { "devenv shell -- cargo" }

check:
    {{cargo}} check --all-targets

test:
    {{cargo}} test

fmt:
    {{cargo}} fmt

format-check:
    {{cargo}} fmt -- --check

clippy:
    {{cargo}} clippy --all-targets -- -D warnings

lint: clippy

# Fetch the pinned upstream client as a sparse, gitignored reference. Not a
# dependency: it is read for interface fidelity only.
reference:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -d {{reference_dir}}/.git ]; then
      echo "reference already present:"; git -C {{reference_dir}} rev-parse HEAD; exit 0
    fi
    git clone --filter=blob:none --no-checkout --depth 1 --branch {{reference_tag}} {{reference_repo}} {{reference_dir}}
    git -C {{reference_dir}} sparse-checkout init --cone
    git -C {{reference_dir}} sparse-checkout set packages/tui packages/sdk packages/protocol packages/docs
    git -C {{reference_dir}} checkout
    git -C {{reference_dir}} rev-parse HEAD

# Every behavior contract must live beside the code it validates and stay
# well-formed.
features:
    #!/usr/bin/env bash
    set -euo pipefail
    shopt -s nullglob
    files=(features/*.feature crates/*/features/*.feature)
    [[ ${#files[@]} -gt 0 ]] || { echo "no .feature files found"; exit 1; }
    fail=0
    for f in "${files[@]}"; do
      grep -q '^Feature:' "$f" || { echo "missing Feature: in $f"; fail=1; }
      grep -q 'Scenario' "$f" || { echo "no Scenario in $f"; fail=1; }
    done
    [[ $fail -eq 0 ]] && echo "features OK: ${#files[@]} file(s)"

# Binding for features/attach-and-prompt.feature: the offline scenarios run
# through cucumber-rs against a pinned `opencode serve`.
contracts: features
    {{cargo}} test --test contract

# The @live scenarios additionally need a reachable model provider.
contracts-live: features
    OPENCODE_TUI_CONTRACT_LIVE=1 {{cargo}} test --test contract

ci: format-check lint test features contracts
    @echo "CI GREEN (fmt + clippy + tests + features + contracts)"

# Attach to a running server. URL defaults to the local server. Uses the
# release build: the TUI must redraw smoothly, and an unoptimized build is
# visibly slow.
url := env_var_or_default("OPENCODE_URL", "http://127.0.0.1:4096")
run: release
    ./target/release/opencode-tui --url {{url}}

# Build the optimized binary.
release:
    {{cargo}} build --release

# Run the opt-in live tests against a running server.
live server="http://127.0.0.1:4096":
    OPENCODE_TUI_LIVE_URL={{server}} {{cargo}} test -- --ignored
