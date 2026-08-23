import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { ChevronsUpDown, Code, Loader2, Play, Square, TerminalSquare } from 'lucide-react'
import type { Screen } from '@/components/sidebar'
import { useAppStore } from '@/store/useAppStore'
import type { Sandbox } from './types'

const MAX_VISIBLE = 4

type BusyAction = 'start' | 'stop' | 'vscode' | 'terminal' | null

export function SidebarSandboxList({ onNavigate }: { onNavigate: (screen: Screen) => void }) {
  const sandboxes = useAppStore((s) => s.sandboxes)
  const projects = useAppStore((s) => s.projects)
  const loadSandboxes = useAppStore((s) => s.loadSandboxes)
  const [busy, setBusy] = useState<{ id: string; action: BusyAction } | null>(null)

  async function run(sandbox: Sandbox, action: Exclude<BusyAction, null>, invoker: () => Promise<unknown>) {
    setBusy({ id: sandbox.id, action })
    try {
      await invoker()
      await loadSandboxes()
    } catch {
      // Best-effort — surfacing errors here would need its own UI; the full
      // Sandboxes screen already shows detailed error state.
    } finally {
      setBusy(null)
    }
  }

  if (sandboxes.length === 0) return null

  const visible = sandboxes.slice(0, MAX_VISIBLE)
  const remaining = sandboxes.length - visible.length

  return (
    <div className="flex flex-col gap-1 border-t border-sidebar-border p-2">
      <span className="px-2.5 text-xs font-medium tracking-wide text-muted-foreground uppercase">Sandboxes</span>
      {visible.map((sandbox) => {
        const projectName = projects.find((p) => p.id === sandbox.project_id)?.name ?? 'Unknown project'
        const isRunning = sandbox.status === 'running'
        const isBusy = busy?.id === sandbox.id

        return (
          <div
            key={sandbox.id}
            className="flex items-center gap-1 rounded-md px-2 py-1.5 text-sm text-sidebar-foreground hover:bg-sidebar-accent/60"
          >
            <span className="flex-1 truncate" title={sandbox.name ?? projectName}>
              {sandbox.name ?? projectName}
            </span>
            {isRunning && (
              <>
                <button
                  type="button"
                  aria-label="Open in VS Code"
                  title="Open in VS Code"
                  disabled={isBusy}
                  onClick={() =>
                    run(sandbox, 'vscode', () => invoke('open_sandbox_vscode', { id: sandbox.id }))
                  }
                  className="flex size-6 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-accent-foreground disabled:opacity-50"
                >
                  {isBusy && busy?.action === 'vscode' ? (
                    <Loader2 className="size-3 animate-spin" />
                  ) : (
                    <Code className="size-3" strokeWidth={1.5} />
                  )}
                </button>
                <button
                  type="button"
                  aria-label="Open terminal"
                  title="Open terminal"
                  disabled={isBusy}
                  onClick={() =>
                    run(sandbox, 'terminal', () => invoke('open_sandbox_terminal', { id: sandbox.id }))
                  }
                  className="flex size-6 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-accent-foreground disabled:opacity-50"
                >
                  {isBusy && busy?.action === 'terminal' ? (
                    <Loader2 className="size-3 animate-spin" />
                  ) : (
                    <TerminalSquare className="size-3" strokeWidth={1.5} />
                  )}
                </button>
              </>
            )}
            <button
              type="button"
              aria-label={isRunning ? 'Stop sandbox' : 'Start sandbox'}
              title={isRunning ? 'Stop sandbox' : 'Start sandbox'}
              disabled={isBusy}
              onClick={() =>
                run(
                  sandbox,
                  isRunning ? 'stop' : 'start',
                  () => invoke(isRunning ? 'stop_sandbox' : 'start_sandbox', { id: sandbox.id })
                )
              }
              className="flex size-6 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-accent-foreground disabled:opacity-50"
            >
              {isBusy && (busy?.action === 'start' || busy?.action === 'stop') ? (
                <Loader2 className="size-3 animate-spin" />
              ) : isRunning ? (
                <Square className="size-3" strokeWidth={1.5} />
              ) : (
                <Play className="size-3" strokeWidth={1.5} />
              )}
            </button>
          </div>
        )
      })}
      {remaining > 0 && (
        <button
          type="button"
          onClick={() => onNavigate('sandboxes')}
          className="flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-left text-xs text-muted-foreground hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground"
        >
          <ChevronsUpDown className="size-3" strokeWidth={1.5} />
          View More
        </button>
      )}
    </div>
  )
}
