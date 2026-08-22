# Graph Report - overnight  (2026-08-22)

## Corpus Check
- 23 files · ~23,500 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 513 nodes · 898 edges · 37 communities (26 shown, 11 thin omitted)
- Extraction: 96% EXTRACTED · 4% INFERRED · 0% AMBIGUOUS · INFERRED: 40 edges (avg confidence: 0.79)
- Token cost: 60,000 input · 11,076 output

## Community Hubs (Navigation)
- Frontend UI Components
- Linting & Dev Tooling
- Sessions & Container Metrics
- Frontend Dependencies
- Tauri Command Layer
- Jira Integration
- App TypeScript Config
- Tauri App Config
- Architecture Documentation
- shadcn Component Config
- Settings & Migrations
- Node/Vite TS Config
- Projects Repository
- Jira Wire Types
- Tasks Repository
- Tauri Capabilities
- Metrics Repository
- Activity Repository
- DB Error Handling
- DB Connection Pool
- Jira Error Handling
- Jira Credentials Storage
- Root TypeScript Config
- Design Tokens Doc
- App Entry Point
- Vite Config
- Activity Table (doc)
- Container Metrics Table (doc)
- Metrics Table (doc)
- Settings Table (doc)
- Jira Issue Type
- DB Error Serialize Helper
- DB Error Serialize Helper
- DB Error Serialize Helper

## God Nodes (most connected - your core abstractions)
1. `test_conn()` - 22 edges
2. `compilerOptions` - 19 edges
3. `cn()` - 15 edges
4. `compilerOptions` - 15 edges
5. `react` - 14 edges
6. `Session` - 13 edges
7. `create()` - 12 edges
8. `Task` - 12 edges
9. `now_millis()` - 12 edges
10. `create()` - 11 edges

## Surprising Connections (you probably didn't know these)
- `@vitejs/plugin-react (Oxc-based)` --conceptually_related_to--> `oxlint`  [INFERRED]
  README.md → package.json
- `resync_is_idempotent_and_updates_in_place()` --calls--> `test_conn()`  [INFERRED]
  src-tauri/src/db/jira_issues.rs → src-tauri/src/db/migrations.rs
- `upsert_many()` --calls--> `now_millis()`  [INFERRED]
  src-tauri/src/db/jira_issues.rs → src-tauri/src/db/models.rs
- `upsert_then_list()` --calls--> `test_conn()`  [INFERRED]
  src-tauri/src/db/jira_issues.rs → src-tauri/src/db/migrations.rs
- `get_missing_returns_none()` --calls--> `test_conn()`  [INFERRED]
  src-tauri/src/db/settings.rs → src-tauri/src/db/migrations.rs

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Schema v1 SQLite Data Model Tables** — architecture_tasks_table, architecture_sessions_table, architecture_activity_table, architecture_metrics_table, architecture_container_metrics_table, architecture_settings_table, architecture_projects_table [EXTRACTED 1.00]
- **AgentProvider Abstraction Components** — architecture_agent_provider_trait, architecture_session_handle, architecture_agent_event, architecture_claude_code_provider, architecture_capability [EXTRACTED 1.00]
- **Rust Backend Module Layout (agent-agnostic core vs provider)** — architecture_src_tauri_lib_rs, architecture_src_tauri_commands_rs, architecture_src_tauri_db_module, architecture_providers_claude_code_module [EXTRACTED 1.00]

## Communities (37 total, 11 thin omitted)

### Community 0 - "Frontend UI Components"
Cohesion: 0.07
Nodes (37): react, App(), JiraConfig, JiraIssue, JiraIssueList(), handleSync(), loadIssues(), LogoWordmark() (+29 more)

### Community 1 - "Linting & Dev Tooling"
Cohesion: 0.05
Nodes (38): oxlint, plugins, rules, react/only-export-components, react/rules-of-hooks, $schema, devDependencies, oxlint (+30 more)

### Community 2 - "Sessions & Container Metrics"
Cohesion: 0.12
Nodes (36): list_for_session(), make_session(), record(), record_and_list(), row_to_container_metric(), Connection, Result, Row (+28 more)

### Community 3 - "Frontend Dependencies"
Cohesion: 0.06
Nodes (31): class-variance-authority, clsx, @fontsource/jetbrains-mono, @fontsource-variable/inter, lucide-react, dependencies, class-variance-authority, clsx (+23 more)

### Community 4 - "Tauri Command Layer"
Cohesion: 0.25
Nodes (30): JiraIssue, AppSettings, create_project(), create_task(), delete_project(), delete_task(), end_session(), get_jira_config() (+22 more)

### Community 5 - "Jira Integration"
Cohesion: 0.13
Nodes (21): Client, Issue, Self, issue(), list(), resync_is_idempotent_and_updates_in_place(), row_to_issue(), Connection (+13 more)

