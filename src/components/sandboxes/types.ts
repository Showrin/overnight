export type SandboxMode = 'mount' | 'clone'
export type SandboxStatus = 'starting' | 'running' | 'stopped' | 'error'

export interface Sandbox {
  id: string
  project_id: string
  mode: SandboxMode
  status: SandboxStatus
  container_id: string | null
  folder_path: string | null
  host_port: number | null
  created_at: number
  stopped_at: number | null
}

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

export interface SandboxUsage {
  input_tokens: number
  output_tokens: number
}
