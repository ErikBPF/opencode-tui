# opencode-tui implementation plan (M1)

**Milestone:** attach + read + prompt.
**Owner:** `opencode-tui`.
**Basis:** the `homelab` proposal
`docs/proposals/2026-09-19-opencode-tui-rust-client.md` (human seed, decisions
D1–D11, party ledger, frontier Q1–Q4) and the behavior contract
[`features/attach-and-prompt.feature`](../features/attach-and-prompt.feature).
**Pinned server / reference:** `anomalyco/opencode@v1.18.30`
(`3104c1428ec91f809e5ab86631300de41eb6952e`).

This plan implements; it does not re-decide. A disputed destination or behavior
returns to `/pl`.

## Grounding

- The client attaches to an explicit URL and never manages a server (D9).
- Transport is the v1 REST + SSE surface (D2). The seam it mirrors is upstream
  `packages/tui/src/context/sdk.tsx`; the store lifecycle mirrors
  `context/sync.tsx`; typed dispatch mirrors `context/event.ts`.
- The connection path in `src/app.rs` already runs the initial REST load
  **before** `ratatui::init()`, so an unreachable server is a plain-text error
  and raw mode is never entered. This invariant is graded in S1.
- The event union is versioned: the decoder must tolerate unknown event types
  (D2 risk gate), and the reference tag is pinned so drift is visible in review.

## Test seams

Fewest useful seams, in cost order:

| Seam | Location | Covers | Cost |
|---|---|---|---|
| Pure unit tests | `#[cfg(test)]` in `sdk.rs`, `event.rs`, `sync.rs`, `runtime.rs`, `util/mod.rs` | header encoding, backoff, decode, reducers, args | none (no server) |
| Stub HTTP/SSE server | `tests/support/` (dev-only) | REST decode + SSE framing + reconnect | one in-process stub |
| Opt-in live integration | `tests/live_attach.rs`, gated by `OPENCODE_TUI_LIVE_URL` | real server happy path | needs a running server; skipped in CI |
| Contract binding | `tests/` with `cucumber` + a pinned `opencode serve` (S7) | the `.feature` scenarios | largest; last |

The pure seams carry most of the grading. The stub is preferred over `axum` for
REST/SSE because the payloads are small and fixed; add a real dev-server
dependency only if the stub becomes the thing that breaks.

## Slices

Ordered leaf-first. Each slice lists: observable result → RED → GREEN → checks →
owner/deps → rollout/rollback.

### S1 — Attach and show the session list

- **Observable:** launching against a reachable server shows the server's
  sessions; launching against an unreachable URL names the URL and exits 1
  without touching the terminal.
- **Scenario:** "Connecting makes the client ready", "An unreachable server is
  reported, not hidden".
- **RED:** `runtime::Args` unit tests (default URL, `--url`, `--dir`,
  `OPENCODE_DIRECTORY`, unknown arg); the directory-scope tests in `sdk.rs`
  (query param on GET, encoded header on writes, absent when unset); a live
  integration test that is `#[ignore]` unless `OPENCODE_TUI_LIVE_URL` is set.
  Command: `just test`. Expected failure: argument parsing is unasserted.
  (The `sdk.rs` directory tests landed in the RV revision and pass; the `Args`
  and live tests are still owed.)
- **GREEN:** already-shipped `Args`, `OpencodeClient::new`,
  `OpencodeClient::list_sessions`, `Store::loaded`, `routes::home::render`.
  Add the tests only; no new behavior.
- **Checks:** `just ci`; the documented unreachable-server command
  `cargo run -- --url http://127.0.0.1:59999` exits 1 and prints the URL
  (verified once in the RV revision).
- **Deps:** none. **Rollback:** revert the test files.

### S2 — Event stream degrades and recovers

- **Observable:** the footer status moves `Loading → Complete` on
  `server.connected`; a dropped stream becomes `Partial`; the client retries
  with 1s..30s backoff and returns to `Complete` on reconnect.
