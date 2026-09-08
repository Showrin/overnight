import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { GitBranch, Loader2, RefreshCw } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { parseDiffStat } from '@/lib/sandboxDisplay'
import type { Route } from '@/lib/router'
import { useAppStore } from '@/store/useAppStore'
import { SandboxBreadcrumb } from './SandboxBreadcrumb'
import type { BranchDiff, SandboxDiff } from './types'

const BASE_BRANCH_UNKNOWN_COPY = 'Base branch unknown — this sandbox predates branch tracking.'

function BranchCard({ diff, onShowDiffs }: { diff: BranchDiff; onShowDiffs: () => void }) {
  const stats = parseDiffStat(diff.stat)

  return (
    <div className="flex items-center justify-between gap-4 rounded-lg border border-border p-3">
      <div className="flex min-w-0 flex-col gap-1">
        <span className="truncate font-mono text-sm text-foreground">{diff.branch}</span>
        <span className="text-xs text-muted-foreground">
          {stats.filesChanged} file{stats.filesChanged === 1 ? '' : 's'} changed
          {stats.insertions > 0 && <span className="text-success"> +{stats.insertions}</span>}
          {stats.deletions > 0 && <span className="text-destructive"> -{stats.deletions}</span>}
        </span>
      </div>
      <Button size="sm" variant="outline" onClick={onShowDiffs}>
        Show Diffs
      </Button>
    </div>
  )
}

export function BranchesPage({ sandboxId, navigate }: { sandboxId: string; navigate: (route: Route) => void }) {
  const sandbox = useAppStore((s) => s.sandboxes.find((sb) => sb.id === sandboxId))
  const project = useAppStore((s) => s.projects.find((p) => p.id === sandbox?.project_id))
  const [diff, setDiff] = useState<SandboxDiff | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function load() {
    setLoading(true)
    setError(null)
    try {
      const result = await invoke<SandboxDiff>('get_sandbox_diff', { id: sandboxId })
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
  }, [sandboxId])

  if (!sandbox) {
    return <p className="text-sm text-muted-foreground">This sandbox couldn't be found — it may have been deleted.</p>
  }

  const title = sandbox.name ?? project?.name ?? 'Sandbox'

  return (
    <div className="flex flex-col gap-6">
      <SandboxBreadcrumb
        segments={[
          { label: 'Sandboxes', onClick: () => navigate({ screen: 'sandboxes' }) },
          { label: title, onClick: () => navigate({ screen: 'sandboxes', sandboxId }) },
          { label: 'Branches' },
        ]}
      />

      <div className="flex items-center justify-between">
        <h1 className="text-lg font-medium">Branches</h1>
        <Button size="sm" variant="outline" disabled={loading} onClick={load}>
          {loading ? <Loader2 className="size-3.5 animate-spin" /> : <RefreshCw className="size-3.5" />}
          Refresh
        </Button>
      </div>

      {error && <p className="text-sm text-destructive">{error}</p>}

      {!diff || diff.base_branch === null ? (
        <p className="text-sm text-muted-foreground">{loading && !diff ? 'Loading…' : BASE_BRANCH_UNKNOWN_COPY}</p>
      ) : diff.branches.length === 0 ? (
        <p className="flex items-center gap-2 text-sm text-muted-foreground">
          <GitBranch className="size-4" />
          No branches to diff.
        </p>
      ) : (
        <div className="flex flex-col gap-2">
          {diff.branches.map((branchDiff) => (
            <BranchCard
              key={branchDiff.branch}
              diff={branchDiff}
              onShowDiffs={() => navigate({ screen: 'sandboxes', sandboxId, branches: true, branch: branchDiff.branch })}
            />
          ))}
        </div>
      )}
    </div>
  )
}
