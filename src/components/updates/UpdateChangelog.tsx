import { MarkdownDocument } from '@/components/sandboxes/MarkdownDocument'
import type { ChangelogEntry } from '@/store/useUpdater'

function formatDate(date: string | null): string | null {
  if (!date) return null
  const parsed = new Date(date)
  return Number.isNaN(parsed.getTime()) ? null : parsed.toLocaleDateString(undefined, { dateStyle: 'medium' })
}

/** Release notes for every version between the installed one and the update, newest first. */
export function UpdateChangelog({ entries }: { entries: ChangelogEntry[] }) {
  return (
    <div className="flex flex-col gap-6">
      {entries.map((entry) => {
        const date = formatDate(entry.date)
        return (
          <section key={entry.version} className="flex flex-col gap-3">
            <div className="flex items-baseline gap-2 border-b border-border pb-2">
              <h2 className="text-base font-semibold text-foreground">v{entry.version}</h2>
              {date && <span className="text-xs text-muted-foreground">{date}</span>}
            </div>
            {entry.notes.trim() ? (
              <MarkdownDocument content={entry.notes} />
            ) : (
              <p className="text-sm text-muted-foreground">No release notes.</p>
            )}
          </section>
        )
      })}
    </div>
  )
}
