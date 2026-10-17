// Mirrors src-tauri/src/agent_config.rs.
export interface NamedFile {
  name: string
  content: string
}

export interface AgentConfig {
  hooks: Record<string, unknown>
  hook_files: NamedFile[]
  output_styles: NamedFile[]
  output_style: string | null
}

export function parseHooks(text: string): { hooks: Record<string, unknown> | null; error: string | null } {
  if (!text.trim()) return { hooks: {}, error: null }
  try {
    const value = JSON.parse(text)
    if (value == null || typeof value !== 'object' || Array.isArray(value)) {
      return { hooks: null, error: 'Hooks must be a JSON object, e.g. { "PreToolUse": [...] }' }
    }
    return { hooks: value, error: null }
  } catch (e) {
    return { hooks: null, error: (e as Error).message }
  }
}

export function styleName(fileName: string): string {
  return fileName.replace(/\.md$/i, '')
}

// Later entries with the same name replace earlier ones.
export function mergeFiles(existing: NamedFile[], incoming: NamedFile[]): NamedFile[] {
  const byName = new Map(existing.map((f) => [f.name, f]))
  for (const f of incoming) byName.set(f.name, f)
  return Array.from(byName.values())
}
