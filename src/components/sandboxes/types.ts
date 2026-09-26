export type SandboxMode = 'mount' | 'clone'
export type SandboxStatus = 'starting' | 'running' | 'stopping' | 'stopped' | 'error'

export interface Sandbox {
  id: string
  project_id: string
  name: string | null
  mode: SandboxMode
  agent: string
  permission_mode: string
  status: SandboxStatus
  sbx_name: string | null
  folder_path: string | null
  host_port: number | null
  created_at: number
  stopped_at: number | null
  // "open" | "locked-down" | null (null = inherit the global default)
  network_preset_override: string | null
  // Unix-ms timestamp of the last `backup_sandbox_claude_data` run, or null
  // if this sandbox's ~/.claude has never been backed up.
  last_backup_at: number | null
  // Host destination folder the last backup landed in.
  last_backup_path: string | null
  // The host repo's checked-out branch, snapshotted once at creation time.
  // Null on detached HEAD, a non-git repo_path, or for rows created before
  // branch tracking was added.
  base_branch: string | null
  // The sandbox's currently checked-out branch, as of the last branch
  // snapshot. Null before any snapshot has been taken, or on a detached
  // HEAD inside the sandbox.
  current_branch: string | null
  // Local branch names inside the sandbox, as of the last snapshot.
  branches: string[]
  // Worktrees inside the sandbox, as of the last snapshot.
  worktrees: WorktreeInfo[]
  // Unix-ms timestamp of the last branch snapshot, or null before the
  // first one. Drives the Branch tab's "live" vs "as of <relative time>"
  // freshness label.
  branch_snapshot_at: number | null
  // Unix-ms timestamp of the last Git Sync run, or null if this sandbox
  // has never been synced.
  last_git_sync_at: number | null
  // Per-branch outcome of the last Git Sync run. Empty until the first
  // sync.
  last_git_sync_result: BranchSyncOutcome[]
  // Scheduled backups only; manual and before-stop/delete backups ignore it.
  backup_enabled: boolean
  // null = use the global interval.
  backup_interval_minutes: number | null
}

export interface WorktreeInfo {
  path: string
  branch: string | null
  head_sha: string
}

export interface SandboxUsage {
  input_tokens: number
  output_tokens: number
}

export type BranchSyncStatus = 'fast_forwarded' | 'needs_manual_merge' | 'new_branch'

export interface BranchSyncOutcome {
  branch: string
  status: BranchSyncStatus
}

// One branch's diff against base_branch: mount mode has exactly one (the
// host's current branch, diffed against its live working tree); clone mode
// has one per branch found on the sandbox's fetched sandbox-<name> remote.
export interface BranchDiff {
  branch: string
  stat: string
  patch: string
}

// Stats-only counterpart to BranchDiff — no patch text, for the Branches
// list view which only ever renders `stat`.
export interface BranchStat {
  branch: string
  stat: string
}

export interface SandboxDiffStats {
  base_branch: string | null
  branches: BranchStat[]
}

// One commit `get_branch_commits` found between a branch and its base.
export interface CommitInfo {
  hash: string
  author: string
  authored_at: number
  message: string
}

// One `.md` file found under the sandbox's `/.claude/plans` after a
// sync_sandbox_plans call.
export interface PlanFile {
  name: string
  content: string
  modified_at: number
}

export interface HostMetric {
  id: string
  captured_at: number
  cpu_percent: number
  memory_percent: number
  memory_used_mb: number
  memory_total_mb: number
  disk_percent: number
  disk_used_mb: number
  disk_total_mb: number
  network_rx_kb_per_sec: number
  network_tx_kb_per_sec: number
}

// A single point sampled by get_sandbox_resource_usage (live) or returned
// by get_sandbox_resource_history (seed). Despite the column names,
// network_rx_bytes/network_tx_bytes hold KB/s rates (deltas computed
// server-side against the sandbox's previous sample), the same "already a
// rate, not a cumulative counter" shape HostMetric's network fields use —
// container_metrics predates that convention and was never renamed.
export interface ContainerMetric {
  id: string
  session_id: string | null
  sandbox_id: string | null
  captured_at: number
  cpu_percent: number
  memory_mb: number
  network_rx_bytes: number
  network_tx_bytes: number
}
