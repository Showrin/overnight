export type SandboxMode = 'mount' | 'clone'
export type SandboxStatus = 'starting' | 'running' | 'stopping' | 'stopped' | 'error'

export interface Sandbox {
  id: string
  project_id: string
  name: string | null
  mode: SandboxMode
  permission_mode: string
  status: SandboxStatus
  sbx_name: string | null
  folder_path: string | null
  host_port: number | null
  created_at: number
  stopped_at: number | null
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