### Community 6 - "App TypeScript Config"
Cohesion: 0.08
Nodes (24): DOM, src, vite/client, compilerOptions, allowArbitraryExtensions, allowImportingTsExtensions, erasableSyntaxOnly, jsx (+16 more)

### Community 7 - "Tauri App Config"
Cohesion: 0.08
Nodes (24): icons/128x128@2x.png, icons/128x128.png, icons/32x32.png, icons/icon.icns, icons/icon.ico, debugApplicationIdSuffix, app, security (+16 more)

### Community 8 - "Architecture Documentation"
Cohesion: 0.10
Nodes (24): AgentEvent, AgentProvider Trait Abstraction, Capability (supports check), ClaudeCodeProvider, Hard Rule: Claude-specific detail stays in providers/claude_code, Overnight, OVN-53 (AgentProvider ticket), OVN-55 (projects schema v2 ticket) (+16 more)

### Community 9 - "shadcn Component Config"
Cohesion: 0.09
Nodes (21): aliases, components, hooks, lib, ui, utils, iconLibrary, menuAccent (+13 more)

### Community 10 - "Settings & Migrations"
Cohesion: 0.17
Nodes (19): migrations(), migrations_apply_cleanly(), Connection, test_conn(), missing_id_operations_return_not_found(), get(), get_json(), get_missing_returns_none() (+11 more)

### Community 11 - "Node/Vite TS Config"
Cohesion: 0.10
Nodes (19): node, vite.config.ts, compilerOptions, allowImportingTsExtensions, erasableSyntaxOnly, lib, module, moduleDetection (+11 more)

### Community 12 - "Projects Repository"
Cohesion: 0.32
Nodes (16): create(), create_get_list_update_delete(), delete(), deleting_project_nulls_task_project_id(), get(), list(), row_to_project(), Connection (+8 more)

### Community 13 - "Jira Wire Types"
Cohesion: 0.26
Nodes (13): Map, AssigneeField, Issue, IssueFields, IssueTypeField, PriorityField, Option, String (+5 more)

### Community 14 - "Tasks Repository"
Cohesion: 0.33
Nodes (14): Task, create(), create_get_list_update_delete(), delete(), duplicate_jira_key_rejected(), get(), list(), row_to_task() (+6 more)

### Community 15 - "Tauri Capabilities"
Cohesion: 0.14
Nodes (13): core:default, core:window:allow-close, core:window:allow-maximize, core:window:allow-minimize, core:window:allow-start-dragging, dialog:allow-open, main, notification:default (+5 more)

### Community 16 - "Metrics Repository"
Cohesion: 0.29
Nodes (13): list_for_session(), make_session(), record(), record_and_sum(), row_to_metric(), Connection, Option, Result (+5 more)

### Community 17 - "Activity Repository"
Cohesion: 0.32
Nodes (12): append(), append_and_list_in_order(), deleting_session_cascades_to_activity(), list_for_session(), make_session(), row_to_activity(), Connection, Result (+4 more)

### Community 18 - "DB Error Handling"
Cohesion: 0.25
Nodes (6): Ok, S, Serialize, Error, Result, String

### Community 19 - "DB Connection Pool"
Cohesion: 0.52
Nodes (6): AppHandle, PathBuf, db_path(), init_pool(), DbPool, Result

### Community 20 - "Jira Error Handling"
Cohesion: 0.29
Nodes (5): Error, Ok, Result, S, Serialize

### Community 21 - "Jira Credentials Storage"
Cohesion: 0.47
Nodes (5): get_token(), has_token(), Result, String, store_token()

### Community 22 - "Root TypeScript Config"
Cohesion: 0.40
Nodes (4): compilerOptions, paths, files, references

## Knowledge Gaps
- **144 isolated node(s):** `appWindow`, `react/rules-of-hooks`, `$schema`, `oxc`, `warn` (+139 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **11 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `plugins` connect `Linting & Dev Tooling` to `Frontend UI Components`?**
  _High betweenness centrality (0.038) - this node is a cross-community bridge._
- **Are the 18 inferred relationships involving `test_conn()` (e.g. with `append_and_list_in_order()` and `deleting_session_cascades_to_activity()`) actually correct?**
  _`test_conn()` has 18 INFERRED edges - model-reasoned connections that need verification._
- **What connects `appWindow`, `react/rules-of-hooks`, `$schema` to the rest of the system?**
  _144 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Frontend UI Components` be split into smaller, more focused modules?**
  _Cohesion score 0.07211538461538461 - nodes in this community are weakly interconnected._
- **Should `Linting & Dev Tooling` be split into smaller, more focused modules?**
  _Cohesion score 0.05 - nodes in this community are weakly interconnected._
- **Should `Sessions & Container Metrics` be split into smaller, more focused modules?**
  _Cohesion score 0.1241565452091768 - nodes in this community are weakly interconnected._
- **Should `Frontend Dependencies` be split into smaller, more focused modules?**
  _Cohesion score 0.06451612903225806 - nodes in this community are weakly interconnected._