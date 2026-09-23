import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { ChevronsUpDown, Code, Loader2, Play, Square } from 'lucide-react'
import type { Route } from '@/lib/router'
import { notify } from '@/lib/notify'
import { AGENT_ACCENT_CLASSES, AGENT_LABELS, type Agent } from '@/lib/agentHost'
import { AGENT_ICONS } from '@/components/agent-icons'
import { useAppStore } from '@/store/useAppStore'
import type { Sandbox } from './types'

const MAX_VISIBLE = 4

type BusyAction = 'start' | 'stop' | 'vscode' | 'agent' | null

export function SidebarSandboxList({ onNavigate }: { onNavigate: (route: Route) => void }) {
  const sandboxes = useAppStore((s) => s.sandboxes)
  const projects = useAppStore((s) => s.projects)
  const loadSandboxes = useAppStore((s) => s.loadSandboxes)
  const platform = useAppStore((s) => s.platform)
  const [busy, setBusy] = useState<{ id: string; action: BusyAction } | null>(null)

  async function run(sandbox: Sandbox, action: Exclude<BusyAction, null>, invoker: () => Promise<unknown>) {
    setBusy({ id: sandbox.id, action })
    try {
      await invoker()
      const title = sandbox.name ?? projects.find((p) => p.id === sandbox.project_id)?.name ?? 'Sandbox'
      if (action === 'start') notify('Sandbox started', `${title} is up and running.`, sandbox.id)
      if (action === 'stop') notify('Sandbox stopped', `${title} has been stopped.`, sandbox.id)
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
      <span className="px-2.5 text-xs font-normal text-muted-foreground">Sandboxes</span>
      {visible.map((sandbox) => {
        const projectName = projects.find((p) => p.id === sandbox.project_id)?.name ?? 'Unknown project'
        const isRunning = sandbox.status === 'running'
        const isStopping = sandbox.status === 'stopping'
        const isBusy = busy?.id === sandbox.id
        const AgentIcon = AGENT_ICONS[sandbox.agent as Agent] ?? AGENT_ICONS.claude

        return (
          <div
            key={sandbox.id}
            className="flex items-center gap-1 rounded-md px-2 py-1.5 text-sm text-sidebar-foreground hover:bg-sidebar-accent/60"
          >
            <button
              type="button"
              onClick={() => onNavigate({ screen: 'sandboxes', sandboxId: sandbox.id })}
              title={sandbox.name ?? projectName}
              className="flex-1 truncate text-left hover:underline"
            >
              {sandbox.name ?? projectName}
            </button>
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
                {platform === 'windows' && (
                  <button
                    type="button"
                    aria-label={`Run ${AGENT_LABELS[sandbox.agent as Agent] ?? sandbox.agent}`}
                    title={`Run ${AGENT_LABELS[sandbox.agent as Agent] ?? sandbox.agent}`}
                    disabled={isBusy}
                    onClick={() =>
                      run(sandbox, 'agent', () =>
                        invoke('open_sandbox_agent', { id: sandbox.id, agent: sandbox.agent }),
                      )
                    }
                    className={`flex size-6 items-center justify-center rounded border disabled:opacity-50 ${AGENT_ACCENT_CLASSES[sandbox.agent as Agent] ?? AGENT_ACCENT_CLASSES.claude}`}
                  >
                    {isBusy && busy?.action === 'agent' ? (
                      <Loader2 className="size-3 animate-spin" />
                    ) : (
                      <AgentIcon className="size-3" />
                    )}
                  </button>
                )}
              </>
            )}
            <button
              type="button"
              aria-label={isRunning ? 'Stop sandbox' : 'Start sandbox'}
              title={isRunning ? 'Stop sandbox' : 'Start sandbox'}
              disabled={isBusy || isStopping}
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
          onClick={() => onNavigate({ screen: 'sandboxes' })}
          className="flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-left text-xs text-muted-foreground hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground"
        >
          <ChevronsUpDown className="size-3" strokeWidth={1.5} />
          View More
        </button>
      )}
    </div>
  )
}
