import { useState } from 'react'
import { Plus, Trash2, TriangleAlert } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import type { Project } from '@/components/projects/types'
import { kitScopeLabel, type Kit } from '@/lib/kits'
import { useAutosave } from '@/lib/useAutosave'
import { cn } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'
import { KitEditor } from '../kits/KitEditor'

export function KitsTab() {
  const kits = useAppStore((s) => s.kits)
  const projects = useAppStore((s) => s.projects)
  const createKit = useAppStore((s) => s.createKit)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const selected = kits.find((k) => k.id === selectedId) ?? kits[0]

  async function addKit() {
    const kit = await createKit('New kit')
    setSelectedId(kit.id)
  }

  return (
    <div className="flex h-full min-h-0 gap-4 pt-4">
      <aside className="flex w-56 shrink-0 flex-col gap-1 overflow-auto">
        <Button size="sm" variant="outline" onClick={addKit}>
          <Plus className="size-3.5" />
          New kit
        </Button>
        {kits.map((kit) => (
          <button
            key={kit.id}
            type="button"
            onClick={() => setSelectedId(kit.id)}
            className={cn(
              'flex flex-col rounded-lg px-2.5 py-2 text-left text-sm',
              kit.id === selected?.id ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/60'
            )}
          >
            <span className="truncate">{kit.name}</span>
            <span className="text-xs text-muted-foreground">{kitScopeLabel(kit)}</span>
          </button>
        ))}
      </aside>
      {selected ? (
        <KitDetail key={selected.id} kit={selected} kits={kits} projects={projects} />
      ) : (
        <p className="text-sm text-muted-foreground">
          No kits yet. A kit installs tools (for example a pinned Node.js or Python) when a sandbox is created.
        </p>
      )}
    </div>
  )
}

function KitDetail({ kit, kits, projects }: { kit: Kit; kits: Kit[]; projects: Project[] }) {
  const updateKit = useAppStore((s) => s.updateKit)
  const setKitScope = useAppStore((s) => s.setKitScope)
  const deleteKit = useAppStore((s) => s.deleteKit)
  const [draft, setDraft] = useState({ name: kit.name, spec: kit.spec })
  const [scopeError, setScopeError] = useState<string | null>(null)
  const [confirmingDelete, setConfirmingDelete] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const { state, error } = useAutosave(draft, (d) => updateKit(kit.id, d.name, d.spec), !deleting)

  async function remove() {
    setDeleting(true)
    await deleteKit(kit.id)
  }

  async function applyScope(global: boolean, projectIds: string[]) {
    setScopeError(null)
    try {
      await setKitScope(kit.id, global, projectIds)
    } catch (e) {
      setScopeError(String(e))
    }
  }

  function toggleProject(projectId: string) {
    const ids = kit.project_ids.includes(projectId)
      ? kit.project_ids.filter((id) => id !== projectId)
      : [...kit.project_ids, projectId]
    applyScope(false, ids)
  }

  const notApplied = !kit.is_global && kit.project_ids.length === 0
  const status = state === 'saving' ? 'Saving…' : state === 'saved' ? 'Saved' : state === 'error' ? error : null

  return (
    <div className="flex min-w-0 flex-1 flex-col gap-3">
      <div className="flex items-center gap-2">
        <Input
          value={draft.name}
          onChange={(e) => setDraft((d) => ({ ...d, name: e.target.value }))}
          className="max-w-sm"
        />
        <span className={cn('text-xs', state === 'error' ? 'text-destructive' : 'text-muted-foreground')}>{status}</span>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Delete kit"
          className="ml-auto"
          onClick={() => setConfirmingDelete(true)}
        >
          <Trash2 className="size-4" strokeWidth={1.5} />
        </Button>
      </div>

      <div className="flex flex-col gap-2">
        <div className="flex gap-2">
          <Button size="sm" variant={kit.is_global ? 'default' : 'outline'} onClick={() => applyScope(true, [])}>
            Global
          </Button>
          <Button
            size="sm"
            variant={kit.is_global ? 'outline' : 'default'}
            onClick={() => applyScope(false, kit.project_ids)}
          >
            Projects
          </Button>
        </div>
        {!kit.is_global && (
          <div className="flex flex-wrap gap-3">
            {projects.map((project) => {
              const other =
                project.kit_id && project.kit_id !== kit.id ? kits.find((k) => k.id === project.kit_id) : undefined
              return (
                <label key={project.id} className="flex items-center gap-1.5 text-sm">
                  <input
                    type="checkbox"
                    checked={kit.project_ids.includes(project.id)}
                    onChange={() => toggleProject(project.id)}
                  />
                  {project.name}
                  {other && <span className="text-xs text-muted-foreground">(uses {other.name})</span>}
                </label>
              )
            })}
          </div>
        )}
        {notApplied && (
          <p className="flex items-center gap-1.5 text-xs text-warning">
            <TriangleAlert className="size-3.5" strokeWidth={1.5} />
            Not applied. Pick at least one project or make it global.
          </p>
        )}
        {scopeError && <p className="text-xs text-destructive">{scopeError}</p>}
        <p className="text-xs text-muted-foreground">
          Install commands run once when a sandbox is created. Allow their download hosts in Network settings.
        </p>
      </div>

      <KitEditor value={draft.spec} onChange={(spec) => setDraft((d) => ({ ...d, spec }))} />

      <Dialog open={confirmingDelete} onOpenChange={setConfirmingDelete}>
        <DialogContent title="Delete kit?">
          <Card className="w-full">
            <CardHeader>
              <CardTitle>Delete {kit.name}?</CardTitle>
            </CardHeader>
            <CardContent className="flex justify-end gap-2">
              <Button variant="outline" onClick={() => setConfirmingDelete(false)}>
                Cancel
              </Button>
              <Button variant="destructive" disabled={deleting} onClick={remove}>
                Delete
              </Button>
            </CardContent>
          </Card>
        </DialogContent>
      </Dialog>
    </div>
  )
}
