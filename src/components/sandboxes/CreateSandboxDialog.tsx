import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Copy, Loader2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { notify } from '@/lib/notify'
import { PERMISSION_MODES } from '@/lib/permissionModes'
import { useAppStore } from '@/store/useAppStore'
import type { Sandbox, SandboxMode } from './types'

const DEFAULT_PERMISSION_MODE = '__settings_default__'

function ErrorDetails({ message }: { message: string }) {
  const [copied, setCopied] = useState(false)

  if (!message.includes('\n')) {
    return <p className="text-sm text-destructive">{message}</p>
  }

  async function copy() {
    await navigator.clipboard.writeText(message)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between">
        <p className="text-sm text-destructive">Sandbox couldn't start</p>
        <Button size="sm" variant="outline" onClick={copy} type="button">
          <Copy className="size-3.5" />
          {copied ? 'Copied' : 'Copy'}
        </Button>
      </div>
      <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded-lg border border-border bg-muted p-2.5 text-xs text-foreground">
        {message}
      </pre>
    </div>
  )
}

export function CreateSandboxDialog({
  defaultProjectId,
  onCreated,
  onCancel,
}: {
  defaultProjectId?: string
  onCreated: () => void
  onCancel: () => void
}) {
  const projects = useAppStore((s) => s.projects)
  const sandboxes = useAppStore((s) => s.sandboxes)
  const [projectId, setProjectId] = useState(defaultProjectId ?? projects[0]?.id ?? '')
  const [name, setName] = useState('')
  const [permissionMode, setPermissionMode] = useState('')
  const [mode, setMode] = useState<SandboxMode>('mount')
  const [creating, setCreating] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [needsPolicyInit, setNeedsPolicyInit] = useState(false)
  const [initializingPolicy, setInitializingPolicy] = useState(false)

  // Sourced from the polled store, not local state, so this survives a
  // reload mid-creation instead of relying on `creating` alone.
  const hasActiveMount =
    mode === 'mount' &&
    sandboxes.some(
      (sb) => sb.project_id === projectId && sb.mode === 'mount' && (sb.status === 'starting' || sb.status === 'running')
    )
  const hasStartingSandbox = sandboxes.some((sb) => sb.project_id === projectId && sb.status === 'starting')

  async function handleCreate() {
    setCreating(true)
    setError(null)
    setNeedsPolicyInit(false)
    try {
      const sandbox = await invoke<Sandbox>('create_sandbox', {
        projectId,
        mode,
        name: name.trim() || null,
        permissionMode: permissionMode || null,
      })
      notify('Sandbox started', 'Your sandbox is up and running.', sandbox.id)
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
    <Card className="w-full">
      <CardHeader>
        <CardTitle>New sandbox</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <div className="flex flex-col gap-1">
          <Label htmlFor="sandbox-project">Project</Label>
          <Select value={projectId} onValueChange={setProjectId}>
            <SelectTrigger id="sandbox-project">
              <SelectValue placeholder="Select a project" />
            </SelectTrigger>
            <SelectContent>
              {projects.map((project) => (
                <SelectItem key={project.id} value={project.id}>
                  {project.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="sandbox-name">Name (optional)</Label>
          <Input
            id="sandbox-name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={projects.find((p) => p.id === projectId)?.name}
          />
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="sandbox-permission-mode">Permission mode</Label>
          <Select
            value={permissionMode || DEFAULT_PERMISSION_MODE}
            onValueChange={(value) =>
              setPermissionMode(value === DEFAULT_PERMISSION_MODE ? '' : value)
            }
          >
            <SelectTrigger id="sandbox-permission-mode">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={DEFAULT_PERMISSION_MODE}>Use Settings default</SelectItem>
              {PERMISSION_MODES.map((m) => (
                <SelectItem key={m} value={m}>
                  {m}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
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
        {hasActiveMount && (
          <p className="text-xs text-destructive">A mount-mode sandbox is already running for this project.</p>
        )}
        {!hasActiveMount && hasStartingSandbox && (
          <p className="text-xs text-destructive">A sandbox is already starting for this project.</p>
        )}
        {error && <ErrorDetails message={error} />}

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
          <Button
            className="flex-1"
            onClick={handleCreate}
            disabled={creating || !projectId || hasActiveMount || hasStartingSandbox}
          >
            {creating && <Loader2 className="size-3.5 animate-spin" />}
            {creating
              ? 'Starting…'
              : hasActiveMount
                ? 'Sandbox running'
                : hasStartingSandbox
                  ? 'Sandbox starting…'
                  : 'Start sandbox'}
          </Button>
          <Button variant="outline" onClick={onCancel} disabled={creating}>
            Cancel
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