- **Scenario:** "A dropped event stream degrades and recovers".
- **RED:** `event` unit tests decoding fixture envelopes (payload wrapper,
  `sync` frames, unknown type → `Unknown`); `backoff(attempt)` unit tests
  asserting 1, 2, 4 … 30 s and the cap; `sync` unit tests for
  `ServerConnected`/`SessionUpdated`/`SessionDeleted`/`degraded`. Command:
  `just test`. Expected failure: the stub-server round-trip test does not exist.
  (The `event`/`sync`/`backoff` unit tests landed in the RV revision and pass.)
- **GREEN:** `event::backoff(attempt) -> Duration` landed in the RV revision,
  the SSE decode is now traced instead of silently dropped, `sync::apply` handles
  `session.deleted`, and the queue is bounded (1024). Remaining: the stub-server
  reconnect test.
- **Checks:** `just ci`; stub-server integration test that closes the stream and
  asserts a second `server.connected` is observed within the backoff bound.
- **Deps:** S1. **Rollback:** revert to the inline loop.

### S3 — Open a session and render its transcript

- **Observable:** selecting a session lists its messages in server order;
  assistant parts render by type; tool parts show name and status.
- **Scenario:** "Opening a session renders its transcript".
- **RED:** `sdk` unit test for `GET /session/{id}/message` decode;
  `sync` reducer test for `message.updated` / `message.part.updated` upserting
  into the open session; a `routes::session` render smoke test on a fixed store.
  Command: `just test`. Expected failure: the route renders a placeholder and
  the message types do not exist.
- **GREEN:** add `Message`/`MessagePart` types (serde, camelCase) in `sdk.rs`;
  add `Store::transcript` with ordered upsert; add
  `OpencodeClient::list_messages(session_id)`; replace the placeholder in
  `routes/session.rs` with a scrollable list of parts.
- **Checks:** `just ci`; live test opening the newest session.
- **Deps:** S1, S2. **Rollback:** revert `routes/session.rs` to the placeholder;
  message types are additive.

### S4 — Submit a prompt and stream the reply

- **Observable:** on an open session, submitting `"reply with the single word
  pong"` posts to that session and the reply appears without a manual refresh.
- **Scenario:** "Submitting a prompt streams the assistant reply".
- **RED:** `sdk` unit test for the `POST /session/{id}/message` body shape
  (`{"parts":[{"type":"text","text":...}]}`, the v1 prompt operation) and
  non-2xx mapping; a `context::prompt` unit test for input editing; a reducer
  test that an optimistic user part and the streamed assistant part both land
  once. Command: `just test`. Expected failure: there is no prompt type or POST.
- **GREEN:** add `context/prompt.rs` (mirrors upstream `context/prompt.tsx`)
  and the session-screen input line; `sdk::send_prompt(session_id, text)`
  mirrors the upstream `SessionPromptData` `parts` shape.
- **Checks:** `just ci`; live test asserting a `pong` part arrives within a
  bounded wait; the reply must be built from the event stream, not from the POST
  response.
- **Deps:** S3. **Rollback:** hide the prompt line; the POST is additive.
- **As built:** `Enter` is one command with route-dependent meaning: on the home
  screen it opens the selected session, in a session it submits. `Ctrl-C` quits
  anywhere; `Esc` is `session_interrupt` (returns to the list, since the pinned
  v1 API has no interrupt endpoint); bare `q` quits only on the home screen.

### S5 — Permission requests are surfaced, not answered

- **Observable:** a `permission.updated` event renders the request and its
  scope; the client does not call the reply endpoint.
- **Scenario:** "A permission request is surfaced but not answered".
- **RED:** reducer test that a fixture `permission.updated` sets a pending
  permission for the open session; the banner renders its title and pattern.
  Command: `just test`. Expected failure: the event is decoded but dropped.
- **GREEN:** add `Store.pending_permission`, the `Permission` wire type, and a
  read-only banner on the session screen cleared by `permission.replied`. No
  reply action (frontier Q3).
- **Checks:** `just ci`; live test that a denied tool shows the banner and the
  session continues.
- **Deps:** S3. **Rollback:** remove the banner; the store field is additive.

### S6 — Directory scope and CLI/config polish

- **Observable:** `--dir "/a path/with spaces"` reaches the server; the footer
  shows the abbreviated path; `-h` prints usage.
