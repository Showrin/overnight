import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Plus } from 'lucide-react'
import { ComposeOverrideEditor, composeOverrideError } from '@/components/settings/ComposeOverrideEditor'
import { SettingsSection } from '@/components/settings/SettingsLayout'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import type { ComposeOverride } from '@/lib/composeOverrides'
import { useAutosave } from '@/lib/useAutosave'
import { cn } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'
import type { Sandbox } from './types'

const NO_OVERRIDE = '__no_override__'

export function SandboxComposeTab({ sandbox }: { sandbox: Sandbox }) {
  const items = useAppStore((s) => s.composeOverrides)
  const loadSandboxes = useAppStore((s) => s.loadSandboxes)
  const createItem = useAppStore((s) => s.createComposeOverride)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const selected = items.find((o) => o.id === sandbox.compose_override_id)

  async function select(overrideId: string | null) {
    setBusy(true)
    setError(null)
    try {
      await invoke('set_sandbox_compose_override', { id: sandbox.id, overrideId })
    } catch (e) {
      setError(String(e))
    } finally {
      await loadSandboxes()
      setBusy(false)
    }
  }

  async function addNew() {
    const item = await createItem(`${sandbox.name ?? sandbox.sbx_name ?? 'Sandbox'} override`)
    await select(item.id)
  }

  return (
    <SettingsSection title="Docker Compose Override">
      <div className="flex flex-col gap-3 py-4">
        <p className="text-xs text-muted-foreground">
          Written next to the workspace compose file as docker-compose.override.yml (or compose.override.yaml) and
          excluded from git. Manage the full list in Settings › Compose Overrides.
        </p>
        <div className="flex flex-wrap items-center gap-2">
          <Select value={selected?.id ?? NO_OVERRIDE} onValueChange={(v) => select(v === NO_OVERRIDE ? null : v)} disabled={busy}>
            <SelectTrigger id="sandbox-compose-select" className="w-64">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={NO_OVERRIDE}>No override</SelectItem>
              {items.map((o) => (
                <SelectItem key={o.id} value={o.id}>
                  {o.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button size="sm" variant="outline" onClick={addNew} disabled={busy}>
            <Plus className="size-3.5" />
            New override
          </Button>
          {busy && <span className="text-xs text-muted-foreground">Applying…</span>}
        </div>
        {error && <p className="text-xs break-all text-destructive">{error}</p>}
        {selected && <OverrideEditor key={selected.id} item={selected} />}
      </div>
    </SettingsSection>
  )
}

function OverrideEditor({ item }: { item: ComposeOverride }) {
  const updateItem = useAppStore((s) => s.updateComposeOverride)
  const users = useAppStore((s) => s.sandboxes.filter((sb) => sb.compose_override_id === item.id).length)
  const [draft, setDraft] = useState({ name: item.name, content: item.content })
  const valid = composeOverrideError(draft.content) == null
  const { state, error } = useAutosave(draft, (d) => updateItem(item.id, d.name, d.content), valid)
  const status = state === 'saving' ? 'Saving…' : state === 'saved' ? 'Saved' : state === 'error' ? error : null

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <Input
          id="sandbox-compose-name"
          value={draft.name}
          onChange={(e) => setDraft((d) => ({ ...d, name: e.target.value }))}
          className="max-w-sm"
        />
        <span className={cn('text-xs break-all', state === 'error' ? 'text-destructive' : 'text-muted-foreground')}>{status}</span>
      </div>
      <ComposeOverrideEditor value={draft.content} onChange={(content) => setDraft((d) => ({ ...d, content }))} />
      <p className="text-xs text-muted-foreground">
        Shared with {users === 1 ? 'this sandbox only' : `${users} sandboxes`}. Edits are pushed to every running sandbox
        that uses it.
      </p>
    </div>
  )
}
