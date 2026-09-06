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
  addNewDisabledReason,
  onChanged,
  highlightedSandboxId,
}: {
  title: string
  sandboxes: Sandbox[]
  projectName: string
  onAddNew?: () => void
  addNewDisabledReason?: string
  onChanged: () => void
  highlightedSandboxId?: string | null
}) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <span className="text-xs font-normal text-muted-foreground">{title}</span>
        {onAddNew && (
          <Button
            size="sm"
            variant="outline"
            onClick={onAddNew}
            disabled={!!addNewDisabledReason}
            title={addNewDisabledReason}
          >
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
            <SandboxCard
              key={sandbox.id}
              sandbox={sandbox}
              projectName={projectName}
              onChanged={onChanged}
              highlighted={sandbox.id === highlightedSandboxId}
            />
          ))}
        </div>
      )}
    </div>
  )
}

export function SandboxesScreen({
  highlightedSandboxId,
}: {
  highlightedSandboxId?: string | null
} = {}) {
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
    <div className="flex w-full flex-col gap-6 @min-[820px]:flex-row">
      <div className="flex flex-1 flex-col gap-6">
        <h1 className="text-lg font-medium">Sandboxes</h1>
        <p className="text-xs text-muted-foreground">
          If this is the first sandbox created on this machine, `sbx` may prompt
          for a network policy the first time — run <code>sbx run claude</code>{" "}
          once yourself in a terminal beforehand if creation seems stuck.
        </p>

        <Dialog
          open={creatingForProjectId != null}
          onOpenChange={(open) => !open && setCreatingForProjectId(null)}
        >
          <DialogContent title="New sandbox">
            {creatingForProjectId && (
              <CreateSandboxDialog
                defaultProjectId={creatingForProjectId}
                onCreated={() => {
                  setCreatingForProjectId(null);
                  loadSandboxes();
                }}
                onCancel={() => setCreatingForProjectId(null)}
              />
            )}
          </DialogContent>
        </Dialog>

        {projects.length === 0 && (
          <p className="text-sm text-muted-foreground">
            Create a project first, then start a sandbox for it.
          </p>
        )}

        {projects.map((project) => {
          const projectSandboxes = sandboxes.filter(
            (sb) => sb.project_id === project.id,
          );
          const hasActiveMount = projectSandboxes.some(
            (sb) =>
              sb.mode === "mount" &&
              (sb.status === "starting" || sb.status === "running"),
          );
          const hasStarting = projectSandboxes.some((sb) => sb.status === 'starting')
          const addNewDisabledReason = hasActiveMount
            ? 'A mount-mode sandbox is already running for this project'
            : hasStarting
              ? 'A sandbox is already starting for this project'
              : undefined
          return (
            <SandboxGroup
              key={project.id}
              title={project.name}
              projectName={project.name}
              sandboxes={projectSandboxes}
              onAddNew={() => setCreatingForProjectId(project.id)}
              addNewDisabledReason={addNewDisabledReason}
              onChanged={loadSandboxes}
              highlightedSandboxId={highlightedSandboxId}
            />
          );
        })}

        {orphanSandboxes.length > 0 && (
          <SandboxGroup
            title="Unknown project"
            projectName="Unknown project"
            sandboxes={orphanSandboxes}
            onChanged={loadSandboxes}
            highlightedSandboxId={highlightedSandboxId}
          />
        )}
      </div>

      <div className="hidden w-72 shrink-0 self-start @min-[820px]:block @min-[820px]:sticky @min-[820px]:top-6">
        <HostStatsPanel />
      </div>

      <div className="sticky bottom-0 z-10 -mx-6 -mb-6 border-t border-border-subtle bg-background p-4 @min-[820px]:hidden">
        <HostStatsPanel layout="bar" />
      </div>
    </div>
  );
}
