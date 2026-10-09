import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Loader2, PanelLeftClose, PanelLeftOpen, RefreshCw } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { AGENT_HOME_DIRS, type Agent } from '@/lib/agentHost'
import { MarkdownDocument } from './MarkdownDocument'
import type { PlanFile, Sandbox } from './types'

// Syncs on mount plus a manual "Resync" button — plans only change when the
// agent inside the sandbox writes new ones, so no auto-polling here (same
// reasoning as BranchesPage's "Refresh" button).
export function SandboxPlansTab({ sandbox }: { sandbox: Sandbox }) {
  const [plans, setPlans] = useState<PlanFile[]>([])
  const [selected, setSelected] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [listCollapsed, setListCollapsed] = useState(false)
  const isRunning = sandbox.status === 'running'

  async function load() {
    setLoading(true)
    setError(null)
    try {
      const result = await invoke<PlanFile[]>('sync_sandbox_plans', { id: sandbox.id })
      setPlans(result)
      setSelected((prev) => (prev && result.some((p) => p.name === prev) ? prev : (result[0]?.name ?? null)))
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    if (isRunning) load()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sandbox.id])

  const selectedPlan = plans.find((p) => p.name === selected) ?? null

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 text-sm">
      <div className="flex items-center justify-between">
        <span className="text-xs text-muted-foreground/70">
          Plans from {AGENT_HOME_DIRS[sandbox.agent as Agent] ?? AGENT_HOME_DIRS.claude}/plans
        </span>
        <Button size="sm" variant="outline" disabled={loading || !isRunning} onClick={load}>
          {loading ? <Loader2 className="size-3.5 animate-spin" /> : <RefreshCw className="size-3.5" />}
          Resync
        </Button>
      </div>

      {!isRunning && <p className="text-sm text-muted-foreground">Sandbox must be running to sync plans.</p>}
      {error && <p className="text-sm text-destructive">{error}</p>}

      {isRunning && !error && plans.length === 0 && (
        <p className="text-sm text-muted-foreground">{loading ? 'Syncing…' : 'No plans found.'}</p>
      )}

      {plans.length > 0 && (
        <div className="flex min-h-0 flex-1 gap-4">
          {listCollapsed ? (
            <div className="flex shrink-0 flex-col border-r border-border">
              <Button size="sm" variant="ghost" onClick={() => setListCollapsed(false)} title="Show plans">
                <PanelLeftOpen className="size-3.5" />
              </Button>
            </div>
          ) : (
            <div className="flex w-48 shrink-0 flex-col gap-1 overflow-auto border-r border-border pr-2">
              <div className="flex items-center justify-between px-1 pb-1">
                <span className="text-xs text-muted-foreground/70">Plans</span>
                <Button size="sm" variant="ghost" onClick={() => setListCollapsed(true)} title="Hide plans">
                  <PanelLeftClose className="size-3.5" />
                </Button>
              </div>
              {plans.map((plan) => (
                <button
                  key={plan.name}
                  type="button"
                  onClick={() => setSelected(plan.name)}
                  title={plan.name}
                  className={`truncate rounded-md px-2 py-1 text-left text-xs ${
                    plan.name === selected ? 'bg-muted text-foreground/70' : 'text-muted-foreground/70 hover:bg-muted/50'
                  }`}
                >
                  {plan.name}
                </button>
              ))}
            </div>
          )}

          <div className="min-h-0 min-w-0 flex-1 overflow-auto rounded-md border border-border p-4">
            {selectedPlan && <MarkdownDocument content={selectedPlan.content} />}
          </div>
        </div>
      )}
    </div>
  )
}
