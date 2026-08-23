import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Button } from '@/components/ui/button'
import type { Project } from '@/components/projects/types'
import { CreateSandboxDialog } from './CreateSandboxDialog'
import { SandboxCard } from './SandboxCard'
import type { Sandbox } from './types'

export function SandboxesScreen() {
  const [sbxError, setSbxError] = useState<string | null>(null)
  const [checkingSbx, setCheckingSbx] = useState(true)
  const [sandboxes, setSandboxes] = useState<Sandbox[]>([])
  const [projects, setProjects] = useState<Project[]>([])
  const [creating, setCreating] = useState(false)
  const [error, setError] = useState<string | null>(null)

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

  async function loadSandboxes() {
    setError(null)
    try {
      const [sandboxList, projectList] = await Promise.all([
        invoke<Sandbox[]>('list_sandboxes'),
        invoke<Project[]>('list_projects'),
      ])
      setSandboxes(sandboxList)
      setProjects(projectList)
    } catch (e) {
      setError(String(e))
    }
  }

  useEffect(() => {
    checkSbx()
    loadSandboxes()
  }, [])

  function projectName(projectId: string) {
    return projects.find((p) => p.id === projectId)?.name ?? 'Unknown project'
  }

  if (checkingSbx) {
    return (
      <div className="flex w-full flex-1 items-center justify-center p-6">
        <p className="text-sm text-muted-foreground">Checking sbx…</p>
      </div>
    )
  }

  if (sbxError) {
    return (
      <div className="flex w-full flex-1 items-center justify-center p-6">
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
    <div className="flex w-full max-w-3xl flex-col gap-4 p-6">
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
      {error && <p className="text-sm text-destructive">{error}</p>}

      {creating && (
        <CreateSandboxDialog
          projects={projects}
          onCreated={() => {
            setCreating(false)
            loadSandboxes()
          }}
          onCancel={() => setCreating(false)}
        />
      )}

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
