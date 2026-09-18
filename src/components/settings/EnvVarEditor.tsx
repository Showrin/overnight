import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { type EnvVar, isValidEnvKey } from '@/lib/envVars'
import { MaskedValueInput } from './MaskedValueInput'

/**
 * Key/value row editor for env vars — add a row, edit a value inline,
 * remove a row. Purely controlled: the caller owns the list and decides
 * when/how to persist it. Values are masked by default (see
 * MaskedValueInput). `readOnly` hides the add/remove controls and shows
 * the list for reference only — used for a scope the caller can view but
 * not edit from here (e.g. global credentials shown on a sandbox's own
 * tab).
 */
export function EnvVarEditor({
  vars,
  onChange,
  readOnly = false,
}: {
  vars: EnvVar[]
  onChange: (vars: EnvVar[]) => void
  readOnly?: boolean
}) {
  const [newKey, setNewKey] = useState('')
  const [newValue, setNewValue] = useState('')
  const [error, setError] = useState<string | null>(null)

  function addRow() {
    const key = newKey.trim()
    if (!key) return
    if (!isValidEnvKey(key)) {
      setError('Keys must start with a letter or underscore and contain only letters, digits, and underscores.')
      return
    }
    if (vars.some((v) => v.key === key)) {
      setError(`"${key}" is already set.`)
      return
    }
    setError(null)
    onChange([...vars, { key, value: newValue }])
    setNewKey('')
    setNewValue('')
  }

  function updateValue(key: string, value: string) {
    onChange(vars.map((v) => (v.key === key ? { ...v, value } : v)))
  }

  function removeRow(key: string) {
    onChange(vars.filter((v) => v.key !== key))
  }

  return (
    <div className="flex flex-col gap-3">
      {!readOnly && (
        <div className="flex gap-2">
          <Input className="h-8 max-w-40 font-mono" placeholder="KEY" value={newKey} onChange={(e) => setNewKey(e.target.value)} />
          <Input className="h-8" placeholder="value" value={newValue} onChange={(e) => setNewValue(e.target.value)} />
          <Button size="sm" type="button" onClick={addRow} disabled={!newKey.trim()}>
            Add
          </Button>
        </div>
      )}
      {!readOnly && error && <p className="text-xs text-destructive">{error}</p>}
      {vars.length === 0 ? (
        <p className="text-xs text-muted-foreground">No credentials set.</p>
      ) : (
        <ul className="flex flex-col gap-1.5">
          {vars.map((v) => (
            <li key={v.key} className="flex items-center gap-2">
              <span className="w-40 shrink-0 truncate font-mono text-sm">{v.key}</span>
              <MaskedValueInput className="h-8" value={v.value} onChange={(value) => updateValue(v.key, value)} readOnly={readOnly} />
              {!readOnly && (
                <button
                  type="button"
                  onClick={() => removeRow(v.key)}
                  className="shrink-0 text-xs text-muted-foreground hover:text-destructive"
                >
                  Remove
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
