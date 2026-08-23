import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Plus } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { useAppStore } from '@/store/useAppStore'
import { CreateSandboxDialog } from './CreateSandboxDialog'
import { HostStatsPanel } from './HostStatsPanel'
import { SandboxCard } from './SandboxCard'
import type { Sandbox } from './types'

function SandboxGroup({
  title,
  sandboxes,
  projectName,
  onAddNew,
  onChanged,
}: {
  title: string
  sandboxes: Sandbox[]
  projectName: string
  onAddNew?: () => void
  onChanged: () => void
}) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <span className="text-xs font-normal text-muted-foreground">{title}</span>
        {onAddNew && (
          <Button size="sm" variant="outline" onClick={onAddNew}>
            <Plus className="size-3.5" />
            Add New
          </Button>
        )}
      </div>
      {sandboxes.length === 0 ? (
        <p className="text-sm text-muted-foreground">No sandboxes yet.</p>
      ) : (
        <div className="flex flex-col gap-2">
          {sandboxes.map((sandbox) => (
            <SandboxCard key={sandbox.id} sandbox={sandbox} projectName={projectName} onChanged={onChanged} />
          ))}
        </div>
      )}
    </div>
  )
}

export function SandboxesScreen() {
  const sandboxes = useAppStore((s) => s.sandboxes)
  const projects = useAppStore((s) => s.projects)
  const loadSandboxes = useAppStore((s) => s.loadSandboxes)
  const [sbxError, setSbxError] = useState<string | null>(null)
  const [checkingSbx, setCheckingSbx] = useState(true)
  const [creatingForProjectId, setCreatingForProjectId] = useState<string | null>(null)

  async function checkSbx() {
    setCheckingSbx(true)
    try {
      await invoke('sbx_health_check')
      setSbxError(null)
    } catch (e) {
      setSbxError(String(e))
    } finally {
      setCheckingSbx(false)
    }
  }

  useEffect(() => {
    checkSbx()
  }, [])

  if (checkingSbx) {
    return (
      <div className="flex w-full items-center justify-center">
        <p className="text-sm text-muted-foreground">Checking sbx…</p>
      </div>
    )
  }

  if (sbxError) {
    return (
      <div className="flex w-full items-center justify-center">
        <div className="flex max-w-md flex-col items-center gap-3 text-center">
          <h1 className="text-lg font-medium">sbx isn't available</h1>
          <p className="text-sm text-muted-foreground">
            Sandboxes need the <code>sbx</code> CLI (Docker Sandboxes) installed and signed in. Run{' '}
            <code>sbx login</code> in a terminal, then try again.
          </p>
          <p className="text-xs text-destructive">{sbxError}</p>
          <Button size="sm" onClick={checkSbx}>
            Retry
          </Button>
        </div>
      </div>
    )
  }

  const orphanSandboxes = sandboxes.filter((sb) => !projects.some((p) => p.id === sb.project_id))

  return (
    <div className="flex w-full gap-6">
      <div className="flex flex-1 flex-col gap-6">
        <h1 className="text-lg font-medium">Sandboxes</h1>
        <p className="text-xs text-muted-foreground">
          If this is the first sandbox created on this machine, `sbx` may prompt for a network policy the first time —
          run <code>sbx run claude</code> once yourself in a terminal beforehand if creation seems stuck.
        </p>

        <Dialog open={creatingForProjectId != null} onOpenChange={(open) => !open && setCreatingForProjectId(null)}>
          <DialogContent title="New sandbox">
            {creatingForProjectId && (
              <CreateSandboxDialog
                defaultProjectId={creatingForProjectId}
                onCreated={() => {
                  setCreatingForProjectId(null)
                  loadSandboxes()
                }}
                onCancel={() => setCreatingForProjectId(null)}
              />
            )}
          </DialogContent>
        </Dialog>

        {projects.length === 0 && (
          <p className="text-sm text-muted-foreground">Create a project first, then start a sandbox for it.</p>
        )}

        {projects.map((project) => (
          <SandboxGroup
            key={project.id}
            title={project.name}
            projectName={project.name}
            sandboxes={sandboxes.filter((sb) => sb.project_id === project.id)}
            onAddNew={() => setCreatingForProjectId(project.id)}
            onChanged={loadSandboxes}
          />
        ))}

        {orphanSandboxes.length > 0 && (
          <SandboxGroup
            title="Unknown project"
            projectName="Unknown project"
            sandboxes={orphanSandboxes}
            onChanged={loadSandboxes}
          />
        )}
      </div>

      <div className="w-72 shrink-0">
        <HostStatsPanel />
      </div>
    </div>
  )
}