- **Scenario:** "The directory scope is carried and encoded".
- **RED:** unit tests already added in S1 cover encoding; add a `util` test for
  `abbreviate_home`. Command: `just test`.
- **GREEN:** already shipped; tests close the gap. Keybind overrides from
  `~/.config/opencode/tui.json` shipped as S6b (below).
- **Deps:** S1. **Rollback:** none, additive.

### S6b — Keybind overrides from `tui.json`

- **Observable:** `~/.config/opencode/tui.json` `keybinds` entries replace the
  built-in bindings; `"none"`/`false` disables one; unknown or malformed keys are
  reported and ignored rather than silently applied.
- **Scenario:** "Keybindings can be overridden from tui.json".
- **RED:** `config::keybind` unit tests for defaults, lists, `none`/`false`, and
  unknown-key reporting; the contract scenario asserts the same through the real
  resolver. Command: `just test` / `just contracts`.
- **GREEN:** `Keybinds::resolve` merges user overrides over `defaults()` using the
  upstream binding-string form (`ctrl+c`, `escape,q`, `none`); `config::load_keybinds`
  reads `$OPENCODE_CONFIG_DIR/tui.json` or `~/.config/opencode/tui.json` and
  falls back to defaults on a missing or malformed file. A malformed value is
  recorded in `Keybinds::unknown` and warned, not applied.
- **Deps:** S1. **Rollback:** delete the overrides and the loader; defaults are
  built in.

### S7 — Bind the behavior contract

- **Observable:** `just contracts` runs the seven offline `.feature` scenarios
  through cucumber-rs against a pinned `opencode serve` and they pass; the
  `@live` prompt scenario runs only under `just contracts-live`.
- **RED:** the binding was added while S3–S5 existed and the failing set was
  observed: unreachable/ready, session-list and directory-encoding scenarios
  failed first, then the transcript seed. Recorded here as the milestone's
  acceptance anchor rather than re-derived from the S1 skeleton.
- **GREEN:** no production change beyond the `--check` readiness flag and the
  short-timeout `ready()` probe, which make the readiness scenario bindable
  headlessly. Two contract-shape corrections were required and applied:
  `query_pairs()` percent-decodes, so the encoding assertion reads the raw query
  string; and the offline transcript seeds the store through the same reducers
  the SSE loop uses, because a real assistant turn needs a model provider, so
  that scenario is `@live`.
- **Checks:** `just ci` (contracts included: `cargo test --test contract`).
  The `UNAUTOMATED CONTRACT` header is gone from the `.feature`.
- **Deps:** S4, S5. **Rollback:** revert the slice commit; the `features` recipe
  still validates the file shape independently.

## Rollout and rollback

- All work lands on `opencode-tui` `main` as conventional commits; leaf-first is
  not applicable (single crate), so the order is slice order.
- No live target is authorized beyond a local/pinned `opencode serve`; the opt-in
  live tests default to skipped, and `@live` scenarios require
  `OPENCODE_TUI_CONTRACT_LIVE=1`.
- Rollback per slice is reverting that commit; nothing is installed, deployed or
  pinned into another repository by this milestone. The `homelab-iac` repo
  creation is `prevent_destroy`.

## Grill findings

Two bounded rounds, applied.

- **R1 — "The event union is the real risk."** Pinning the reference tag does not
  pin the *server* the user runs. Resolution: decode unknown types to `Unknown`
  and keep the store's `status` independent of any event type; S2 grades this.
- **R1 — "The stub server is a second implementation to maintain."** Resolution:
  keep the stub to fixed fixtures only; do not grow it into a fake server. If a
  scenario needs real semantics, it belongs in the S7 live binding.
- **R1 — "Prompt fidelity is the easiest thing to fake."** A reply read from the
  POST response would pass a naive test but break streaming. Resolution: S4's
  check requires the reply to arrive via the event stream.
- **R2 — "Terminal restore on panic."** ~~A panic unwinds past
  `ratatui::restore()`.~~ Corrected in the RV revision: `ratatui::init()` already
  installs a panic hook that restores the terminal, and the `Err` path calls
  `restore()` explicitly, so no extra hook is needed.
