import { useMemo } from 'react'
import CodeMirror from '@uiw/react-codemirror'
import { yaml } from '@codemirror/lang-yaml'
import { parse } from 'yaml'

const extensions = [yaml()]

export function composeOverrideError(value: string): string | null {
  if (!value.trim()) return null
  try {
    const doc = parse(value)
    if (doc == null || typeof doc !== 'object' || Array.isArray(doc)) return 'Override must be a YAML mapping (e.g. services: ...).'
    return null
  } catch (e) {
    return (e as Error).message
  }
}

export function ComposeOverrideEditor({
  value,
  onChange,
  placeholder,
}: {
  value: string
  onChange: (value: string) => void
  placeholder?: string
}) {
  const isDark = document.documentElement.classList.contains('dark')
  const error = useMemo(() => composeOverrideError(value), [value])

  return (
    <div className="flex flex-col gap-1">
      <CodeMirror
        value={value}
        height="180px"
        theme={isDark ? 'dark' : 'light'}
        extensions={extensions}
        placeholder={placeholder ?? 'services:\n  web:\n    ports:\n      - "3000:3000"'}
        basicSetup={{ lineNumbers: true, foldGutter: false, tabSize: 2 }}
        onChange={onChange}
        className="overflow-hidden rounded-lg border border-border font-mono text-sm"
      />
      {error && <p className="font-mono text-xs text-destructive">{error}</p>}
    </div>
  )
}
