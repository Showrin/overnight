import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Loader2 } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { notify } from '@/lib/notify'
import type { Sandbox, SandboxUsage } from './types'

const USAGE_POLL_MS = 5000

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
  const [usage, setUsage] = useState<SandboxUsage | null>(null)
  const [busyAction, setBusyAction] = useState<BusyAction>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (sandbox.status !== 'running') return
    let cancelled = false

    async function poll() {
      try {
        const u = await invoke<SandboxUsage>('get_sandbox_usage', { id: sandbox.id })
        if (!cancelled) setUsage(u)
      } catch {
        // Best-effort — a single missed poll shouldn't show as a card error.
      }
    }

    poll()
    const interval = setInterval(poll, USAGE_POLL_MS)
    return () => {
      cancelled = true
      clearInterval(interval)
    }
  }, [sandbox.id, sandbox.status])

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

        {sandbox.status === 'running' && (
          <div className="flex flex-wrap gap-2">
            <Badge variant="outline">
              tokens {usage ? (usage.input_tokens + usage.output_tokens).toLocaleString() : '—'}
            </Badge>
          </div>
        )}

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
                {busyAction === 'vscode' && <Loader2 className="size-3.5 animate-spin" />}
                Open in VS Code
              </Button>
              <Button
                size="sm"
                variant="outline"
                disabled={busyAction != null}
                onClick={() => run('terminal', () => invoke('open_sandbox_terminal', { id: sandbox.id }))}
              >
                {busyAction === 'terminal' && <Loader2 className="size-3.5 animate-spin" />}
                Terminal
              </Button>
              {sandbox.host_port != null && (
                <Button size="sm" variant="outline" onClick={openInBrowser}>
                  Open in browser
                </Button>
              )}
              <Button
                size="sm"
                variant="outline"
                disabled={busyAction != null}
                onClick={() => run('stop', () => invoke('stop_sandbox', { id: sandbox.id }))}
              >
                {busyAction === 'stop' && <Loader2 className="size-3.5 animate-spin" />}
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
              {busyAction === 'start' && <Loader2 className="size-3.5 animate-spin" />}
              {busyAction === 'start' ? 'Starting…' : 'Start'}
            </Button>
          )}
          <Button
            size="sm"
            variant="destructive"
            disabled={busyAction != null}
            onClick={() => run('delete', () => invoke('delete_sandbox', { id: sandbox.id }))}
          >
            {busyAction === 'delete' && <Loader2 className="size-3.5 animate-spin" />}
            Delete
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
