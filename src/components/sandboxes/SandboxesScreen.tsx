import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { useAppStore } from '@/store/useAppStore'
import { CreateSandboxDialog } from './CreateSandboxDialog'
import { SandboxCard } from './SandboxCard'

export function SandboxesScreen() {
  const sandboxes = useAppStore((s) => s.sandboxes)
  const projects = useAppStore((s) => s.projects)
  const loadSandboxes = useAppStore((s) => s.loadSandboxes)
  const [sbxError, setSbxError] = useState<string | null>(null)
  const [checkingSbx, setCheckingSbx] = useState(true)
  const [creating, setCreating] = useState(false)

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

  function projectName(projectId: string) {
    return projects.find((p) => p.id === projectId)?.name ?? 'Unknown project'
  }

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

  return (
    <div className="flex w-full flex-col gap-4">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-medium">Sandboxes</h1>
        <Button size="sm" onClick={() => setCreating(true)} disabled={projects.length === 0}>
          New Sandbox
        </Button>
      </div>
      {projects.length === 0 && (
        <p className="text-sm text-muted-foreground">Create a project first, then start a sandbox for it.</p>
      )}
      <p className="text-xs text-muted-foreground">
        If this is the first sandbox created on this machine, `sbx` may prompt for a network policy the first time —
        run <code>sbx run claude</code> once yourself in a terminal beforehand if creation seems stuck.
      </p>

      <Dialog open={creating} onOpenChange={setCreating}>
        <DialogContent title="New sandbox">
          <CreateSandboxDialog
            onCreated={() => {
              setCreating(false)
              loadSandboxes()
            }}
            onCancel={() => setCreating(false)}
          />
        </DialogContent>
      </Dialog>

      {sandboxes.length === 0 && (
        <p className="text-sm text-muted-foreground">No sandboxes yet — click New Sandbox to start one.</p>
      )}
      <div className="flex flex-col gap-2">
        {sandboxes.map((sandbox) => (
          <SandboxCard
            key={sandbox.id}
            sandbox={sandbox}
            projectName={projectName(sandbox.project_id)}
            onChanged={loadSandboxes}
          />
        ))}
      </div>
    </div>
  )
}
