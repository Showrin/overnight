# Graph Report - overnight  (2026-08-22)

## Corpus Check
- Corpus is ~21,029 words - fits in a single context window. You may not need a graph.

## Summary
- 443 nodes · 698 edges · 34 communities (28 shown, 6 thin omitted)
- Extraction: 95% EXTRACTED · 5% INFERRED · 0% AMBIGUOUS · INFERRED: 34 edges (avg confidence: 0.79)
- Token cost: 71,505 input · 0 output

## Community Hubs (Navigation)
- Jira Integration (client + DB)
- Frontend UI Components
- Metrics & Data Models
- Frontend Dependencies
- Build Tooling & Dev Dependencies
- App TypeScript Config
- Tauri App Configuration
- shadcn Component Config
- Tauri Command Handlers
- Node TypeScript Config
- DB Migrations & Settings
- Session Persistence
- Task Persistence
- Tauri Permission Capabilities
- AgentProvider Architecture
- Activity Log Persistence
- Oxlint Configuration
- DB Pool Initialization
- DB Error Type
- Jira Error Type
- App Entry & Shell
- Jira Credential Storage
- Root TypeScript Config
- README Tooling Notes
- Design Tokens
- Module Declarations
- Vite Config
- Activity Table Schema
- Container Metrics Table Schema
- Metrics Table Schema
- Settings Table Schema

## God Nodes (most connected - your core abstractions)
1. `test_conn()` - 19 edges
2. `compilerOptions` - 19 edges
3. `compilerOptions` - 15 edges
4. `Session` - 13 edges
5. `cn()` - 13 edges
6. `Task` - 12 edges
7. `create()` - 12 edges
8. `now_millis()` - 10 edges
9. `create()` - 10 edges
10. `IssueFields` - 10 edges

## Surprising Connections (you probably didn't know these)
- `Overnight app HTML shell (index.html)` --conceptually_related_to--> `Overnight (Tauri desktop app)`  [INFERRED]
  index.html → ARCHITECTURE.md
- `append()` --calls--> `new_id()`  [INFERRED]
  src-tauri/src/db/activity.rs → src-tauri/src/db/models.rs
- `append()` --calls--> `now_millis()`  [INFERRED]
  src-tauri/src/db/activity.rs → src-tauri/src/db/models.rs
- `append_and_list_in_order()` --calls--> `test_conn()`  [INFERRED]
  src-tauri/src/db/activity.rs → src-tauri/src/db/migrations.rs
- `deleting_session_cascades_to_activity()` --calls--> `test_conn()`  [INFERRED]
  src-tauri/src/db/activity.rs → src-tauri/src/db/migrations.rs

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **AgentProvider abstraction: trait, session handle, event stream, and first implementation** — architecture_agentprovider, architecture_claudecodeprovider, architecture_sessionhandle, architecture_agentevent [INFERRED 0.85]
- **SQLite schema v1: tasks, sessions, activity, metrics, container_metrics, settings** — architecture_tasks_table, architecture_sessions_table, architecture_activity_table, architecture_metrics_table, architecture_container_metrics_table, architecture_settings_table [EXTRACTED 1.00]

## Communities (34 total, 6 thin omitted)

### Community 0 - "Jira Integration (client + DB)"
Cohesion: 0.09
Nodes (32): Client, Map, Self, issue(), list(), resync_is_idempotent_and_updates_in_place(), row_to_issue(), Connection (+24 more)

### Community 1 - "Frontend UI Components"
Cohesion: 0.12
Nodes (23): react, App(), JiraConfig, JiraConfigForm(), JiraIssue, JiraIssueList(), handleSync(), loadIssues() (+15 more)

### Community 2 - "Metrics & Data Models"
Cohesion: 0.13
Nodes (29): list_for_session(), make_session(), record(), record_and_list(), row_to_container_metric(), Connection, Result, Row (+21 more)

### Community 3 - "Frontend Dependencies"
Cohesion: 0.07
Nodes (29): class-variance-authority, clsx, @fontsource/jetbrains-mono, @fontsource-variable/inter, lucide-react, dependencies, class-variance-authority, clsx (+21 more)

### Community 4 - "Build Tooling & Dev Dependencies"
Cohesion: 0.07
Nodes (28): oxlint, devDependencies, oxlint, shadcn, @tauri-apps/cli, @types/node, @types/react, @types/react-dom (+20 more)

### Community 5 - "App TypeScript Config"
Cohesion: 0.08
Nodes (24): DOM, src, vite/client, compilerOptions, allowArbitraryExtensions, allowImportingTsExtensions, erasableSyntaxOnly, jsx (+16 more)

### Community 6 - "Tauri App Configuration"
Cohesion: 0.08
Nodes (24): icons/128x128@2x.png, icons/128x128.png, icons/32x32.png, icons/icon.icns, icons/icon.ico, debugApplicationIdSuffix, app, security (+16 more)

### Community 7 - "shadcn Component Config"
Cohesion: 0.09
Nodes (21): aliases, components, hooks, lib, ui, utils, iconLibrary, menuAccent (+13 more)

### Community 8 - "Tauri Command Handlers"
Cohesion: 0.33
Nodes (21): create_task(), delete_task(), end_session(), get_jira_config(), get_session(), get_task(), JiraConfig, list_jira_issues() (+13 more)

