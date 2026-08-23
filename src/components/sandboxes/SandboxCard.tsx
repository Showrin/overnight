import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Code, ExternalLink, Loader2, Play, Square, TerminalSquare, Trash2 } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { notify } from '@/lib/notify'
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
  const [busyAction, setBusyAction] = useState<BusyAction>(null)
  const [error, setError] = useState<string | null>(null)

  async function run(which: Exclude<BusyAction, null>, action: () => Promise<unknown>) {
    setBusyAction(which)
    setError(null)
    try {
      await action()
      if (which === 'start') notify('Sandbox started', `${projectName} is up and running.`)
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
      <CardHeader className="flex flex-row items-center justify-between">
        <div className="flex flex-col">
          <CardTitle>{sandbox.name ?? projectName}</CardTitle>
          {sandbox.name && <span className="text-xs text-muted-foreground">{projectName}</span>}
        </div>
        <div className="flex gap-2">
          <Badge variant="outline">{sandbox.mode}</Badge>
          <Badge variant="outline">{sandbox.permission_mode}</Badge>
          <Badge
            variant={
              sandbox.status === 'running' ? 'default' : sandbox.status === 'error' ? 'destructive' : 'secondary'
            }
          >
            {sandbox.status}
          </Badge>
        </div>
      </CardHeader>
      <CardContent className="flex flex-col gap-3 text-sm text-muted-foreground">
        {sandbox.folder_path && <span className="truncate">{sandbox.folder_path}</span>}

        {error && <p className="text-destructive">{error}</p>}

        <div className="flex flex-wrap gap-2">
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
