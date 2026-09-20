import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { GitBranch, Loader2, RefreshCw } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { branchSyncBadgeVariant, branchSyncStatusLabel, formatRelativeTime, parseDiffStat } from '@/lib/sandboxDisplay'
import type { Route } from '@/lib/router'
import { cn } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'
import { SandboxHeader } from './SandboxHeader'
import type { BranchStat, SandboxDiffStats } from './types'

const BASE_BRANCH_UNKNOWN_COPY = 'Base branch unknown — this sandbox predates branch tracking.'

function BranchCard({ diff, onShowDiffs }: { diff: BranchStat; onShowDiffs: () => void }) {
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
  const [diff, setDiff] = useState<SandboxDiffStats | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function load() {
    setLoading(true)
    setError(null)
    try {
      const result = await invoke<SandboxDiffStats>('get_sandbox_diff_stats', { id: sandboxId })
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

  return (
    <div className="flex flex-col gap-6">
      <SandboxHeader
        sandbox={sandbox}
        project={project}
        navigate={navigate}
        activeTab="branches"
        breadcrumbExtra={[{ label: 'Branches' }]}
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

      {sandbox.mode === 'clone' && (
        <div className="mt-4 flex flex-col gap-3 border-t border-dashed border-border pt-8">
          <h2 className="text-base font-semibold text-foreground">Git Sync Output</h2>
          {sandbox.last_git_sync_at == null ? (
            <p className="text-sm text-muted-foreground">Not synced yet.</p>
          ) : (
            <>
              <p className="text-xs text-muted-foreground">
                Last synced {formatRelativeTime(sandbox.last_git_sync_at)}
              </p>
              {sandbox.last_git_sync_result.length === 0 ? (
                <p className="text-sm text-muted-foreground">No sandbox branches to sync.</p>
              ) : (
                <ul className="flex flex-col gap-1.5">
                  {sandbox.last_git_sync_result.map((o) => (
                    <li
                      key={o.branch}
                      className={cn(
                        'flex items-center justify-between gap-4 rounded-md border px-3 py-2 text-sm',
                        o.status === 'needs_manual_merge' ? 'border-warning/40 bg-warning/10' : 'border-border'
                      )}
                    >
                      <span className="truncate font-mono text-foreground">{o.branch}</span>
                      <Badge variant={branchSyncBadgeVariant(o.status)}>{branchSyncStatusLabel(o.status)}</Badge>
                    </li>
                  ))}
                </ul>
              )}
            </>
          )}
        </div>
      )}
    </div>
  )
}