- **R2 — "Permission answering is a hidden dependency."** If the server blocks a
  tool until permission is answered, an unanswered request stalls the session.
  Resolution: confirm during S5 with a live tool call; if it blocks, an
  auto-deny-by-default option becomes a new `/pl` question, not a silent feature.

## RV revision (2026-09-19)

The scaffold was reviewed by independent correctness/conformance and
security/reliability passes. Applied: directory scope moved to a `directory`
query param on GET/HEAD and the encoded header only on writes (matching the SDK);
the v1 prompt endpoint corrected to `POST /session/{id}/message`; `Esc` returns
instead of quitting; backoff no longer resets on connect; the event queue is
bounded; `session.deleted` prunes the store; connect timeout, redirect refusal
and URL-userinfo redaction added; decode failures are traced; unused
`thiserror`/`crossterm event-stream`/`tokio full` removed and TLS switched to
native roots. Unit seams for args, directory scope, backoff, decode, reducers and
path abbreviation landed with the fixes. Deferred (recorded, not fixed): a
stub-server reconnect test, `Args` unit tests, and the cucumber binding.

## Next

`/ip` slices: **S1–S7 are implemented.** S1 parses `Args` by a pure
`parse(argv, url_env, dir_env)` with unit tests and adds `--check`, a headless
readiness probe; the live test is opt-in via `just live`. S2 exercises
`app::spawn_event_stream` with a stub TCP/SSE server and a bounded exponential
backoff. S3 installs the transcript from `GET /session/{id}/message` and folds
`message.updated` / `message.part.updated` into the open session. S4 adds
`context/prompt.rs` and `POST /session/{id}/message`. S5 surfaces
`permission.updated` read-only and clears it on `permission.replied`. S6 shell
polish shipped with the others. S7 binds the contract: `just contracts` runs the
nine offline scenarios through cucumber-rs against a pinned `opencode serve`, and
`just contracts-live` adds the model-dependent `@live` prompt scenario. S6b reads
`~/.config/opencode/tui.json` `keybinds` overrides, with `none`/`false` to
disable and unknown keys reported.

### S8 — Command set, leader prefix, and release build

- **Observable:** the default keymap covers quit (`ctrl+c`, `ctrl+d`, leader+q),
  the command palette (`ctrl+p`), submit (`enter`), interrupt (`escape`), new
  session (leader+n), list movement (arrows), and transcript paging
  (`pageup`/`pagedown`, `home`/`end`); the leader prefix (`ctrl+x`) arms its
  bindings and shows in the footer.
- **Scenario:** "The default keymap covers navigation and the leader prefix".
- **GREEN:** `Command` is the twelve-command upstream-aligned set;
  `Keybinds::resolve` parses the `leader` override; `app::dispatch` is a leader
  state machine; `App` holds `selected`, `scroll`, and `palette` state;
  `component/command_palette.rs` renders `Command::ALL` with its binding and
  description. Performance: `[profile.dev.package."*"] opt-level = 3` and a
  `just release` build (5.3 MB) replace the unoptimized 90 MB debug binary for
  interactive use; `just run` builds and runs the release profile.
- **Note:** upstream has no `session_back`; Escape binds `session_interrupt` and
  the pinned v1 API has no interrupt endpoint, so it returns to the list.
- **Deps:** S4, S6b. **Rollback:** revert to the three-command set; the palette
  is additive.

### S8b — Viewport correctness and surfaced failures

- **Observable:** opening a session or sending a prompt leaves the transcript at
  its true bottom, so the streamed reply and the `> prompt` line are visible; a
  failed action is shown in the footer instead of only logged.
- **GREEN:** `routes::session::render` returns the *wrapped* row count
  (`wrapped_height(line_width, width)`) instead of the source-line count, and
  `app::draw` returns `(total_rows, viewport_height)` so `app::last_scroll`
  clamps `messages_last` / post-send scrolling to the real bottom. `App` gains
  `last_error`, set by the failing helpers and rendered in the footer; any key
  clears it.
- **Checks:** unit tests `wrapped_height_counts_rows_and_never_zero` and
  `last_scroll_keeps_the_viewport_filled`; contract gate stays green.
- **Deps:** S8. **Rollback:** revert the render return to the line count and drop
  the footer error branch.

