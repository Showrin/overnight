// Windows-only: which console app wraps the sandbox terminal launch
// (`sbx exec -it <name> bash`). Ignored on macOS/Linux — the backend is a
// harmless no-op there, so this stays unscoped by OS on the frontend too.
export const TERMINAL_HOSTS = ['cmd', 'powershell'] as const
export type TerminalHost = (typeof TERMINAL_HOSTS)[number]

export const TERMINAL_HOST_LABELS: Record<TerminalHost, string> = {
  cmd: 'Command Prompt',
  powershell: 'PowerShell',
}
