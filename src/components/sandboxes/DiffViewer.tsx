import { useState } from 'react'
import { Check, Copy } from 'lucide-react'
import { Button } from '@/components/ui/button'

export interface DiffFilePatch {
  filePath: string
  patch: string
}

// Hand-rolled per-file split — no diff npm package, matching the codebase's
// existing preference for thin wrappers around real CLI output (see
// git.rs's diff/diff_stat). `git diff` concatenates every changed file's
// patch into one string, each starting with its own "diff --git a/X b/Y"
// line, so splitting on that line boundary recovers per-file patches.
export function splitPatchByFile(patch: string): DiffFilePatch[] {
  if (!patch) return []
  const lines = patch.split('\n')
  const files: DiffFilePatch[] = []
  let current: string[] | null = null

  function pushCurrent() {
    if (current) files.push({ filePath: extractFilePath(current[0]), patch: current.join('\n') })
  }

  for (const line of lines) {
    if (line.startsWith('diff --git ')) {
      pushCurrent()
      current = [line]
    } else if (current) {
      current.push(line)
    }
  }
  pushCurrent()
  return files
}

// "diff --git a/old b/new" — prefer the "b/" (new) path since that's what
// users recognize the file as; fall back to "a/" for a deleted file, where
// "b/" points at /dev/null instead of a real path.
function extractFilePath(diffGitLine: string): string {
  const match = diffGitLine.match(/^diff --git a\/(.+) b\/(.+)$/)
  if (!match) return diffGitLine
  const [, oldPath, newPath] = match
  return newPath !== '/dev/null' ? newPath : oldPath
}

export type DiffCellKind = 'context' | 'remove' | 'add' | 'empty'

export interface DiffCell {
  kind: DiffCellKind
  text: string
  lineNumber: number | null
}

export interface DiffRow {
  left: DiffCell
  right: DiffCell
}

export interface DiffHunk {
  header: string
  rows: DiffRow[]
}

const HUNK_HEADER_RE = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/

// Pairs a single file's unified-diff hunks into side-by-side rows: within a
// hunk, consecutive removed lines and consecutive added lines are zipped
// row-by-row (the same pairing a side-by-side view conventionally shows,
// treating a same-position remove+add as "this line changed" rather than
// "a line vanished, then an unrelated one appeared"), padding the shorter
// side with an empty cell. Context lines flush any pending pair first and
// then appear identically on both sides. File-header lines ("diff --git",
// "index", "---", "+++") are skipped — the accordion row above already
// shows the filename. Line numbers are seeded from the hunk header and
// incremented per side as each line is consumed, independent of the
// left/right zipping above.
export function parseFileDiff(patch: string): DiffHunk[] {
  const lines = patch.split('\n')
  if (lines[lines.length - 1] === '') lines.pop()

  const hunks: DiffHunk[] = []
  let current: DiffHunk | null = null
  let pendingRemoves: string[] = []
  let pendingAdds: string[] = []
  let oldLine = 1
  let newLine = 1

  function flush() {
    if (!current) return
    const max = Math.max(pendingRemoves.length, pendingAdds.length)
    for (let i = 0; i < max; i++) {
      const removed = pendingRemoves[i]
      const added = pendingAdds[i]
      current.rows.push({
        left: removed !== undefined ? { kind: 'remove', text: removed, lineNumber: oldLine++ } : { kind: 'empty', text: '', lineNumber: null },
        right: added !== undefined ? { kind: 'add', text: added, lineNumber: newLine++ } : { kind: 'empty', text: '', lineNumber: null },
      })
    }
    pendingRemoves = []
    pendingAdds = []
  }

  for (const line of lines) {
    if (line.startsWith('@@')) {
      flush()
      const match = line.match(HUNK_HEADER_RE)
      oldLine = match ? Number(match[1]) : 1
      newLine = match ? Number(match[2]) : 1
      current = { header: line, rows: [] }
      hunks.push(current)
      continue
    }
    if (!current) continue
    if (line.startsWith('\\')) continue // "\ No newline at end of file"
    if (line.startsWith('-') && !line.startsWith('---')) {
      pendingRemoves.push(line.slice(1))
      continue
    }
    if (line.startsWith('+') && !line.startsWith('+++')) {
      pendingAdds.push(line.slice(1))
      continue
    }
    flush()
    const text = line.startsWith(' ') ? line.slice(1) : line
    current.rows.push({
      left: { kind: 'context', text, lineNumber: oldLine++ },
      right: { kind: 'context', text, lineNumber: newLine++ },
    })
  }
  flush()
  return hunks
}

