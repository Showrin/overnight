# Architecture

This document is the reference every subsequent phase (2.x, 3.x, 5.x) should
be implemented against. Any ticket touching agent invocation, session
lifecycle, plan capture, or usage/token reporting should be implemented
against this doc, not re-derive the layering independently.

## Overview

Overnight is a Tauri v2 desktop app: a React/TypeScript frontend talking to a
Rust backend over Tauri's IPC layer. The backend persists to a local SQLite
database (via `rusqlite`) and will drive one or more coding-agent CLI
subprocesses (Claude Code first, others later) behind a common abstraction.

## Module layout

```
src/                      # React frontend
  components/
    ui/                    # shadcn/ui primitives
    dashboard/             # Jira issue list (dashboard screen)
    projects/              # Projects screen: list, create/edit form
    settings/              # Settings screen: permission mode, Jira config form
    sidebar.tsx            # left nav (Dashboard / Projects / Settings)
  assets/                  # logo SVGs etc.
  index.css                # design tokens (see below)

src-tauri/src/
  lib.rs                   # app bootstrap: plugins, tray, window events,
                            #   invoke_handler, db pool init + startup diagnostics
  commands.rs               # #[tauri::command] thin wrappers over db::*
  db/
    mod.rs                  # connection pool builder, app_data_dir resolution
    error.rs                 # shared db::Error (crosses Tauri IPC via Serialize)
    migrations.rs             # schema (rusqlite_migration), test_conn() helper
    models.rs
    tasks.rs / sessions.rs / activity.rs / metrics.rs / container_metrics.rs / settings.rs / projects.rs
  providers/                # not yet built (OVN-53) — see below
    claude_code/
```

**Hard rule**: Claude-specific detail — CLI flags, `stream-json` parsing,
session-file conventions — must stay inside `providers/claude_code/`. It must
never leak into `db/` or `commands.rs`. Those two stay agent-agnostic.

## Data model

Schema v1 (SQLite, via `rusqlite` + `rusqlite_migration`, see
`src-tauri/src/db/migrations.rs`):

- **`tasks`** — title/description/status, optional `jira_key` (unique when
  present), optional `project_path` (free text placeholder — see note below),
  and a `metadata` JSON catch-all for provider-specific fields (e.g. Jira
  assignee/labels) that don't yet need their own column.
- **`sessions`** — one row per agent run against a task. `agent_provider`
  (`TEXT NOT NULL`) records which backend ran it, from day one — this is
  the column the AgentProvider abstraction below depends on existing.
  `provider_session_id` holds the backend's own session/conversation id,
  needed for `resume()`. `parent_session_id` self-references the row a
  resumed session continues from, so history/metrics stay per-run rather
  than being overwritten in place. `mode` is `plan` or `autonomous`,
  matching `launch_plan_session`/`launch_autonomous_session` below exactly.
- **`activity`** — timestamped event log per session (`event_type` +
  JSON `payload`), the backbone for the live chat/transcript view.
- **`metrics`** — token/cost accounting per session, with `model` per row
  since accounting is per-model.
- **`container_metrics`** — sandbox resource usage (cpu/memory) over time,
  for autonomous-mode sessions running in a container.
- **`settings`** — generic key/JSON-value store.
- **`projects`** (OVN-55, schema v2) — user-managed local projects: `name`,
  `repo_path` (validated as an existing git repo — `.git` marker check — on
  create/update), optional `plans_path` (defaults to `.agent/plans` under
  `repo_path` when unset, resolved in application code, not SQL), optional
  per-project `dev_server_port`, and `extra_clone_paths` (JSON array of glob
  patterns for extra files/folders to copy into a clone of the project — not
  restricted to gitignored files). No GitHub/Bitbucket OAuth — local path
  only.

All timestamps are unix-epoch-milliseconds integers; all primary keys are
UUIDv4 text. Foreign keys cascade on delete.

**Note on `tasks.project_path`**: superseded by `tasks.project_id`, a
nullable FK to `projects` (`ON DELETE SET NULL`, same convention as
`sessions.parent_session_id`). The old free-text column is left in place
non-destructively — not backfilled or dropped — since no production data
depended on it.

## AgentProvider abstraction

Not implemented yet — this is forward design guidance for OVN-53, written
now so the schema and every future ticket can be built against a stable
target instead of re-deriving the layering piecemeal.

The idea is ports-and-adapters: a stable Rust trait abstracts "coding agent
backend" so the rest of the app (chat UI, plan/transcript capture, token
tracking, sandbox orchestration) never talks to a specific agent CLI
directly.

```rust
trait AgentProvider {
    fn launch_plan_session(&self, /* ... */) -> Result<SessionHandle>;
    fn launch_autonomous_session(&self, /* ... */) -> Result<SessionHandle>;
    fn stream_events(&self, handle: &SessionHandle) -> impl Stream<Item = AgentEvent>;
    fn stop(&self, handle: &SessionHandle) -> Result<()>;
    fn resume(&self, provider_session_id: &str, /* ... */) -> Result<SessionHandle>;
    fn capture_plan(&self, handle: &SessionHandle) -> Result<Plan>;
    fn get_usage(&self, handle: &SessionHandle) -> Result<Usage>;
    fn supports(&self, capability: Capability) -> bool;
}
```

- `SessionHandle` is opaque to callers — it wraps whatever the provider
  needs internally (process handle, provider session id, etc.), and is what
  gets persisted as `sessions.provider_session_id` for later `resume()`.
- `AgentEvent` is a normalized event enum — providers translate their own
  wire format (e.g. Claude Code's `stream-json`) into this before it ever
  reaches the frontend.
- `ClaudeCodeProvider` (in `providers/claude_code/`) is the first concrete
  implementation, translating Claude Code's CLI flags, `stream-json`
  output, and session-file conventions into the interface above.
- **Non-goal**: future providers (e.g. Codex) are not assumed to have
  feature parity with Claude Code. Plan-mode equivalence, resume semantics,
  and rate-limit event structure may differ. `supports(capability)` exists
  specifically so the UI can adapt rather than assume identical behavior —
  mapping a new provider's actual capabilities is scoped as future work
  when that provider is added, not part of any ticket building this trait.

### Sandbox parameterization (forward-looking)

Sandbox/container tickets are a later phase, not yet built. When they land,
image builds should be parameterized per-provider — different CLIs, auth
mechanisms, and potentially different firewall allowlist domains per
provider — rather than assuming one fixed sandbox image works for every
agent backend.

## Design tokens

Design tokens (colors, spacing, radius, typography, elevation via border
contrast rather than shadows) live in `src/index.css`, formalized in OVN-58.
See the comment block at the top of that file for the icon convention and
elevation rules.
