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
- **GREEN:** add `context/prompt.rs` (mirrors upstream `context/prompt.tsx`),
  `component/prompt.rs` (input line), `sdk::prompt(session_id, text)`; wire the
  session screen and keymap (`Enter` submits, `Esc` returns).
- **Checks:** `just ci`; live test asserting a `pong` part arrives within a
  bounded wait; the reply must be built from the event stream, not from the POST
  response.
- **Deps:** S3. **Rollback:** hide the prompt line; the POST is additive.

### S5 — Permission requests are surfaced, not answered

- **Observable:** a `permission.asked` event renders the request and its options;
  the client does not call the reply endpoint.
- **Scenario:** "A permission request is surfaced but not answered".
- **RED:** reducer test that a fixture `permission.asked` sets a pending
  permission; a render test shows its options. Command: `just test`. Expected
  failure: the event is decoded but dropped.
- **GREEN:** add `Store.pending_permission` and a read-only banner on the session
  screen. No reply action (frontier Q3).
- **Checks:** `just ci`; live test that a denied tool shows the banner and the
  session continues.
- **Deps:** S3. **Rollback:** remove the banner; the store field is additive.

### S6 — Directory scope and CLI/config polish

- **Observable:** `--dir "/a path/with spaces"` reaches the server; the footer
  shows the abbreviated path; `-h` prints usage.
- **Scenario:** "The directory scope is carried and encoded".
- **RED:** unit tests already added in S1 cover encoding; add a `util` test for
  `abbreviate_home`. Command: `just test`.
- **GREEN:** already shipped; tests close the gap. Optional: read
  `~/.config/opencode/tui.json` keybind overrides (deferred unless requested).
- **Deps:** S1. **Rollback:** none, additive.

### S7 — Bind the behavior contract

- **Observable:** `just contracts` runs the `.feature` scenarios against a pinned
  `opencode serve` and they pass; the binding was observed failing before S3–S5.
- **RED:** add the `cucumber` dev-dependency, steps in `tests/`, and a harness
  that starts a pinned `opencode serve` on a throwaway `OPENCODE_DB`. Run
  `just contracts` against the S1 skeleton — scenarios for transcript, prompt and
  permission must fail. This RED is the milestone's acceptance.
- **GREEN:** no production change; S3–S5 make the scenarios pass.
- **Checks:** `just ci` (contracts included). Remove the "unautomated contract"
  header from the `.feature` in the same change.
- **Deps:** S4, S5. **Rollback:** keep the shell binding from `tests/contract.sh`
  as the fallback so the gate still validates the file.

## Rollout and rollback

- All work lands on `opencode-tui` `main` as conventional commits; leaf-first is
  not applicable (single crate), so the order is slice order.
- No live target is authorized beyond a local/pinned `opencode serve`; the opt-in
  live tests default to skipped.
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

`/ip` slices: **S1 and S2 are implemented**: `Args` is parsed by a pure
`parse(argv, url_env, dir_env)` with five unit tests; the live test is
opt-in via `just live`; and `app::spawn_event_stream` is exercised by a stub
TCP/SSE server test that closes the stream and observes the second
`server.connected` after the 1s backoff (20 tests, 19 passing + 1 ignored).
Remaining: S3 transcript, S4 prompt, S5 read-only permission, S6 CLI/config
polish, S7 cucumber binding. Frontier Q1–Q4 remain open with defaults in use:
devenv + justfile only; single crate; no permission answering in M1; GitHub
Actions CI.