### S8c — Start screen, native slashes, and a responsive send

- **Observable:** the client opens on a start screen showing the logo, the model
  the server routes to (`GET /config`), and an input line; typing fills the
  prompt and `enter` starts a conversation (creates a session and sends the
  text), while an empty `enter` or `<leader>l` opens the session list. Typing
  `/` lists client-native commands first, then the server's. A prompt is queued
  without waiting for the model turn, so the UI stays live during generation.
- **Scenarios:** "The start screen names the model and offers both entries" plus
  the slash-command coverage in "The default keymap covers navigation and the
  leader prefix".
- **GREEN:** `routes::home::render` draws the logo (`LOGO_LEFT`/`LOGO_RIGHT`,
  mirroring upstream `logo.ts`), the model line, the `Ask anything…` placeholder
  with an example, the session-count hint, and the session list when
  `App::session_list` is set. `Command::SessionList` (`<leader>l`) toggles it;
  `session_back`/`escape` leaves it. `NativeSlash`/`NATIVE_SLASHES` and
  `Command::slash_name` supply the native entries; `app::native_slash_command`
  routes a typed `/name` locally and `submit_prompt` sends anything matching
  `GET /command` through `session.command`, else a normal prompt. The three send
  calls are fire-and-forget: `OpencodeClient::send_prompt`, `send_command`, and
  `abort` build the request and hand it to `spawn_send`, which reports only
  transport/status failures — the reply arrives on the event stream, so a
  long generation no longer freezes the draw loop.
- **Note:** the fix for "super slow and with no option for writing". Awaiting
  `POST /session/{id}/message` (synchronous, 120 s timeout) in the draw loop was
  the slowness; upstream is fire-and-forget (`component/prompt/index.tsx`).
  Upstream's wider `slashName` set (`/models`, `/agents`, `/themes`, ...) is
  omitted where M1 has no screen, rather than advertised as a dead option.
- **Deps:** S8, S8b. **Rollback:** drop `SessionList` and the start-screen
  branch; revert `send_prompt`/`send_command` to awaited calls.

### S8d — Continuous integration and a Nix package (Q4, Q1)

- **Observable:** pushing to `main` runs the whole `just ci` gate on GitHub
  Actions, a failing run pings the Cleytin CI webhook, and a secret-scanning
  workflow pings the security webhook; `nix build .#default` yields a runnable
  `opencode-tui`, and a home-manager module installs it with `OPENCODE_URL`.
- **Scenarios:** the gate, the two notification shapes and the Nix build are
  asserted by `tests/ci-notification-contract.sh`, the homelab
  `ci-notification-contract.sh` / `security-notification-contract.sh`, and
  `nix build` itself. No new `.feature` scenario: the behavior is CI wiring,
  not client behavior.
- **GREEN:** `.github/workflows/ci.yml` (checkout pinned, rustup 1.89.0 with
  rustfmt+clippy, apt `just`, `just ci`, `notify-ci` on failure);
  `.github/workflows/security.yml` (gitleaks 8.24.3 with SHA256 verify,
  `--redact --no-banner`, notify on failure); `tests/ci-notification-contract.sh`
  mirrored as a `notify-contract` recipe inside `just ci`; `flake.nix`
  (flake-parts, `packages.default`/`opencode-tui`, `apps.default`,
  `formatter = alejandra`, dev shell) with `nix/package.nix`
  (`rustPlatform.buildRustPackage`, `cargoLock.lockFile = ../Cargo.lock`,
  `doCheck = false` because the contract test needs a live server) and
  `nix/home-manager.nix` (`programs.opencode-tui.{enable,package,url}`, sets
  `OPENCODE_URL`, never starts a server).
- **Deps:** S8c. **Rollback:** delete the workflows and `nix/`; the flake does
  not publish to FlakeHub, so nothing downstream is broken by removing it.

Frontier Q1–Q4 are now closed: Q4 wired (`ci.yml` + `security.yml` +
`notify-contract`), Q1 packaged (`flake.nix` + `nix/home-manager.nix`, consumed
by `desktop-nixos` as a github input); Q2 (single crate) and Q3 (no permission
answering in M1) keep their defaults.
