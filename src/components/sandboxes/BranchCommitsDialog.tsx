import { useState } from 'react'
import { Check, Copy } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { formatRelativeTime } from '@/lib/sandboxDisplay'
import type { CommitInfo } from './types'

function CommitRow({ commit }: { commit: CommitInfo }) {
  const [copied, setCopied] = useState(false)

  async function copyHash() {
    await navigator.clipboard.writeText(commit.hash)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }

  return (
    <div className="flex flex-col gap-1 border-b border-border py-3 last:border-b-0">
      <div className="flex items-center justify-between gap-2">
        <span className="truncate text-sm text-foreground">{commit.message}</span>
        <Button size="sm" variant="outline" onClick={copyHash} type="button">
          {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
          <span className="font-mono">{commit.hash.slice(0, 7)}</span>
        </Button>
      </div>
      <span className="text-xs text-muted-foreground">
        {commit.author} · {formatRelativeTime(commit.authored_at)}
      </span>
    </div>
  )
}

export function BranchCommitsDialog({
  commits,
  open,
  onOpenChange,
}: {
  commits: CommitInfo[]
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent title="Commits" className="max-w-2xl">
        <Card className="w-full">
          <CardHeader>
            <CardTitle>Commits ({commits.length})</CardTitle>
          </CardHeader>
          <CardContent className="max-h-[60vh] overflow-auto">
            {commits.length === 0 ? (
              <p className="py-3 text-sm text-muted-foreground">No commits.</p>
            ) : (
              commits.map((commit) => <CommitRow key={commit.hash} commit={commit} />)
            )}
          </CardContent>
        </Card>
      </DialogContent>
    </Dialog>
  )
}
