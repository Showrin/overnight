export type DiffLineKind = 'meta' | 'hunk' | 'add' | 'remove' | 'context'

export interface DiffLine {
  kind: DiffLineKind
  text: string
}

// Hand-rolled unified-diff line classifier — no diff npm package, matching
// the codebase's existing preference for thin wrappers around real CLI
// output (see git.rs's diff/diff_stat). Classifies each line by its first
// character(s):
//   - "diff --git", "index ", "--- ", "+++ "  -> meta (file/header lines)
//   - "@@"                                    -> hunk (hunk header)
//   - "+" (not "+++")                         -> add
//   - "-" (not "---")                         -> remove
//   - anything else                           -> context
// The "---"/"+++ " file-header special-case matters: without it, those two
// lines would be misclassified as a removed/added content line just
// because they start with the same character.
export function parseUnifiedDiff(patch: string): DiffLine[] {
  if (!patch) return []
  return patch.split('\n').map((text) => ({ kind: classifyLine(text), text }))
}

function classifyLine(text: string): DiffLineKind {
  if (
    text.startsWith('diff --git') ||
    text.startsWith('index ') ||
    text.startsWith('--- ') ||
    text.startsWith('+++ ')
  ) {
    return 'meta'
  }
  if (text.startsWith('@@')) return 'hunk'
  if (text.startsWith('+')) return 'add'
  if (text.startsWith('-')) return 'remove'
  return 'context'
}

// Reuses the app's existing semantic success/destructive theme tokens
// (see badge.tsx) rather than raw Tailwind color utilities, so the diff
// stays correctly themed in both light and dark mode without inventing a
// second color system.
const LINE_CLASS: Record<DiffLineKind, string> = {
  meta: 'text-muted-foreground',
  hunk: 'font-semibold text-foreground',
  add: 'text-success',
  remove: 'text-destructive',
  context: 'text-muted-foreground',
}

// Pure component: renders a unified-diff patch string as colored lines.
// Takes no sandbox/fetch concerns of its own — SandboxDiffTab owns loading
// and passes the patch string straight through.
export function DiffViewer({ patch }: { patch: string }) {
  const lines = parseUnifiedDiff(patch)

  if (lines.length === 0) {
    return <p className="text-sm text-muted-foreground">No changes.</p>
  }

  return (
    <pre className="overflow-x-auto rounded-md border border-border bg-muted/30 p-3 text-xs leading-relaxed">
      {lines.map((line, i) => (
        <div key={i} className={LINE_CLASS[line.kind]}>
          {line.text.length > 0 ? line.text : ' '}
        </div>
      ))}
    </pre>
  )
}
