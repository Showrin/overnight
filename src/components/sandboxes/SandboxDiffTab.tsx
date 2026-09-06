import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { ChevronDown, ChevronRight, Loader2, RefreshCw } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { DiffViewer } from './DiffViewer'
import type { BranchDiff, Sandbox, SandboxDiff } from './types'

const BASE_BRANCH_UNKNOWN_COPY = 'Base branch unknown — this sandbox predates branch tracking.'

function BranchDiffSection({ diff }: { diff: BranchDiff }) {
  const [expanded, setExpanded] = useState(false)

  return (
    <div className="flex flex-col gap-2 rounded-lg border border-border p-3">
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="flex items-start gap-2 text-left"
      >
        {expanded ? (
          <ChevronDown className="mt-0.5 size-3.5 shrink-0 text-muted-foreground" />
        ) : (
          <ChevronRight className="mt-0.5 size-3.5 shrink-0 text-muted-foreground" />
        )}
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <span className="truncate font-mono text-sm text-foreground">{diff.branch}</span>
          <pre className="overflow-x-auto whitespace-pre text-xs text-muted-foreground">
            {diff.stat.length > 0 ? diff.stat : 'No changes.'}
          </pre>
        </div>
      </button>

      {expanded && <DiffViewer patch={diff.patch} />}
    </div>
  )
}

// Loads get_sandbox_diff on mount plus a manual "Refresh" button — unlike
// the Metrics tab, diffs aren't continuously changing (they only move when
// someone commits), so there's no auto-polling here.
export function SandboxDiffTab({ sandbox }: { sandbox: Sandbox }) {
  const [diff, setDiff] = useState<SandboxDiff | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function load() {
    setLoading(true)
    setError(null)
    try {
      const result = await invoke<SandboxDiff>('get_sandbox_diff', { id: sandbox.id })
      setDiff(result)
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    load()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sandbox.id])

  return (
    <div className="flex flex-col gap-4 text-sm">
      <div className="flex items-center justify-between">
        <span className="text-xs text-muted-foreground/70">Changes against base branch</span>
        <Button size="sm" variant="outline" disabled={loading} onClick={load}>
          {loading ? <Loader2 className="size-3.5 animate-spin" /> : <RefreshCw className="size-3.5" />}
          Refresh
        </Button>
      </div>

      {error && <p className="text-sm text-destructive">{error}</p>}

      {!diff || diff.base_branch === null ? (
        <p className="text-sm text-muted-foreground">
          {loading && !diff ? 'Loading…' : BASE_BRANCH_UNKNOWN_COPY}
        </p>
      ) : diff.branches.length === 0 ? (
        <p className="text-sm text-muted-foreground">No branches to diff.</p>
      ) : (
        <div className="flex flex-col gap-2">
          {diff.branches.map((branchDiff) => (
            <BranchDiffSection key={branchDiff.branch} diff={branchDiff} />
          ))}
        </div>
      )}
    </div>
  )
}
