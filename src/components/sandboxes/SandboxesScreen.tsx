import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Button } from '@/components/ui/button'
import type { Project } from '@/components/projects/types'
import { CreateSandboxDialog } from './CreateSandboxDialog'
import { LogsPanel } from './LogsPanel'
import { SandboxCard } from './SandboxCard'
import type { Sandbox } from './types'

export function SandboxesScreen() {
  const [dockerError, setDockerError] = useState<string | null>(null)
  const [checkingDocker, setCheckingDocker] = useState(true)
  const [sandboxes, setSandboxes] = useState<Sandbox[]>([])
  const [projects, setProjects] = useState<Project[]>([])
  const [creating, setCreating] = useState(false)
  const [viewingLogsFor, setViewingLogsFor] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  async function checkDocker() {
    setCheckingDocker(true)
    try {
      await invoke('docker_health_check')
      setDockerError(null)
    } catch (e) {
      setDockerError(String(e))
    } finally {
      setCheckingDocker(false)
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
    checkDocker()
    loadSandboxes()
  }, [])

  function projectName(projectId: string) {
    return projects.find((p) => p.id === projectId)?.name ?? 'Unknown project'
  }

  if (checkingDocker) {
    return (
      <div className="flex w-full flex-1 items-center justify-center p-6">
        <p className="text-sm text-muted-foreground">Checking Docker…</p>
      </div>
    )
  }

  if (dockerError) {
    return (
      <div className="flex w-full flex-1 items-center justify-center p-6">
        <div className="flex max-w-md flex-col items-center gap-3 text-center">
          <h1 className="text-lg font-medium">Docker isn't available</h1>
          <p className="text-sm text-muted-foreground">
            Sandboxes need a running Docker daemon. Make sure Docker is installed and running, then try again.
          </p>
          <p className="text-xs text-destructive">{dockerError}</p>
          <Button size="sm" onClick={checkDocker}>
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

      {viewingLogsFor && <LogsPanel sandboxId={viewingLogsFor} onClose={() => setViewingLogsFor(null)} />}

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
            onViewLogs={setViewingLogsFor}
          />
        ))}
      </div>
    </div>
  )
}
