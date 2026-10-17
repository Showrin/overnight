import { useState } from 'react'
import { Plus, Trash2, TriangleAlert } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import type { Project } from '@/components/projects/types'
import type { ComposeOverride } from '@/lib/composeOverrides'
import { kitScopeLabel } from '@/lib/kits'
import { useAutosave } from '@/lib/useAutosave'
import { cn } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'
import { ComposeOverrideEditor, composeOverrideError } from '../ComposeOverrideEditor'

export function ComposeOverridesTab() {
  const items = useAppStore((s) => s.composeOverrides)
  const projects = useAppStore((s) => s.projects)
  const createItem = useAppStore((s) => s.createComposeOverride)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const selected = items.find((o) => o.id === selectedId) ?? items[0]

  async function addItem() {
    const item = await createItem('New override')
    setSelectedId(item.id)
  }

  return (
    <div className="flex h-full min-h-0 gap-4 pt-4">
      <aside className="flex w-56 shrink-0 flex-col gap-1 overflow-auto">
        <Button size="sm" variant="outline" onClick={addItem}>
          <Plus className="size-3.5" />
          New override
        </Button>
        {items.map((item) => (
          <button
            key={item.id}
            type="button"
            onClick={() => setSelectedId(item.id)}
            className={cn(
              'flex flex-col rounded-lg px-2.5 py-2 text-left text-sm',
              item.id === selected?.id ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/60'
            )}
          >
            <span className="truncate">{item.name}</span>
            <span className="text-xs text-muted-foreground">{kitScopeLabel(item)}</span>
          </button>
        ))}
      </aside>
      {selected ? (
        <OverrideDetail key={selected.id} item={selected} items={items} projects={projects} />
      ) : (
        <p className="text-sm text-muted-foreground">
          No compose overrides yet. An override is written next to the workspace compose file, for example to publish
          ports or add a service, and kept out of git.
        </p>
      )}
    </div>
  )
}

function OverrideDetail({ item, items, projects }: { item: ComposeOverride; items: ComposeOverride[]; projects: Project[] }) {
  const updateItem = useAppStore((s) => s.updateComposeOverride)
  const setScope = useAppStore((s) => s.setComposeOverrideScope)
  const deleteItem = useAppStore((s) => s.deleteComposeOverride)
  const [draft, setDraft] = useState({ name: item.name, content: item.content })
  const [scopeError, setScopeError] = useState<string | null>(null)
  const [confirmingDelete, setConfirmingDelete] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const valid = composeOverrideError(draft.content) == null
  const { state, error } = useAutosave(draft, (d) => updateItem(item.id, d.name, d.content), !deleting && valid)

  async function remove() {
    setDeleting(true)
    try {
      await deleteItem(item.id)
    } catch (e) {
      setScopeError(String(e))
    }
  }

  async function applyScope(global: boolean, projectIds: string[]) {
    setScopeError(null)
    try {
      await setScope(item.id, global, projectIds)
    } catch (e) {
      setScopeError(String(e))
    }
  }

  function toggleProject(projectId: string) {
    const ids = item.project_ids.includes(projectId)
      ? item.project_ids.filter((id) => id !== projectId)
      : [...item.project_ids, projectId]
    applyScope(false, ids)
  }

  const notApplied = !item.is_global && item.project_ids.length === 0
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
          aria-label="Delete override"
          className="ml-auto"
          onClick={() => setConfirmingDelete(true)}
        >
          <Trash2 className="size-4" strokeWidth={1.5} />
        </Button>
      </div>

      <div className="flex flex-col gap-2">
        <div className="flex gap-2">
          <Button size="sm" variant={item.is_global ? 'default' : 'outline'} onClick={() => applyScope(true, [])}>
            Global
          </Button>
          <Button
            size="sm"
            variant={item.is_global ? 'outline' : 'default'}
            onClick={() => applyScope(false, item.project_ids)}
          >
            Projects
          </Button>
        </div>
        {!item.is_global && (
          <div className="flex flex-wrap gap-3">
            {projects.map((project) => {
              const other =
                project.compose_override_id && project.compose_override_id !== item.id
                  ? items.find((o) => o.id === project.compose_override_id)
                  : undefined
              return (
                <label key={project.id} className="flex items-center gap-1.5 text-sm">
                  <input
                    type="checkbox"
                    checked={item.project_ids.includes(project.id)}
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
          New sandboxes in these projects start with this override. Saving pushes it to running sandboxes that use
          it.
        </p>
      </div>

      <ComposeOverrideEditor value={draft.content} onChange={(content) => setDraft((d) => ({ ...d, content }))} />

      <Dialog open={confirmingDelete} onOpenChange={setConfirmingDelete}>
        <DialogContent title="Delete override?">
          <Card className="w-full">
            <CardHeader>
              <CardTitle>Delete {item.name}?</CardTitle>
            </CardHeader>
            <CardContent className="flex flex-col gap-3">
              <p className="text-sm text-muted-foreground">
                Running sandboxes that use it lose the override file. Projects that default to it fall back to the global
                override.
              </p>
              <div className="flex justify-end gap-2">
              <Button variant="outline" onClick={() => setConfirmingDelete(false)}>
                Cancel
              </Button>
              <Button variant="destructive" disabled={deleting} onClick={remove}>
                Delete
              </Button>
              </div>
            </CardContent>
          </Card>
        </DialogContent>
      </Dialog>
    </div>
  )
}
