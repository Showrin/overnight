import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Label } from '@/components/ui/label'
import type { Project } from '@/components/projects/types'
import type { Sandbox, SandboxMode } from './types'

export function CreateSandboxDialog({
  projects,
  onCreated,
  onCancel,
}: {
  projects: Project[]
  onCreated: () => void
  onCancel: () => void
}) {
  const [projectId, setProjectId] = useState(projects[0]?.id ?? '')
  const [mode, setMode] = useState<SandboxMode>('mount')
  const [creating, setCreating] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [needsPolicyInit, setNeedsPolicyInit] = useState(false)
  const [initializingPolicy, setInitializingPolicy] = useState(false)

  async function handleCreate() {
    setCreating(true)
    setError(null)
    setNeedsPolicyInit(false)
    try {
      await invoke<Sandbox>('create_sandbox', { projectId, mode })
      onCreated()
    } catch (e) {
      const message = String(e)
      setError(message)
      if (message.includes("network policy hasn't been initialized")) {
        setNeedsPolicyInit(true)
      }
    } finally {
      setCreating(false)
    }
  }

  async function initPolicyAndRetry(preset: 'allow-all' | 'balanced' | 'deny-all') {
    setInitializingPolicy(true)
    setError(null)
    try {
      await invoke('init_sbx_policy', { preset })
      setNeedsPolicyInit(false)
      await handleCreate()
    } catch (e) {
      setError(String(e))
    } finally {
      setInitializingPolicy(false)
    }
  }

  return (
    <Card className="w-96">
      <CardHeader>
        <CardTitle>New sandbox</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <div className="flex flex-col gap-1">
          <Label htmlFor="sandbox-project">Project</Label>
          <select
            id="sandbox-project"
            value={projectId}
            onChange={(e) => setProjectId(e.target.value)}
            className="h-8 rounded-lg border border-border bg-background px-2.5 text-sm"
          >
            {projects.map((project) => (
              <option key={project.id} value={project.id}>
                {project.name}
              </option>
            ))}
          </select>
        </div>
        <div className="flex flex-col gap-1">
          <Label>Mode</Label>
          <div className="flex gap-2">
            <Button
              type="button"
              size="sm"
              variant={mode === 'mount' ? 'default' : 'outline'}
              onClick={() => setMode('mount')}
            >
              Mount existing folder
            </Button>
            <Button
              type="button"
              size="sm"
              variant={mode === 'clone' ? 'default' : 'outline'}
              onClick={() => setMode('clone')}
            >
              Clone fresh copy
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">
            {mode === 'mount'
              ? 'Runs directly against the project\'s existing local folder — edits appear on your host immediately. Only one mount-mode sandbox can run per project at a time.'
              : 'Clones the project\'s repo into an isolated copy inside the sandbox itself. Your local folder is untouched. Multiple clone-mode sandboxes can run per project.'}
          </p>
        </div>
        {error && <p className="text-sm text-destructive">{error}</p>}

        {needsPolicyInit && (
          <div className="flex flex-col gap-2 rounded-lg border border-border p-2.5">
            <p className="text-xs text-muted-foreground">
              sbx needs a one-time network policy for this machine before it can create sandboxes. Balanced is a
              good default — allows common dev services, blocks everything else.
            </p>
            <div className="flex flex-wrap gap-2">
              <Button size="sm" variant="outline" disabled={initializingPolicy} onClick={() => initPolicyAndRetry('allow-all')}>
                Open
              </Button>
              <Button size="sm" disabled={initializingPolicy} onClick={() => initPolicyAndRetry('balanced')}>
                Balanced (recommended)
              </Button>
              <Button size="sm" variant="outline" disabled={initializingPolicy} onClick={() => initPolicyAndRetry('deny-all')}>
                Locked down
              </Button>
            </div>
          </div>
        )}

        <div className="flex gap-2">
          <Button className="flex-1" onClick={handleCreate} disabled={creating || !projectId}>
            {creating ? 'Starting…' : 'Start sandbox'}
          </Button>
          <Button variant="outline" onClick={onCancel} disabled={creating}>
            Cancel
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
