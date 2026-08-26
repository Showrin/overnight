# Graph Report - overnight  (2026-08-27)

## Corpus Check
- 49 files · ~26,627 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 769 nodes · 1595 edges · 42 communities (34 shown, 8 thin omitted)
- Extraction: 96% EXTRACTED · 4% INFERRED · 0% AMBIGUOUS · INFERRED: 58 edges (avg confidence: 0.79)
- Token cost: 94,508 input · 0 output

## Community Hubs (Navigation)
- Claude Code Provider & Sync
- Tauri Command Handlers
- Sandbox Host Monitor
- Jira Client & Issue Sync
- DB Migrations & Settings
- Activity & Metrics Logging
- Oxlint & Dev Tooling
- Frontend Dependencies
- Tauri App Config & Icons
- Jira/Sandbox UI Forms
- Architecture & README Docs
- TS App Config
- Process Output Streaming
- Sandbox Cards & App Store
- App Shell & Sidebar UI
- shadcn Components Config
- TS Node Config
- Session DB Model
- UI Primitives
- Sandbox Dialogs & Charts
- Container Metrics DB
- Projects DB
- Tasks DB
- Tauri Capabilities & Permissions
- Host Metrics DB
- JSON Lines Parser
- DB Error Type
- DB Pool Init
- Project Form UI
- Jira Error Type
- Jira Credentials Store
- Root TS Config
- Projects/Tasks Table Docs
- Vite Config
- Settings Table Doc
- Jira Issue Command Type
- DB Error Ok Variant
- DB Error String Variant
- DB Error Serialize Impl

## God Nodes (most connected - your core abstractions)
1. `test_conn()` - 32 edges
2. `react` - 20 edges
3. `compilerOptions` - 19 edges
4. `SessionHandle` - 18 edges
5. `run()` - 17 edges
6. `compilerOptions` - 15 edges
7. `Session` - 15 edges
8. `now_millis()` - 15 edges
9. `useAppStore` - 15 edges
10. `create()` - 13 edges

## Surprising Connections (you probably didn't know these)
- `missing_id_operations_return_not_found()` --calls--> `test_conn()`  [INFERRED]
  src-tauri/src/db/tasks.rs → src-tauri/src/db/migrations.rs
- `index.html (app shell + splash screen)` --conceptually_related_to--> `Design tokens (src/index.css)`  [INFERRED]
  index.html → ARCHITECTURE.md
- `index.html (app shell + splash screen)` --conceptually_related_to--> `React 19 / TypeScript frontend`  [INFERRED]
  index.html → README.md
- `index.html (app shell + splash screen)` --conceptually_related_to--> `Tauri v2`  [INFERRED]
  index.html → README.md
- `Docker Sandboxes feature` --conceptually_related_to--> `Sandbox parameterization (forward-looking)`  [INFERRED]
  README.md → ARCHITECTURE.md

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **AgentProvider abstraction components** — architecture_agentprovider_trait, architecture_claudecodeprovider, architecture_sessionhandle, architecture_agentevent [INFERRED 0.85]
- **Session-scoped data tables** — architecture_sessions_table, architecture_activity_table, architecture_metrics_table, architecture_container_metrics_table [INFERRED 0.80]

## Communities (42 total, 8 thin omitted)

### Community 0 - "Claude Code Provider & Sync"
Cohesion: 0.07
Nodes (37): AppHandle, ClaudeCodeProvider, EmittedEvent, event_type_name(), persist_and_emit(), AppHandle, Box, DbPool (+29 more)

### Community 1 - "Tauri Command Handlers"
Cohesion: 0.15
Nodes (53): JiraIssue, AppSettings, check_free_memory(), create_project(), create_sandbox(), create_task(), delete_project(), delete_sandbox() (+45 more)

### Community 2 - "Sandbox Host Monitor"
Cohesion: 0.12
Nodes (33): Instant, Networks, Self, container_start_failed_message(), container_start_failed_message_includes_raw_stderr_and_a_platform_hint(), create(), Error, expand_home() (+25 more)

