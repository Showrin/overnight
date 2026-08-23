import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import type { Sandbox, SandboxUsage } from './types'

const USAGE_POLL_MS = 5000

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
  const [busy, setBusy] = useState(false)
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

  async function run(action: () => Promise<unknown>) {
    setBusy(true)
    setError(null)
    try {
      await action()
      onChanged()
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(false)
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
        <CardTitle>{projectName}</CardTitle>
        <div className="flex gap-2">
          <Badge variant="outline">{sandbox.mode}</Badge>
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
              <Button size="sm" variant="outline" disabled={busy} onClick={() => run(() => invoke('open_sandbox_vscode', { id: sandbox.id }))}>
                Open in VS Code
              </Button>
              <Button size="sm" variant="outline" disabled={busy} onClick={() => run(() => invoke('open_sandbox_terminal', { id: sandbox.id }))}>
                Terminal
              </Button>
              {sandbox.host_port != null && (
                <Button size="sm" variant="outline" onClick={openInBrowser}>
                  Open in browser
                </Button>
              )}
              <Button size="sm" variant="outline" disabled={busy} onClick={() => run(() => invoke('stop_sandbox', { id: sandbox.id }))}>
                Stop
              </Button>
            </>
          )}
          {sandbox.status === 'stopped' && (
            <Button size="sm" variant="outline" disabled={busy} onClick={() => run(() => invoke('start_sandbox', { id: sandbox.id }))}>
              Start
            </Button>
          )}
          <Button size="sm" variant="destructive" disabled={busy} onClick={() => run(() => invoke('delete_sandbox', { id: sandbox.id }))}>
            Delete
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
