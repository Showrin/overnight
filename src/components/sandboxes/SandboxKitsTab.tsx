import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Loader2, Pencil, Plus, RotateCw } from 'lucide-react'
import { KitEditor } from '@/components/settings/kits/KitEditor'
import { SettingsSection } from '@/components/settings/SettingsLayout'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import type { Kit } from '@/lib/kits'
import { useAutosave } from '@/lib/useAutosave'
import { cn } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'
import type { Sandbox } from './types'

export function SandboxKitsTab({ sandbox }: { sandbox: Sandbox }) {
  const kits = useAppStore((s) => s.kits)
  const loadSandboxes = useAppStore((s) => s.loadSandboxes)
  const createKit = useAppStore((s) => s.createKit)
  const [addId, setAddId] = useState<string>('')
  const [editingId, setEditingId] = useState<string | null>(null)
  const [confirming, setConfirming] = useState<Kit | null>(null)
  const [applying, setApplying] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const running = sandbox.status === 'running'
  const available = kits.filter((k) => !sandbox.kit_ids.includes(k.id))
  const editing = kits.find((k) => k.id === editingId)

  async function apply(kit: Kit) {
    setApplying(true)
    setError(null)
    try {
      await invoke('apply_sandbox_kit', { id: sandbox.id, kitId: kit.id })
      setAddId('')
    } catch (e) {
      setError(String(e))
    } finally {
      await loadSandboxes()
      setApplying(false)
      setConfirming(null)
    }
  }

  async function addNew() {
    const kit = await createKit(`${sandbox.name ?? sandbox.sbx_name ?? 'Sandbox'} kit`)
    setAddId(kit.id)
    setEditingId(kit.id)
  }

  return (
    <SettingsSection title="Kits">
      <div className="flex flex-col gap-4 py-4">
        <p className="text-xs text-muted-foreground">
          Applying a kit restarts the sandbox. Installed packages and agent history are kept. Only env vars, install
          commands and network allow rules are applied, and a kit can't be removed once applied.
        </p>

        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium">Applied</span>
          {sandbox.kit_ids.length === 0 && <p className="text-sm text-muted-foreground">No kits applied.</p>}
          {sandbox.kit_ids.map((kitId) => {
            const kit = kits.find((k) => k.id === kitId)
            return (
              <div key={kitId} className="flex items-center gap-2 border-b border-border-subtle py-1.5 text-sm">
                <span className={cn('flex-1 truncate', !kit && 'text-muted-foreground')}>{kit?.name ?? 'Deleted kit'}</span>
                {kit && (
                  <>
                    <Button size="sm" variant="ghost" onClick={() => setEditingId(kit.id)}>
                      <Pencil className="size-3.5" />
                      Edit
                    </Button>
                    <Button size="sm" variant="outline" disabled={!running || applying} onClick={() => setConfirming(kit)}>
                      <RotateCw className="size-3.5" />
                      Re-apply
                    </Button>
                  </>
                )}
              </div>
            )
          })}
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <Select value={addId} onValueChange={setAddId}>
            <SelectTrigger id="sandbox-kit-add" className="w-64">
              <SelectValue placeholder="Pick a kit to add" />
            </SelectTrigger>
            <SelectContent>
              {available.map((k) => (
                <SelectItem key={k.id} value={k.id}>
                  {k.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button
            size="sm"
            disabled={!running || applying || !addId}
            onClick={() => setConfirming(kits.find((k) => k.id === addId) ?? null)}
          >
            Apply
          </Button>
          {addId && (
            <Button size="sm" variant="ghost" onClick={() => setEditingId(addId)}>
              <Pencil className="size-3.5" />
              Edit
            </Button>
          )}
          <Button size="sm" variant="outline" onClick={addNew}>
            <Plus className="size-3.5" />
            New kit
          </Button>
        </div>
        {!running && <p className="text-xs text-muted-foreground">Start the sandbox to apply kits.</p>}
        {error && <p className="text-xs break-all text-destructive">{error}</p>}

        {editing && <KitDraftEditor key={editing.id} kit={editing} applied={sandbox.kit_ids.includes(editing.id)} />}
      </div>

      <Dialog open={confirming != null} onOpenChange={(open) => !open && !applying && setConfirming(null)}>
        <DialogContent title="Apply kit?">
          <Card className="w-full">
            <CardHeader>
              <CardTitle>Apply {confirming?.name}?</CardTitle>
            </CardHeader>
            <CardContent className="flex flex-col gap-3">
              <p className="text-sm text-muted-foreground">
                The sandbox restarts while the kit is applied. Running agent sessions and terminals in it stop.
              </p>
              <div className="flex justify-end gap-2">
                <Button variant="outline" disabled={applying} onClick={() => setConfirming(null)}>
                  Cancel
                </Button>
                <Button disabled={applying} onClick={() => confirming && apply(confirming)}>
                  {applying && <Loader2 className="size-3.5 animate-spin" />}
                  {applying ? 'Applying…' : 'Apply and restart'}
                </Button>
              </div>
            </CardContent>
          </Card>
        </DialogContent>
      </Dialog>
    </SettingsSection>
  )
}

function KitDraftEditor({ kit, applied }: { kit: Kit; applied: boolean }) {
  const updateKit = useAppStore((s) => s.updateKit)
  const [draft, setDraft] = useState({ name: kit.name, spec: kit.spec })
  const { state, error } = useAutosave(draft, (d) => updateKit(kit.id, d.name, d.spec))
  const status = state === 'saving' ? 'Saving…' : state === 'saved' ? 'Saved' : state === 'error' ? error : null

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <Input
          id="sandbox-kit-name"
          value={draft.name}
          onChange={(e) => setDraft((d) => ({ ...d, name: e.target.value }))}
          className="max-w-sm"
        />
        <span className={cn('text-xs break-all', state === 'error' ? 'text-destructive' : 'text-muted-foreground')}>{status}</span>
      </div>
      <KitEditor value={draft.spec} onChange={(spec) => setDraft((d) => ({ ...d, spec }))} />
      <p className="text-xs text-muted-foreground">
        Shared kit: edits change it for every project and sandbox that uses it.
        {applied ? ' Re-apply to update this sandbox.' : ''}
      </p>
    </div>
  )
}
