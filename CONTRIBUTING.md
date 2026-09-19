# opencode-tui — contributing

`opencode-tui` is a Rust + ratatui terminal client for the
[opencode](https://opencode.ai) server REST + SSE API. It is a faithful port of
the upstream TUI's client seams, rewritten natively so the fleet has a small
front-end that does not carry the Bun runtime. This file is the human contract;
`AGENTS.md` is the agent-facing subset with the same rules.

## Environment

Everything comes from the declared devenv. Do not install toolchains by hand and
do not rely on a host-wide `cargo`.

```bash
devenv shell          # cargo, rustc, rustfmt, clippy, just
just                  # lists every recipe
just check            # cargo check --all-targets
just test             # unit tests
just features         # validates every *.feature contract
just contracts        # binding for features/attach-and-prompt.feature
just ci               # fmt + clippy + tests + features + contracts
```

`just reference` fetches the pinned upstream client as a sparse checkout under
`reference/opencode` (gitignored). It is read for interface fidelity only.

## The rules

1. **Behavior ships with its contract.** Every behavior you change or add gets a
   `.feature` beside the code it validates (`features/`) and a binding that fails
   without your change. A `.feature` with no bound steps is a draft, and its
   header must say so.
2. **Tests live with their owner.** In-file `#[cfg(test)]` for a unit;
   `tests/` for anything exercising the client end to end.
3. **Mirror upstream, do not redesign it.** The module tree tracks
   `packages/tui/src`. If upstream has a seam, this crate names it the same and
   documents the Rust shape in the README fidelity table.
4. **The client attaches, never hosts.** No server process management, no
   implicit localhost assumption — the URL is always explicit input.
5. **Unknown server events are tolerated.** The decoder degrades to `Unknown`
   rather than failing the stream, so a server patch that adds an event cannot
   take the client down.
6. **The terminal is always restored.** Raw mode is entered only after a
   successful initial load, and every exit path restores it.
7. **Errors at the boundary.** `thiserror` in libraries, `anyhow` in binaries.
   Internal detail is logged, not dumped at the user above the CLI.
8. **No new dependency without a reason.** Stdlib or an already-present crate
   first.

## Change flow

1. Ground the change against the pinned upstream seam and the owning server
   endpoint before editing.
2. Write/extend the `.feature` and the failing test first.
3. Implement until `just ci` is green.
4. Update the `homelab` proposal when a cross-repository decision changes.
5. Human review: read the diff in `tuicr`; treat `issue` comments as blocking.

Commit messages are Conventional Commits (`feat(sdk): ...`), imperative,
explaining why and what was verified. Never commit secrets, `.env`, or anything
under `reference/`.

## Reporting security issues

Do not open a public issue for a vulnerability. Report it privately to the
maintainer (see `homelab` repo conventions) with reproduction steps and impact.
