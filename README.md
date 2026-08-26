# Overnight

Overnight is a desktop app for running autonomous coding-agent sessions
(starting with Claude Code) against your own projects — plan a change, hand
it off, and let the agent work in an isolated sandbox.

Built with [Tauri v2](https://tauri.app) — a React/TypeScript frontend and a
Rust backend, packaged as a native desktop app.

## Status

Early-stage / actively developed. Expect breaking changes, incomplete
features, and rough edges. See [ARCHITECTURE.md](./ARCHITECTURE.md) for the
current module layout, data model, and forward design notes (e.g. the
`AgentProvider` abstraction that isn't fully built out yet).

## Tech stack

- **Frontend**: React 19, TypeScript, Vite, Tailwind CSS v4, shadcn/ui
  (Radix primitives), Zustand for state.
- **Backend**: Rust, Tauri v2, SQLite via `rusqlite` (+ `rusqlite_migration`
  for schema migrations), `tokio` for async process/IO work.
- **Package manager**: [pnpm](https://pnpm.io).

## Prerequisites

1. **Node.js** 20+ and [pnpm](https://pnpm.io/installation).
2. **Rust** (stable toolchain) — see [rustup.rs](https://rustup.rs).
3. Tauri's platform-specific prerequisites — follow the official
   [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/)
   for your OS (e.g. WebView2 on Windows — usually already present, Xcode
   Command Line Tools on macOS, `webkit2gtk` and friends on Linux).
4. **Docker** and the `sbx` CLI (Docker Sandboxes) on your `PATH` — required
   for the Sandboxes feature, which runs autonomous agent sessions in
   isolated containers. Not required for local plan-mode-only usage.
5. An Anthropic API key if you want to actually run Claude Code sessions
   (entered in-app; stored via the OS credential store through the `keyring`
   crate — Windows Credential Manager on Windows).

## Getting started

```bash
pnpm install
pnpm tauri dev
```

This starts the Vite dev server and launches the Tauri window with hot
reload for the frontend. First run will also compile the Rust backend,
which can take a few minutes.

To build a release binary:

```bash
pnpm tauri build
```

## Available scripts

Run from the repo root (these drive the frontend only — prefer `pnpm tauri
dev` / `pnpm tauri build` above for the full app):

| Script          | Description                                  |
| ---------------- | --------------------------------------------- |
| `pnpm dev`        | Vite dev server only (no Tauri window).      |
| `pnpm build`       | Type-check (`tsc -b`) and build the frontend. |
| `pnpm lint`        | Lint with [oxlint](https://oxc.rs).           |
| `pnpm preview`      | Preview the built frontend.                  |

For the Rust side, standard Cargo commands work from `src-tauri/`:
`cargo check`, `cargo test`, `cargo clippy`.

## Project structure

```
src/                      # React frontend
  components/
    ui/                    # shadcn/ui primitives
    projects/               # Projects screen
    sandboxes/               # Sandboxes screen + PC stats panel
    settings/                # Settings screen
    sidebar.tsx               # left nav
  assets/                    # logo/wordmark SVGs
  store/                      # Zustand store (shared app state, polling)
  index.css                    # design tokens — see the comment block at
                                #   the top for the icon/elevation conventions

src-tauri/src/
  lib.rs                     # app bootstrap: plugins, tray, window setup,
                              #   invoke_handler, db pool init
  commands.rs                 # #[tauri::command] handlers
  db/                          # SQLite pool, migrations, per-entity queries
  providers/                    # coding-agent backends (Claude Code first)
  sbx/                           # sandbox orchestration (sbx CLI wrapper)
  jira/                           # Jira Cloud REST client + credential storage
  process/                         # subprocess helpers
```

See [ARCHITECTURE.md](./ARCHITECTURE.md) for the full data model and design
rationale — read it before touching agent invocation, session lifecycle,
plan capture, or usage/token reporting.

## Contributing

Issues and pull requests are welcome. A few conventions this repo follows:

- Commit messages are a single imperative line (e.g. `Add tasks repository
  with tests`) — no trailers.
- Keep PRs scoped; prefer several small, reviewable commits over one large
  one where the work naturally splits into steps.
- Match existing code style — no linter/formatter opinions beyond `oxlint`
  are enforced yet, so follow the surrounding file.
- If you're changing the schema, agent invocation, or session lifecycle,
  read [ARCHITECTURE.md](./ARCHITECTURE.md) first — those areas have
  forward-looking design constraints (e.g. keeping Claude-specific logic
  confined to `providers/claude_code/`) that aren't obvious from the code
  alone.

There's no CI configured yet — please run `pnpm build`, `pnpm lint`, and
`cargo check` (from `src-tauri/`) locally before opening a PR.

## License

No license has been chosen for this project yet. Until one is added, all
rights are reserved by default — please open an issue if you'd like to
discuss licensing before contributing significant work.
