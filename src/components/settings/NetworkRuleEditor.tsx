import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import type { NetworkRuleDecision, PolicyRule } from '@/lib/networkPolicy'

/**
 * Chip-list allow/deny editor, shared between the Settings screen's
 * global "Network Policy" card and each SandboxCard's per-sandbox
 * override editor. `rules` always comes from a live `sbx policy ls
 * --wide` read (see sbx/mod.rs's PolicyRule doc comment) — this component
 * never caches or diffs against a local copy, it just renders whatever
 * the caller last fetched and re-fetches via `onAdd`/`onRemove`'s
 * promises settling.
 *
 * Org/kit-sourced rules (source !== "local") are shown read-only, per the
 * docs' governance precedence — only locally-added rules can be removed
 * from here.
 */
export function NetworkRuleEditor({
  rules,
  onAdd,
  onRemove,
}: {
  rules: PolicyRule[]
  onAdd: (decision: NetworkRuleDecision, host: string) => Promise<void>
  onRemove: (host: string) => Promise<void>
}) {
  const [decision, setDecision] = useState<NetworkRuleDecision>('allow')
  const [host, setHost] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const allowRules = rules.filter((r) => r.decision === 'allow')
  const denyRules = rules.filter((r) => r.decision === 'deny')

  async function handleAdd() {
    const trimmed = host.trim()
    if (!trimmed) return
    setBusy(true)
    setError(null)
    try {
      await onAdd(decision, trimmed)
      setHost('')
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(false)
    }
  }

  async function handleRemove(ruleHost: string) {
    setBusy(true)
    setError(null)
    try {
      await onRemove(ruleHost)
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(false)
    }
  }

  function renderChips(list: PolicyRule[], label: string) {
    return (
      <div className="flex flex-col gap-1">
        <span className="text-xs text-muted-foreground/70">{label}</span>
        {list.length === 0 ? (
          <p className="text-xs text-muted-foreground">None</p>
        ) : (
          <ul className="flex flex-wrap gap-1.5">
            {list.map((rule) => (
              <li
                key={`${rule.decision}:${rule.host}`}
                title={`source: ${rule.source}`}
                className="flex items-center gap-1 rounded-full border border-border px-2 py-0.5 text-xs"
              >
                <span className="font-mono">{rule.host}</span>
                {rule.source === 'local' ? (
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => handleRemove(rule.host)}
                    className="text-muted-foreground hover:text-destructive"
                  >
                    ×
                  </button>
                ) : (
                  <span className="text-muted-foreground/60">({rule.source})</span>
                )}
              </li>
            ))}
          </ul>
        )}
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-3">
      {renderChips(allowRules, 'Allow')}
      {renderChips(denyRules, 'Deny')}
      <div className="flex gap-2">
        <Select value={decision} onValueChange={(value) => setDecision(value as NetworkRuleDecision)}>
          <SelectTrigger className="h-8 w-24">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="allow">Allow</SelectItem>
            <SelectItem value="deny">Deny</SelectItem>
          </SelectContent>
        </Select>
        <Input
          className="h-8"
          placeholder="host, e.g. api.example.com"
          value={host}
          onChange={(e) => setHost(e.target.value)}
        />
        <Button size="sm" type="button" onClick={handleAdd} disabled={busy || !host.trim()}>
          Add
        </Button>
      </div>
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  )
}
