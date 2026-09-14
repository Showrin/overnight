export interface CommandLogEntry {
  id: string
  occurred_at: number
  operation: string
  program: string
  // JSON-encoded string[]
  args: string
  success: boolean
  exit_code: number | null
  stderr: string | null
}

export interface DaemonLogResult {
  exists: boolean
  resolved_path: string | null
  content: string
  truncated: boolean
}
