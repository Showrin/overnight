import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { AlertTriangle, Loader2, RotateCw } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { SETUP_STEP_LABELS } from '@/lib/setupSteps'
import { useAppStore } from '@/store/useAppStore'
import type { Sandbox } from './types'

export function SetupFailuresBanner({ sandbox }: { sandbox: Sandbox }) {
  const loadSandboxes = useAppStore((s) => s.loadSandboxes)
  const [retrying, setRetrying] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const failures = sandbox.setup_failures ?? []
  if (failures.length === 0 || sandbox.status !== 'running') return null

  async function retry() {
    setRetrying(true)
    setError(null)
    try {
      await invoke('retry_sandbox_setup', { id: sandbox.id })
      await loadSandboxes()
    } catch (e) {
      setError(String(e))
    } finally {
      setRetrying(false)
    }
  }

  return (
    <div className="flex flex-col gap-2 rounded-lg border border-amber-500/40 bg-amber-500/10 p-3 text-sm">
      <div className="flex items-center gap-2">
        <AlertTriangle className="size-4 shrink-0 text-amber-500" />
        <span className="font-medium">Setup incomplete: {failures.map((f) => SETUP_STEP_LABELS[f.step] ?? f.step).join(', ')}</span>
        <Button size="sm" variant="outline" className="ml-auto" onClick={retry} disabled={retrying}>
          {retrying ? <Loader2 className="size-3.5 animate-spin" /> : <RotateCw className="size-3.5" />}
          Retry setup
        </Button>
      </div>
      <ul className="flex flex-col gap-1 text-xs text-muted-foreground">
        {failures.map((f) => (
          <li key={f.step} className="break-all">
            <span className="text-foreground">{SETUP_STEP_LABELS[f.step] ?? f.step}:</span> {f.error}
          </li>
        ))}
      </ul>
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  )
}
