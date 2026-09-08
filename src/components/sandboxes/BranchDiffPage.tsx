import { useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { GitCommitHorizontal } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { buildFileTree } from '@/lib/buildFileTree'
import type { Route } from '@/lib/router'
import { parseDiffStat } from '@/lib/sandboxDisplay'
import { useAppStore } from '@/store/useAppStore'
import { BranchCommitsDialog } from './BranchCommitsDialog'
import { DiffViewer, splitPatchByFile } from './DiffViewer'
import { FileTreePanel } from './FileTreePanel'
import { SandboxBreadcrumb } from './SandboxBreadcrumb'
import type { CommitInfo, SandboxDiff } from './types'

export function BranchDiffPage({
  sandboxId,
  branch,
  navigate,
}: {
  sandboxId: string
  branch: string
  navigate: (route: Route) => void
}) {
  const sandbox = useAppStore((s) => s.sandboxes.find((sb) => sb.id === sandboxId))
  const project = useAppStore((s) => s.projects.find((p) => p.id === sandbox?.project_id))
  const [diff, setDiff] = useState<SandboxDiff | null>(null)
  const [commits, setCommits] = useState<CommitInfo[] | null>(null)
  const [commitsOpen, setCommitsOpen] = useState(false)
  const [clickedFile, setClickedFile] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    invoke<SandboxDiff>('get_sandbox_diff', { id: sandboxId }).then(setDiff).catch((e) => setError(String(e)))
    invoke<CommitInfo[]>('get_branch_commits', { id: sandboxId, branch })
      .then(setCommits)
      .catch((e) => setError(String(e)))
  }, [sandboxId, branch])

  const branchDiff = diff?.branches.find((b) => b.branch === branch)
  const files = useMemo(() => (branchDiff ? splitPatchByFile(branchDiff.patch) : []), [branchDiff])
  const tree = useMemo(() => buildFileTree(files.map((f) => f.filePath)), [files])
  const selectedFile = clickedFile ?? files[0]?.filePath ?? null

  if (!sandbox) {
    return <p className="text-sm text-muted-foreground">This sandbox couldn't be found — it may have been deleted.</p>
  }

  const title = sandbox.name ?? project?.name ?? 'Sandbox'
  const stats = branchDiff ? parseDiffStat(branchDiff.stat) : null
  const selectedPatch = files.find((f) => f.filePath === selectedFile)?.patch

  return (
    <div className="flex flex-col gap-6">
      <SandboxBreadcrumb
        segments={[
          { label: 'Sandboxes', onClick: () => navigate({ screen: 'sandboxes' }) },
          { label: title, onClick: () => navigate({ screen: 'sandboxes', sandboxId }) },
          { label: 'Branches', onClick: () => navigate({ screen: 'sandboxes', sandboxId, branches: true }) },
          { label: branch },
        ]}
      />

      {error && <p className="text-sm text-destructive">{error}</p>}

      <div className="flex flex-wrap items-center gap-x-8 gap-y-2 border-b border-border pb-4 text-sm">
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-xs text-muted-foreground/70">Branch</span>
          <span className="truncate font-mono text-foreground">{branch}</span>
        </div>
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-xs text-muted-foreground/70">Base Branch</span>
          <span className="truncate font-mono text-foreground">{diff?.base_branch ?? '—'}</span>
        </div>
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-xs text-muted-foreground/70">Changes</span>
          <span className="text-foreground">
            {stats ? (
              <>
                {stats.filesChanged} file{stats.filesChanged === 1 ? '' : 's'}
                {stats.insertions > 0 && <span className="text-success"> +{stats.insertions}</span>}
                {stats.deletions > 0 && <span className="text-destructive"> -{stats.deletions}</span>}
              </>
            ) : (
              '—'
            )}
          </span>
        </div>
        <Button size="sm" variant="outline" onClick={() => setCommitsOpen(true)} disabled={!commits}>
          <GitCommitHorizontal className="size-3.5" />
          Commits {commits ? `(${commits.length})` : ''}
        </Button>
      </div>

      {!diff ? (
        !error && <p className="text-sm text-muted-foreground">Loading…</p>
      ) : files.length === 0 ? (
        <p className="text-sm text-muted-foreground">No changes.</p>
      ) : (
        <div className="flex flex-1 gap-4 overflow-hidden">
          <FileTreePanel tree={tree} selectedFile={selectedFile} onSelectFile={setClickedFile} />
          <div className="min-w-0 flex-1 overflow-auto">{selectedPatch && <DiffViewer patch={selectedPatch} />}</div>
        </div>
      )}

      {commits && <BranchCommitsDialog commits={commits} open={commitsOpen} onOpenChange={setCommitsOpen} />}
    </div>
  )
}