### Community 9 - "Node TypeScript Config"
Cohesion: 0.10
Nodes (19): node, vite.config.ts, compilerOptions, allowImportingTsExtensions, erasableSyntaxOnly, lib, module, moduleDetection (+11 more)

### Community 10 - "DB Migrations & Settings"
Cohesion: 0.18
Nodes (18): migrations(), migrations_apply_cleanly(), Connection, test_conn(), get(), get_json(), get_missing_returns_none(), json_roundtrip() (+10 more)

### Community 11 - "Session Persistence"
Cohesion: 0.31
Nodes (17): Session, create(), create_get_list_end(), deleting_task_cascades_to_sessions(), end(), get(), list_for_task(), make_task() (+9 more)

### Community 12 - "Task Persistence"
Cohesion: 0.31
Nodes (15): now_millis(), Task, create(), create_get_list_update_delete(), delete(), duplicate_jira_key_rejected(), get(), list() (+7 more)

### Community 13 - "Tauri Permission Capabilities"
Cohesion: 0.13
Nodes (14): core:default, core:window:allow-close, core:window:allow-maximize, core:window:allow-minimize, core:window:allow-start-dragging, core:window:allow-toggle-maximize, core:window:allow-unmaximize, main (+6 more)

### Community 14 - "AgentProvider Architecture"
Cohesion: 0.14
Nodes (14): AgentEvent (normalized event enum), AgentProvider trait (ports-and-adapters abstraction), ClaudeCodeProvider, Codex (future agent provider, non-goal of feature parity), OVN-53 (AgentProvider implementation ticket), OVN-55 (Settings / project_folders registry ticket), Ports-and-adapters design rationale, Sandbox parameterization per-provider (forward-looking) (+6 more)

### Community 15 - "Activity Log Persistence"
Cohesion: 0.32
Nodes (12): append(), append_and_list_in_order(), deleting_session_cascades_to_activity(), list_for_session(), make_session(), row_to_activity(), Connection, Result (+4 more)

### Community 16 - "Oxlint Configuration"
Cohesion: 0.22
Nodes (8): plugins, rules, react/only-export-components, react/rules-of-hooks, $schema, oxc, typescript, warn

### Community 17 - "DB Pool Initialization"
Cohesion: 0.52
Nodes (6): AppHandle, PathBuf, db_path(), init_pool(), DbPool, Result

### Community 18 - "DB Error Type"
Cohesion: 0.29
Nodes (5): Error, Ok, Result, S, Serialize

### Community 19 - "Jira Error Type"
Cohesion: 0.29
Nodes (5): Error, Ok, Result, S, Serialize

### Community 20 - "App Entry & Shell"
Cohesion: 0.33
Nodes (6): Overnight (Tauri desktop app), Tauri v2, Overnight app HTML shell (index.html), src/main.tsx (frontend entry point), src-tauri/src/db/error.rs (db::Error), src-tauri/src/lib.rs (app bootstrap)

### Community 21 - "Jira Credential Storage"
Cohesion: 0.47
Nodes (5): get_token(), has_token(), Result, String, store_token()

### Community 22 - "Root TypeScript Config"
Cohesion: 0.40
Nodes (4): compilerOptions, paths, files, references

### Community 23 - "README Tooling Notes"
Cohesion: 0.50
Nodes (4): Oxlint, React Compiler, @vitejs/plugin-react (Oxc-based), @vitejs/plugin-react-swc (SWC-based)

### Community 24 - "Design Tokens"
Cohesion: 0.67
Nodes (3): Design tokens (colors, spacing, radius, typography, elevation), OVN-58 (design tokens ticket), src/index.css (design tokens)

## Knowledge Gaps
- **148 isolated node(s):** `$schema`, `typescript`, `oxc`, `react/rules-of-hooks`, `warn` (+143 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **6 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `test_conn()` connect `DB Migrations & Settings` to `Jira Integration (client + DB)`, `Metrics & Data Models`, `Session Persistence`, `Task Persistence`, `Activity Log Persistence`?**
  _High betweenness centrality (0.025) - this node is a cross-community bridge._
- **Why does `now_millis()` connect `Task Persistence` to `Jira Integration (client + DB)`, `Metrics & Data Models`, `DB Migrations & Settings`, `Session Persistence`, `Activity Log Persistence`?**
  _High betweenness centrality (0.023) - this node is a cross-community bridge._
- **Are the 15 inferred relationships involving `test_conn()` (e.g. with `append_and_list_in_order()` and `deleting_session_cascades_to_activity()`) actually correct?**
  _`test_conn()` has 15 INFERRED edges - model-reasoned connections that need verification._
- **What connects `$schema`, `typescript`, `oxc` to the rest of the system?**
  _148 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Jira Integration (client + DB)` be split into smaller, more focused modules?**
  _Cohesion score 0.09146341463414634 - nodes in this community are weakly interconnected._
- **Should `Frontend UI Components` be split into smaller, more focused modules?**
  _Cohesion score 0.11586452762923351 - nodes in this community are weakly interconnected._
- **Should `Metrics & Data Models` be split into smaller, more focused modules?**
  _Cohesion score 0.12701612903225806 - nodes in this community are weakly interconnected._