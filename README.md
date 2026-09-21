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
- [x] Session transcript with part renderers.
- [x] Prompt submission, start screen, native `/` slash commands and server
      commands, session creation.
- [x] Permission requests rendered read-only.
- [x] Continuous integration (`just ci` on GitHub Actions) and a Nix package
      with a home-manager module.

The behavior contract is
[`features/attach-and-prompt.feature`](features/attach-and-prompt.feature). The
ten offline scenarios run through cucumber-rs against a pinned
`opencode serve` (`just contracts`). The prompt reply needs a model provider, so
that scenario is tagged `@live` and runs only under `just contracts-live`.

## Usage

```sh
opencode serve --port 4096          # in one shell
just run                            # builds release, then runs against OPENCODE_URL
just run url=http://127.0.0.1:4097  # explicit server
./target/release/opencode-tui --url http://127.0.0.1:4096 --dir /some/project
./target/release/opencode-tui --url http://127.0.0.1:4096 --check   # readiness probe
```

`just run` builds `target/release/opencode-tui` (release is far more responsive
than the debug build). `OPENCODE_URL` and `OPENCODE_DIRECTORY` provide the
defaults. The directory scope is sent as an URL-encoded `directory` query
parameter on reads and as the `x-opencode-directory` header on writes, matching
the upstream SDK. Credentials in `--url` userinfo are redacted from errors and
the status line.

### Default keymap

| Command | Binding |
|---|---|
| `app_exit` | `ctrl+c`, `ctrl+d`, `<leader>q` (plus bare `q` on the home screen) |
| `command_list` | `ctrl+p` |
| `input_submit` | `enter` |
| `session_interrupt` | `escape` |
| `session_list` | `<leader>l` |
| `session_new` | `<leader>n` |
| `session_next` / `session_previous` | `down` / `up` |
| `messages_page_up` / `messages_page_down` | `pageup` / `pagedown` |
| `messages_first` / `messages_last` | `home` / `end` |

The leader prefix is `ctrl+x`; while armed it shows in the footer and the next
key resolves a leader binding. `session_back` exists as an overridable name but
is unbound by default, because upstream binds Escape to `session_interrupt`.

### Start screen

The client opens on the start screen, mirroring upstream's home route: the logo,
the model the server routes to by default (`litellm/deepseek-v4.1-flash` on this
fleet, read from `GET /config`), and an input line.

- Type and press `enter` to start a conversation — the client creates a session
  and sends what you typed into it.
- Press `enter` on an empty line, or `<leader>l`, to browse the session list;
  `up`/`down` move the selection and `enter` opens one. `escape` goes back.
- Type `/` to open the slash-command list: client-native entries first
  (`/exit`, `/new`, `/sessions`, `/help`), then the server's commands
  (`/init`, `/review`, `/codehero`, ...). Submit one with `enter`; it runs as a
  server command rather than a prompt.

Keybindings come from `$OPENCODE_CONFIG_DIR/tui.json` (default
`~/.config/opencode/tui.json`), using the upstream binding-string form:

```json
{ "keybinds": { "app_exit": "ctrl+d,q", "session_interrupt": "none", "leader": "ctrl+a" } }
```

Unknown or malformed keys are reported and ignored. A missing or unreadable
file falls back to the built-in defaults.

## Development

Everything runs inside the declared environment:

```sh
devenv shell
just ci          # fmt --check + clippy -D warnings + test + feature contracts + notify contract
just run
```

`just reference` fetches the pinned upstream client
(`anomalyco/opencode@v1.18.30`, commit
`3104c1428ec91f809e5ab86631300de41eb6952e`) into the gitignored
`reference/opencode` as a sparse checkout. It is read for interface fidelity
only and is never a build dependency.

`just ci` runs the `notify-contract` recipe too: it asserts the CI workflow
still carries the Discord webhook, the Cleytin mention and the
`allowed_mentions` shape the homelab `ci-notification-contract.sh` requires.

### Packaging

The flake builds the binary and exposes a home-manager module:

```sh
nix build .#default
nix run .#default -- --url http://127.0.0.1:4096
```

```nix
programs.opencode-tui = {
  enable = true;
  url = "http://127.0.0.1:4096";   # exported as OPENCODE_URL
};
```

The module installs the binary and sets `OPENCODE_URL`; it attaches to a
server that is already running and never starts or manages one. The flake is
consumed by `desktop-nixos` as a `github:ErikBPF/opencode-tui` input; it is not
published to FlakeHub.

### Continuous integration

`.github/workflows/ci.yml` runs `just ci` on pushes to `main` and on pull
requests, and pings the Cleytin CI webhook when the gate fails.
`.github/workflows/security.yml` runs gitleaks and pings the security webhook
when it finds a committed secret.

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
