import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { GitBranch, Loader2, RefreshCw } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { branchSyncBadgeVariant, branchSyncStatusLabel, formatRelativeTime, parseDiffStat } from '@/lib/sandboxDisplay'
import type { Route } from '@/lib/router'
import { dropStaleCompareBases, setCompareBase, useCompareBases } from '@/lib/compareBase'
import { cn } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'
import { AutoGitSyncSelect } from './AutoGitSyncSelect'
import { BaseBranchPicker } from './BaseBranchPicker'
import { useHostBranches, type HostBranches } from './useHostBranches'
import { SandboxHeader } from './SandboxHeader'
import type { BranchStat, SandboxDiffStats } from './types'

function BranchCard({
  diff,
  hostBranches,
  defaultBase,
  override,
  onChangeBase,
  onShowDiffs,
}: {
  diff: BranchStat
  hostBranches: HostBranches
  defaultBase: string | null
  override: string | null
  onChangeBase: (base: string | null) => void
  onShowDiffs: () => void
}) {
  const stats = parseDiffStat(diff.stat)

  return (
    <div className="flex items-center justify-between gap-4 rounded-lg border border-border p-3">
      <div className="flex min-w-0 flex-col gap-1">
        <span className="truncate font-mono text-sm text-foreground">{diff.branch}</span>
        {diff.error ? (
          <span className="truncate text-xs text-destructive" title={diff.error}>
            {diff.error}
          </span>
        ) : diff.base === null ? (
          <span className="text-xs text-muted-foreground">No base branch — pick one to compare against.</span>
        ) : (
          <span className="text-xs text-muted-foreground">
            {stats.filesChanged} file{stats.filesChanged === 1 ? '' : 's'} changed
            {stats.insertions > 0 && <span className="text-success"> +{stats.insertions}</span>}
            {stats.deletions > 0 && <span className="text-destructive"> -{stats.deletions}</span>}
          </span>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-2">
        <BaseBranchPicker hostBranches={hostBranches} defaultBase={defaultBase} value={override} onChange={onChangeBase} />
        <Button size="sm" variant="outline" disabled={diff.base === null} onClick={onShowDiffs}>
          Show Diffs
        </Button>
      </div>
    </div>
  )
}

export function BranchesPage({ sandboxId, navigate }: { sandboxId: string; navigate: (route: Route) => void }) {
  const sandbox = useAppStore((s) => s.sandboxes.find((sb) => sb.id === sandboxId))
  const project = useAppStore((s) => s.projects.find((p) => p.id === sandbox?.project_id))
  const bases = useCompareBases(sandboxId)
  const hostBranches = useHostBranches(sandboxId)
  const [diff, setDiff] = useState<SandboxDiffStats | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function load() {
    setLoading(true)
    setError(null)
    try {
      const result = await invoke<SandboxDiffStats>('get_sandbox_diff_stats', { id: sandboxId, bases })
      setDiff(result)
      dropStaleCompareBases(sandboxId, { sandboxBranches: result.branches.map((b) => b.branch) })
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    load()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sandboxId, bases])

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

      {!diff ? (
        loading && <p className="text-sm text-muted-foreground">Loading…</p>
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
              hostBranches={hostBranches}
              defaultBase={sandbox.base_branch}
              override={bases[branchDiff.branch] ?? null}
              onChangeBase={(base) => setCompareBase(sandboxId, branchDiff.branch, base)}
              onShowDiffs={() => navigate({ screen: 'sandboxes', sandboxId, branches: true, branch: branchDiff.branch })}
            />
          ))}
        </div>
      )}

      {sandbox.mode === 'clone' && (
        <div className="mt-4 flex flex-col gap-3 border-t border-dashed border-border pt-8">
          <div className="flex items-center justify-between gap-2">
            <h2 className="text-base font-semibold text-foreground">Git Sync Output</h2>
            <AutoGitSyncSelect sandbox={sandbox} />
          </div>
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
