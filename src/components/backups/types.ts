export type BackupScope = 'claude' | 'git' | 'all'

export interface SandboxBackup {
  id: string
  sandbox_id: string
  // Snapshot of the sandbox's display name at backup time — the only way
  // to show something readable once the live sandbox row is gone.
  sandbox_label: string | null
  created_at: number
  // "scheduled" | "manual" | "pre_stop" | "pre_delete"
  trigger: string
  host_dir: string
  has_claude: boolean
  has_git: boolean
  base_branch: string | null
  current_branch: string | null
  branches: string[]
  plan_file_count: number
  size_bytes: number
}

// A sandbox with a .claude/.git copy currently in flight, from list_active_backups.
// Backs the per-sandbox local pulsing icon — backups only, never restores.
export interface ActiveBackup {
  sandbox_id: string
  trigger: string
  started_at: number
}

// One entry in the global "operation in progress" indicator, from
// list_active_operations. source_sandbox_id/scope are only set for a
// restore; trigger only for a backup.
export interface ActiveOperation {
  kind: 'backup' | 'restore'
  sandbox_id: string
  source_sandbox_id: string | null
  scope: BackupScope | null
  trigger: string | null
  started_at: number
}