### Community 3 - "Jira Client & Issue Sync"
Cohesion: 0.09
Nodes (33): Client, Issue, Map, issue(), list(), resync_is_idempotent_and_updates_in_place(), row_to_issue(), Connection (+25 more)

### Community 4 - "DB Migrations & Settings"
Cohesion: 0.12
Nodes (38): migrations(), migrations_apply_cleanly(), Connection, test_conn(), missing_id_operations_return_not_found(), create(), create_get_list_update_delete(), delete() (+30 more)

### Community 5 - "Activity & Metrics Logging"
Cohesion: 0.11
Nodes (36): append(), append_and_list_in_order(), deleting_session_cascades_to_activity(), list_for_session(), make_session(), row_to_activity(), Connection, Result (+28 more)

### Community 6 - "Oxlint & Dev Tooling"
Cohesion: 0.05
Nodes (35): oxlint, plugins, rules, react/only-export-components, react/rules-of-hooks, $schema, devDependencies, oxlint (+27 more)

### Community 7 - "Frontend Dependencies"
Cohesion: 0.06
Nodes (33): class-variance-authority, clsx, @fontsource/jetbrains-mono, @fontsource-variable/inter, lucide-react, dependencies, class-variance-authority, clsx (+25 more)

### Community 8 - "Tauri App Config & Icons"
Cohesion: 0.07
Nodes (28): icons/128x128@2x.png, icons/128x128.png, icons/32x32.png, icons/icon.icns, icons/icon.ico, debugApplicationIdSuffix, app, security (+20 more)

### Community 9 - "Jira/Sandbox UI Forms"
Cohesion: 0.16
Nodes (16): JiraConfig, JiraIssue, ErrorDetails(), JiraConfig, JiraConfigForm(), Card(), CardContent(), CardHeader() (+8 more)

### Community 10 - "Architecture & README Docs"
Cohesion: 0.09
Nodes (23): activity table, AgentEvent, AgentProvider trait, ClaudeCodeProvider, container_metrics table, Design tokens (src/index.css), Claude-specific logic isolation rule, metrics table (+15 more)

### Community 11 - "TS App Config"
Cohesion: 0.08
Nodes (24): DOM, src, vite/client, compilerOptions, allowArbitraryExtensions, allowImportingTsExtensions, erasableSyntaxOnly, jsx (+16 more)

### Community 12 - "Process Output Streaming"
Cohesion: 0.14
Nodes (23): CommandEvent, Path, Receiver, emit_to_webview(), Error, AppHandle, Box, CommandChild (+15 more)

### Community 13 - "Sandbox Cards & App Store"
Cohesion: 0.13
Nodes (15): ProjectsScreen(), BusyAction, SandboxCard(), run(), HostMetric, Sandbox, SandboxMode, SandboxStatus (+7 more)

### Community 14 - "App Shell & Sidebar UI"
Cohesion: 0.12
Nodes (15): App(), JiraIssueList(), handleSync(), loadIssues(), BusyAction, SidebarSandboxList(), run(), SettingsScreen() (+7 more)

### Community 15 - "shadcn Components Config"
Cohesion: 0.09
Nodes (21): aliases, components, hooks, lib, ui, utils, iconLibrary, menuAccent (+13 more)

### Community 16 - "TS Node Config"
Cohesion: 0.10
Nodes (19): node, vite.config.ts, compilerOptions, allowImportingTsExtensions, erasableSyntaxOnly, lib, module, moduleDetection (+11 more)

### Community 17 - "Session DB Model"
Cohesion: 0.29
Nodes (19): Session, create(), create_get_list_end(), deleting_task_cascades_to_sessions(), end(), get(), list_for_task(), make_task() (+11 more)

### Community 18 - "UI Primitives"
Cohesion: 0.25
Nodes (9): react, Project, Badge(), badgeVariants, Button(), buttonVariants, Input(), Label() (+1 more)

