# opencode-tui

A Rust + [ratatui](https://ratatui.rs) terminal client for the
[opencode](https://opencode.ai) server REST + SSE API. It is a second,
native front-end for the same server the upstream TypeScript/SolidJS TUI
talks to: point it at `opencode serve` and it reads sessions and streams
events without the Bun runtime.

## Status

First milestone (M1) tracer bullet: **attach + read + prompt**.

- [x] Repository scaffold mirroring the upstream `packages/tui/src` module
      tree (see [Fidelity](#fidelity) for what M1 ports and what it defers).
- [x] Connect to a server, load the session list, hold `/global/event`,
      render the home screen.
- [ ] Session transcript with part renderers.
- [ ] Prompt submission.
- [ ] Permission requests rendered read-only.

The behavior contract is
[`features/attach-and-prompt.feature`](features/attach-and-prompt.feature). Its
scenarios are an **unautomated contract** until bound to cucumber-rs steps and
observed failing against the skeleton.

## Usage

```sh
opencode serve --port 4096          # in one shell
just run                            # or: cargo run -- --url http://127.0.0.1:4096 --dir /some/project
```

`OPENCODE_URL` and `OPENCODE_DIRECTORY` provide the defaults. The directory
scope is sent as an URL-encoded `directory` query parameter on reads and as the
`x-opencode-directory` header on writes, matching the upstream SDK. Credentials
in `--url` userinfo are redacted from errors and the status line.

## Development

Everything runs inside the declared environment:

```sh
devenv shell
just ci          # fmt --check + clippy -D warnings + test + feature contracts
just run
```

`just reference` fetches the pinned upstream client
(`anomalyco/opencode@v1.18.30`, commit
`3104c1428ec91f809e5ab86631300de41eb6952e`) into the gitignored
`reference/opencode` as a sparse checkout. It is read for interface fidelity
only and is never a build dependency.

## Fidelity

The client is a 1:1 port of the upstream TUI's module structure and seams,
rewritten in Rust and ratatui. Load-bearing seams and their upstream
counterparts:

| This crate | Upstream `packages/tui/src` |
|---|---|
| `context/sdk.rs` | `context/sdk.tsx` — client, directory scope, SSE |
| `context/event.rs` | `context/event.ts` — typed event dispatch |
| `context/sync.rs` | `context/sync.tsx` — `loading\|partial\|complete` store |
| `context/route.rs` | `context/route.tsx` |
| `context/theme.rs`, `theme/` | `context/theme.tsx`, `theme/` |
| `config/keybind.rs`, `keymap.rs` | `config/keybind.ts`, `keymap.tsx` |
| `routes/home.rs` | `routes/home.tsx` |
| `routes/session.rs` | `routes/session/index.tsx` |
| `ui/`, `component/` | `ui/`, `component/` |

M1 deliberately defers the upstream `plugin/` and `feature-plugins/` runtime,
`context/data.tsx` (the experimental `/api/*` store), and the theme asset set.
The transport targets the **v1** REST surface; `@opencode-ai/sdk/v2` in
upstream is the SDK generation directory, not API v2, and its `client.v2.*`
namespace is out of scope.

## License

Apache-2.0.
