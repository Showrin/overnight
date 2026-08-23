export type SandboxMode = 'mount' | 'clone'
export type SandboxStatus = 'starting' | 'running' | 'stopped' | 'error'

export interface Sandbox {
  id: string
  project_id: string
  mode: SandboxMode
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
