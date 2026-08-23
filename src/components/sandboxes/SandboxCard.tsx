import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Code, ExternalLink, Loader2, Play, Square, TerminalSquare, Trash2 } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { notify } from '@/lib/notify'
import { permissionBadgeVariant, statusBadgeVariant } from '@/lib/sandboxDisplay'
import { useAppStore } from '@/store/useAppStore'
import type { Sandbox } from './types'

type BusyAction = 'start' | 'stop' | 'delete' | 'vscode' | 'terminal' | null

export function SandboxCard({
  sandbox,
  projectName,
  onChanged,
}: {
  sandbox: Sandbox
  projectName: string
  onChanged: () => void
}) {
  const projectRepoPath = useAppStore(
    (s) => s.projects.find((p) => p.id === sandbox.project_id)?.repo_path
  )
  const [busyAction, setBusyAction] = useState<BusyAction>(null)
  const [error, setError] = useState<string | null>(null)

  async function run(which: Exclude<BusyAction, null>, action: () => Promise<unknown>) {
    setBusyAction(which)
    setError(null)
    try {
      await action()
      const title = sandbox.name ?? projectName
      if (which === 'start') notify('Sandbox started', `${title} is up and running.`)
      if (which === 'stop') notify('Sandbox stopped', `${title} has been stopped.`)
      if (which === 'delete') notify('Sandbox deleted', `${title} has been deleted.`)
      onChanged()
    } catch (e) {
      setError(String(e))
    } finally {
      setBusyAction(null)
    }
  }

  function openInBrowser() {
    if (sandbox.host_port != null) {
      window.open(`http://localhost:${sandbox.host_port}`, '_blank')
    }
  }

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center gap-2">
          <CardTitle>{sandbox.name ?? projectName}</CardTitle>
          <Badge variant={statusBadgeVariant(sandbox.status)}>{sandbox.status}</Badge>
        </div>
      </CardHeader>
      <CardContent className="flex flex-col gap-1.5 text-sm text-muted-foreground">
        {sandbox.sbx_name && <span className="text-xs">{sandbox.sbx_name}</span>}
        {projectRepoPath && <span className="truncate">{projectRepoPath}</span>}
        <span className="text-xs">{sandbox.mode === 'clone' ? 'Cloned' : 'Mounted'}</span>
        <Badge variant={permissionBadgeVariant(sandbox.permission_mode)} className="w-fit">
          {sandbox.permission_mode}
        </Badge>

        {error && <p className="text-destructive">{error}</p>}

        <div className="mt-1 flex flex-wrap gap-2">
          {sandbox.status === 'running' && (
            <>
              <Button
                size="sm"
                variant="outline"
                disabled={busyAction != null}
                onClick={() => run('vscode', () => invoke('open_sandbox_vscode', { id: sandbox.id }))}
              >
                {busyAction === 'vscode' ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  <Code className="size-3.5" />
                )}
                Open in VS Code
              </Button>
              <Button
                size="sm"
                variant="outline"
                disabled={busyAction != null}
                onClick={() => run('terminal', () => invoke('open_sandbox_terminal', { id: sandbox.id }))}
              >
                {busyAction === 'terminal' ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  <TerminalSquare className="size-3.5" />
                )}
                Terminal
              </Button>
              {sandbox.host_port != null && (
                <Button size="sm" variant="outline" onClick={openInBrowser}>
                  <ExternalLink className="size-3.5" />
                  Open in browser
                </Button>
              )}
              <Button
                size="sm"
                variant="outline"
                disabled={busyAction != null}
                onClick={() => run('stop', () => invoke('stop_sandbox', { id: sandbox.id }))}
              >
                {busyAction === 'stop' ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  <Square className="size-3.5" />
                )}
                {busyAction === 'stop' ? 'Stopping…' : 'Stop'}
              </Button>
            </>
          )}
          {sandbox.status === 'stopped' && (
            <Button
              size="sm"
              variant="outline"
              disabled={busyAction != null}
              onClick={() => run('start', () => invoke('start_sandbox', { id: sandbox.id }))}
            >
              {busyAction === 'start' ? (
                <Loader2 className="size-3.5 animate-spin" />
              ) : (
                <Play className="size-3.5" />
              )}
              {busyAction === 'start' ? 'Starting…' : 'Start'}
            </Button>
          )}
          <Button
            size="sm"
            variant="destructive"
            disabled={busyAction != null}
            onClick={() => run('delete', () => invoke('delete_sandbox', { id: sandbox.id }))}
          >
            {busyAction === 'delete' ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <Trash2 className="size-3.5" />
            )}
            Delete
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
