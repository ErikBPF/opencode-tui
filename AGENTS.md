# AGENTS.md — working in `opencode-tui`

Agent-facing rules. They are a subset of `CONTRIBUTING.md`; if the two conflict,
`CONTRIBUTING.md` wins.

## Grounding first

This is a client, not a server. Read the whole path a change touches — the
upstream seam it mirrors, the Rust module, the event decoder, the route that
renders it — before editing. The pinned upstream source lives in the gitignored
`reference/opencode` (run `just reference`); the server contract is its
`packages/sdk/openapi.json` and `packages/tui/src`, at `v1.18.30`. Treat upstream
as read-only truth for interfaces, never as build input.

## Where code goes

| Change | Location |
|---|---|
| Transport, headers, REST calls | `src/context/sdk.rs` |
| Event union / decoder | `src/context/event.rs` |
| Server state store and reducers | `src/context/sync.rs` |
| Active route | `src/context/route.rs` |
| Theme identity | `src/context/theme.rs`, `src/theme/` |
| Keybindings | `src/config/keybind.rs`, `src/keymap.rs` |
| TUI config loading | `src/config/mod.rs` |
| Screen | `src/routes/<screen>.rs` |
| Reusable widget | `src/component/`, `src/ui/` |
| CLI arguments | `src/runtime.rs` |
| App wiring / main loop | `src/app.rs` |
| Behavior contract | `features/*.feature` |
| Unit test | `#[cfg(test)]` in the same file |
| Contract binding | `tests/` |

Module paths mirror upstream `packages/tui/src`, so a reader can diff the two
trees. Keep that mapping; do not invent a new layout.

## Non-negotiables

- The client never spawns or supervises a server. It attaches to a URL.
- The directory scope is URL-encoded and only sent when a directory is set, as a
  `directory` query param on GET/HEAD and the `x-opencode-directory` header on
  writes, exactly as the upstream SDK does.
- An event type the decoder does not know must not kill the stream; decode it to
  `Unknown`.
- The event stream dropping flips status to `Partial` and retries with bounded
  backoff (1s..30s); it never silently claims `Complete`.
- Terminal raw mode is entered only after a successful initial load, so a
  connection error is reported as plain text and leaves the terminal usable.
- Never log credentials or request headers.

## Verify before claiming done

```bash
just ci            # fmt --check + clippy -D warnings + tests + feature contracts
```

Report evidence (command + result).

## Behavior files

Every behavior change lands with a `.feature` beside the code it validates and a
binding that fails without the change. Scenarios state observable outcomes
(screens, statuses, messages), never implementation detail. `just contracts`
runs the offline scenarios through cucumber-rs against a pinned `opencode serve`;
a scenario that needs a live model provider is tagged `@live` and runs only under
`just contracts-live`. Never claim a scenario passed unless its binding ran.

## Repository conventions

- **devenv** is the only toolchain source: add tools to `devenv.nix`, never to a
  global profile.
- **justfile** owns every workflow: add a recipe instead of documenting a long
  command. Recipes that do real work use a bash shebang with `set -euo pipefail`.
- Dependencies are added only with a reason; stdlib or an already-present crate
  first.

## Also

- Do not commit, push, or apply anything unless the user asked for it in this
  session.
- Do not create documentation files the user did not ask for.
- Cross-repository decisions live in the `homelab` proposal
  `docs/proposals/2026-09-19-opencode-tui-rust-client.md`; update it and
  `docs/proposal-index.md` in the same change when a decision changes.
