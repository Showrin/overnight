import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { useAppStore } from '@/store/useAppStore'
import type { Sandbox } from './types'

const OFF = 'off'
const INTERVALS = [15, 30, 60, 240]

function label(minutes: number): string {
  return minutes < 60 ? `Every ${minutes} min` : `Every ${minutes / 60} h`
}

export function AutoGitSyncSelect({ sandbox }: { sandbox: Sandbox }) {
  const loadSandboxes = useAppStore((s) => s.loadSandboxes)
  const [error, setError] = useState<string | null>(null)
  const current = sandbox.git_sync_interval_minutes
  const options = current != null && !INTERVALS.includes(current) ? [...INTERVALS, current].sort((a, b) => a - b) : INTERVALS

  async function change(value: string) {
    setError(null)
    try {
      await invoke('save_sandbox_git_sync_interval', { id: sandbox.id, minutes: value === OFF ? null : Number(value) })
      await loadSandboxes()
    } catch (e) {
      setError(String(e))
    }
  }

  return (
    <div className="flex items-center gap-2">
      <span className="text-xs text-muted-foreground">Auto sync</span>
      <Select value={current == null ? OFF : String(current)} onValueChange={change}>
        <SelectTrigger className="h-8 w-36">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value={OFF}>Off</SelectItem>
          {options.map((m) => (
            <SelectItem key={m} value={String(m)}>
              {label(m)}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {error && <span className="text-xs text-destructive">{error}</span>}
    </div>
  )
}
