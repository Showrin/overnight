import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Loader2, RefreshCw } from 'lucide-react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { Button } from '@/components/ui/button'
import type { PlanFile, Sandbox } from './types'

// Obsidian-like read-only markdown styling via Tailwind's arbitrary child
// selectors — no @tailwindcss/typography plugin is installed, and this is
// the only place in the app that renders arbitrary markdown, so adding one
// just for this wasn't worth it.
const MARKDOWN_CLASS =
  'flex flex-col gap-5 text-sm text-foreground/70 ' +
  '[&_h1]:mt-3 [&_h1]:text-lg [&_h1]:font-semibold [&_h2]:mt-3 [&_h2]:text-base [&_h2]:font-semibold [&_h3]:mt-2 [&_h3]:font-medium ' +
  '[&_p]:leading-relaxed [&_ul]:list-disc [&_ul]:space-y-1 [&_ul]:pl-5 [&_ol]:list-decimal [&_ol]:space-y-1 [&_ol]:pl-5 ' +
  '[&_code]:rounded [&_code]:bg-muted [&_code]:px-1 [&_code]:py-0.5 [&_code]:font-mono [&_code]:text-xs ' +
  '[&_pre]:overflow-x-auto [&_pre]:rounded-md [&_pre]:border [&_pre]:border-border [&_pre]:bg-muted/30 [&_pre]:p-3 ' +
  '[&_blockquote]:border-l-2 [&_blockquote]:border-border [&_blockquote]:pl-3 [&_blockquote]:text-muted-foreground ' +
  '[&_a]:text-primary [&_a]:underline'

// Syncs on mount plus a manual "Resync" button — plans only change when the
// agent inside the sandbox writes new ones, so no auto-polling here (same
// reasoning as SandboxDiffTab).
export function SandboxPlansTab({ sandbox }: { sandbox: Sandbox }) {
  const [plans, setPlans] = useState<PlanFile[]>([])
  const [selected, setSelected] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
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
    <div className="flex flex-col gap-4 text-sm">
      <div className="flex items-center justify-between">
        <span className="text-xs text-muted-foreground/70">Plans from /.claude/plans</span>
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
        <div className="flex gap-4">
          <div className="flex w-48 shrink-0 flex-col gap-1">
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

          <div className="min-w-0 flex-1 rounded-md border border-border p-4">
            {selectedPlan && (
              <div className={MARKDOWN_CLASS}>
                <ReactMarkdown remarkPlugins={[remarkGfm]}>{selectedPlan.content}</ReactMarkdown>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
