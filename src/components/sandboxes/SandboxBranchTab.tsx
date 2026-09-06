import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { formatRelativeTime } from '@/lib/sandboxDisplay'
import type { Sandbox } from './types'

const BASE_BRANCH_UNKNOWN_COPY = 'Unknown — created before branch tracking was added'
const NO_SNAPSHOT_COPY = 'No branch snapshot yet — start the sandbox to capture one.'

// Shows base_branch (captured once at creation) alongside the sandbox's
// current branch/branch-list/worktrees (one persisted "branch snapshot",
// refreshed via get_sandbox_branch_info whenever this tab mounts while the
// sandbox is running). A stopped sandbox has no live state to read, so it
// just shows its last-known snapshot with an "as of <relative time>" label
// instead of the "live" one.
export function SandboxBranchTab({ sandbox }: { sandbox: Sandbox }) {
  const [info, setInfo] = useState<Sandbox>(sandbox)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    setInfo(sandbox)
  }, [sandbox])

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    setError(null)
    invoke<Sandbox>('get_sandbox_branch_info', { id: sandbox.id })
      .then((updated) => {
        if (!cancelled) setInfo(updated)
      })
      .catch((e) => {
        if (!cancelled) setError(String(e))
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
    // Re-fetch when the sandbox transitions running <-> stopped, not on
    // every unrelated field change.
  }, [sandbox.id, sandbox.status])

  const isLive = sandbox.status === 'running'
  const freshnessLabel = isLive
    ? 'live'
    : info.branch_snapshot_at != null
      ? `as of ${formatRelativeTime(info.branch_snapshot_at)}`
      : null

  return (
    <div className="flex flex-col gap-6 text-sm">
      {error && <p className="text-sm text-destructive">{error}</p>}

      <div className="grid grid-cols-2 gap-4 md:grid-cols-4">
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-xs text-muted-foreground/70">Base Branch</span>
          {info.base_branch ? (
            <span className="truncate font-mono text-sm text-foreground">{info.base_branch}</span>
          ) : (
            <span className="text-sm text-muted-foreground">{BASE_BRANCH_UNKNOWN_COPY}</span>
          )}
        </div>
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-xs text-muted-foreground/70">
            Current Branch{freshnessLabel ? ` (${freshnessLabel})` : ''}
          </span>
          {freshnessLabel === null ? (
            <span className="text-sm text-muted-foreground">{NO_SNAPSHOT_COPY}</span>
          ) : info.current_branch ? (
            <span className="truncate font-mono text-sm text-foreground">{info.current_branch}</span>
          ) : (
            <span className="text-sm text-muted-foreground">{loading ? 'Loading…' : 'Detached HEAD'}</span>
          )}
        </div>
      </div>

      <div className="flex flex-col gap-1.5">
        <span className="text-xs text-muted-foreground/70">
          Branches{freshnessLabel ? ` (${freshnessLabel})` : ''}
        </span>
        {freshnessLabel === null ? (
          <span className="text-sm text-muted-foreground">{NO_SNAPSHOT_COPY}</span>
        ) : info.branches.length > 0 ? (
          <ul className="flex flex-col gap-0.5">
            {info.branches.map((branch) => (
              <li key={branch} className="font-mono text-sm text-foreground">
                {branch === info.current_branch ? `* ${branch}` : `  ${branch}`}
              </li>
            ))}
          </ul>
        ) : (
          <span className="text-sm text-muted-foreground">{loading ? 'Loading…' : 'No branches found'}</span>
        )}
      </div>

      <div className="flex flex-col gap-1.5">
        <span className="text-xs text-muted-foreground/70">
          Worktrees{freshnessLabel ? ` (${freshnessLabel})` : ''}
        </span>
        {freshnessLabel === null ? (
          <span className="text-sm text-muted-foreground">{NO_SNAPSHOT_COPY}</span>
        ) : info.worktrees.length > 0 ? (
          <ul className="flex flex-col gap-2">
            {info.worktrees.map((worktree) => (
              <li key={worktree.path} className="flex flex-col">
                <span className="truncate font-mono text-sm text-foreground">{worktree.path}</span>
                <span className="text-xs text-muted-foreground">
                  {worktree.branch ?? 'detached'} @ {worktree.head_sha ? worktree.head_sha.slice(0, 7) : 'unknown'}
                </span>
              </li>
            ))}
          </ul>
        ) : (
          <span className="text-sm text-muted-foreground">{loading ? 'Loading…' : 'No worktrees found'}</span>
        )}
      </div>
    </div>
  )
}