### Community 19 - "Sandbox Dialogs & Charts"
Cohesion: 0.16
Nodes (10): Sparkline(), CreateSandboxDialog(), handleCreate(), initPolicyAndRetry(), formatKbPerSec(), HostStatsPanel(), SandboxesScreen(), Dialog() (+2 more)

### Community 20 - "Container Metrics DB"
Cohesion: 0.29
Nodes (17): list_for_sandbox(), list_for_session(), make_sandbox(), make_session(), record(), record_and_list_for_sandbox(), record_and_list_for_session(), record_for_sandbox() (+9 more)

### Community 21 - "Projects DB"
Cohesion: 0.30
Nodes (17): now_millis(), create(), create_get_list_update_delete(), delete(), deleting_project_nulls_task_project_id(), get(), list(), row_to_project() (+9 more)

### Community 22 - "Tasks DB"
Cohesion: 0.30
Nodes (15): Task, create(), create_get_list_update_delete(), delete(), duplicate_jira_key_rejected(), get(), list(), missing_id_operations_return_not_found() (+7 more)

### Community 23 - "Tauri Capabilities & Permissions"
Cohesion: 0.14
Nodes (13): core:default, core:window:allow-close, core:window:allow-maximize, core:window:allow-minimize, core:window:allow-start-dragging, dialog:allow-open, main, notification:default (+5 more)

### Community 24 - "Host Metrics DB"
Cohesion: 0.31
Nodes (13): list_since(), prune_older_than(), prune_removes_only_old_rows(), record(), record_and_list_since(), row_to_host_metric(), Connection, HostMetric (+5 more)

### Community 25 - "JSON Lines Parser"
Cohesion: 0.27
Nodes (9): parse(), parses_valid_lines_and_skips_malformed_and_empty(), Box, Item, Pin, S, Send, Stream (+1 more)

### Community 26 - "DB Error Type"
Cohesion: 0.25
Nodes (6): Ok, S, Serialize, Error, Result, String

### Community 27 - "DB Pool Init"
Cohesion: 0.52
Nodes (6): PathBuf, db_path(), init_pool(), AppHandle, DbPool, Result

### Community 29 - "Jira Error Type"
Cohesion: 0.29
Nodes (5): Error, Ok, Result, S, Serialize

### Community 30 - "Jira Credentials Store"
Cohesion: 0.47
Nodes (5): get_token(), has_token(), Result, String, store_token()

### Community 31 - "Root TS Config"
Cohesion: 0.40
Nodes (4): compilerOptions, paths, files, references

## Knowledge Gaps
- **151 isolated node(s):** `react/rules-of-hooks`, `$schema`, `oxc`, `warn`, `allowImportingTsExtensions` (+146 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **8 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `AgentProvider` connect `Claude Code Provider & Sync` to `Tauri Command Handlers`?**
  _High betweenness centrality (0.044) - this node is a cross-community bridge._
- **Why does `react` connect `UI Primitives` to `Oxlint & Dev Tooling`, `Jira/Sandbox UI Forms`, `Sandbox Cards & App Store`, `App Shell & Sidebar UI`, `Sandbox Dialogs & Charts`?**
  _High betweenness centrality (0.041) - this node is a cross-community bridge._
- **Are the 28 inferred relationships involving `test_conn()` (e.g. with `append_and_list_in_order()` and `deleting_session_cascades_to_activity()`) actually correct?**
  _`test_conn()` has 28 INFERRED edges - model-reasoned connections that need verification._
- **What connects `react/rules-of-hooks`, `$schema`, `oxc` to the rest of the system?**
  _151 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Claude Code Provider & Sync` be split into smaller, more focused modules?**
  _Cohesion score 0.07272727272727272 - nodes in this community are weakly interconnected._
- **Should `Sandbox Host Monitor` be split into smaller, more focused modules?**
  _Cohesion score 0.11839323467230443 - nodes in this community are weakly interconnected._
- **Should `Jira Client & Issue Sync` be split into smaller, more focused modules?**
  _Cohesion score 0.08859357696567 - nodes in this community are weakly interconnected._