// Red/green shows as a tinted background plus a left-border accent — the
// code text itself stays a faded neutral color (matching the Plans tab's
// faded-text convention) rather than colored red/green, so the diff reads
// as "this line's background/edge says remove/add" instead of "this text
// is red/green".
const CELL_CLASS: Record<DiffCellKind, string> = {
  context: 'text-muted-foreground border-l-2 border-l-transparent',
  remove: 'bg-destructive/10 text-foreground/70 border-l-2 border-l-destructive',
  add: 'bg-success/10 text-foreground/70 border-l-2 border-l-success',
  empty: 'bg-muted/20 border-l-2 border-l-transparent',
}

const GUTTER_CLASS: Record<DiffCellKind, string> = {
  context: 'text-muted-foreground/50',
  remove: 'bg-destructive/10 text-muted-foreground/70',
  add: 'bg-success/10 text-muted-foreground/70',
  empty: 'bg-muted/20',
}

// Common single-line/block comment markers across the languages this app's
// diffs are likely to show — no per-language syntax awareness, just enough
// to fade obviously-comment lines a bit more than regular code.
const COMMENT_PREFIXES = ['//', '#', '--', '/*', '*', '<!--']

function isCommentLine(text: string): boolean {
  const trimmed = text.trimStart()
  return COMMENT_PREFIXES.some((prefix) => trimmed.startsWith(prefix))
}

function DiffViewerHeader({ filePath }: { filePath: string }) {
  const [copied, setCopied] = useState(false)

  async function copy() {
    await navigator.clipboard.writeText(filePath)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }

  return (
    <div className="flex items-center justify-between gap-2 border-b border-border bg-muted/40 px-2 py-1.5 font-mono text-xs">
      <span className="truncate text-foreground">{filePath}</span>
      <Button size="sm" variant="ghost" onClick={copy} type="button" title="Copy file path">
        {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
      </Button>
    </div>
  )
}

// Pure component: renders one file's patch as a side-by-side (old | new)
// table. Takes no fetch concerns of its own — callers own loading.
export function DiffViewer({ patch, filePath }: { patch: string; filePath: string }) {
  const hunks = parseFileDiff(patch)

  if (hunks.length === 0) {
    return <p className="text-sm text-muted-foreground">No changes.</p>
  }

  return (
    <div className="my-4 rounded-md border border-border bg-primary/5">
      <DiffViewerHeader filePath={filePath} />
      {hunks.map((hunk, i) => (
        <div key={i}>
          <div className="bg-muted/40 px-2 py-1 font-mono text-xs text-muted-foreground">{hunk.header}</div>
          <table className="w-full table-fixed border-collapse text-xs leading-relaxed [tab-size:2]">
            <tbody>
              {hunk.rows.map((row, j) => (
                <tr key={j}>
                  <td className={`w-10 select-none px-2 text-right align-top font-mono tabular-nums ${GUTTER_CLASS[row.left.kind]}`}>
                    {row.left.lineNumber ?? ''}
                  </td>
                  <td className={`w-[calc(50%-2.5rem)] px-2 align-top font-mono ${CELL_CLASS[row.left.kind]}`}>
                    <div className={`whitespace-pre-wrap break-all ${isCommentLine(row.left.text) ? 'opacity-60' : ''}`}>
                      {row.left.text.length > 0 ? row.left.text : ' '}
                    </div>
                  </td>
                  <td
                    className={`w-10 select-none border-l border-border px-2 text-right align-top font-mono tabular-nums ${GUTTER_CLASS[row.right.kind]}`}
                  >
                    {row.right.lineNumber ?? ''}
                  </td>
                  <td className={`w-[calc(50%-2.5rem)] px-2 align-top font-mono ${CELL_CLASS[row.right.kind]}`}>
                    <div className={`whitespace-pre-wrap break-all ${isCommentLine(row.right.text) ? 'opacity-60' : ''}`}>
                      {row.right.text.length > 0 ? row.right.text : ' '}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ))}
    </div>
  )
}
