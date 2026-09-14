import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Check, Copy } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { DateTimeRangeFilter } from './DateTimeRangeFilter'
import type { CommandLogEntry } from './types'

function CommandBlock({ command }: { command: string }) {
  const [copied, setCopied] = useState(false)

  async function handleCopy() {
    await navigator.clipboard.writeText(command)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }

  return (
    <div className="relative">
      <pre className="overflow-x-auto rounded-lg border border-border bg-muted p-4 pr-10 font-mono text-xs whitespace-pre-wrap break-all text-muted-foreground">
        {command}
      </pre>
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        className="absolute top-2 right-2"
        onClick={handleCopy}
      >
        {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
      </Button>
    </div>
  )
}

const PAGE_SIZE_OPTIONS = [25, 50, 100]
const PAGE_SIBLINGS = 2
const PAGE_BOUNDARIES = 2

type Filter = 'all' | 'success' | 'failed'

function successOnlyFor(filter: Filter): boolean | undefined {
  if (filter === 'success') return true
  if (filter === 'failed') return false
  return undefined
}

const ACTION_LABEL_MAX_LENGTH = 28

// CSS ellipsis truncation doesn't reliably flow through Radix's item-text
// tracking (the trigger mirrors whatever text an item rendered), so the
// label is shortened here instead — this way both the dropdown list and
// the trigger (which echoes the selected item's text) stay bounded.
function truncateActionLabel(label: string): string {
  return label.length > ACTION_LABEL_MAX_LENGTH ? `${label.slice(0, ACTION_LABEL_MAX_LENGTH - 1)}…` : label
}

// Builds "1 2 3 … 11 12"-style page numbers around the current page, with
// `PAGE_BOUNDARIES` pages pinned at each end and `PAGE_SIBLINGS` around the
// current page — collapsing any gap larger than one page into an ellipsis.
function pageNumbers(current: number, total: number): (number | 'ellipsis')[] {
  if (total <= 0) return []
  const keep = new Set<number>()
  for (let p = 1; p <= Math.min(PAGE_BOUNDARIES, total); p++) keep.add(p)
  for (let p = Math.max(1, current - PAGE_SIBLINGS); p <= Math.min(total, current + PAGE_SIBLINGS); p++) keep.add(p)
  for (let p = Math.max(1, total - PAGE_BOUNDARIES + 1); p <= total; p++) keep.add(p)

  const sorted = Array.from(keep).sort((a, b) => a - b)
  const result: (number | 'ellipsis')[] = []
  let previous = 0
  for (const p of sorted) {
    if (previous && p - previous > 1) result.push('ellipsis')
    result.push(p)
    previous = p
  }
  return result
}

export function CommandLogTab() {
  const [entries, setEntries] = useState<CommandLogEntry[]>([])
  const [operations, setOperations] = useState<string[]>([])
  const [filter, setFilter] = useState<Filter>('all')
  const [operationFilter, setOperationFilter] = useState<string>('all')
  const [fromTime, setFromTime] = useState('')
  const [toTime, setToTime] = useState('')
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(PAGE_SIZE_OPTIONS[0])
  const [totalCount, setTotalCount] = useState(0)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  async function load() {
    setLoading(true)
    setError(null)
    try {
      const params = {
        successOnly: successOnlyFor(filter),
        sinceMs: fromTime ? new Date(fromTime).getTime() : undefined,
        untilMs: toTime ? new Date(toTime).getTime() : undefined,
        operation: operationFilter === 'all' ? undefined : operationFilter,
      }
      const [results, count, ops] = await Promise.all([
        invoke<CommandLogEntry[]>('list_command_log', { limit: pageSize, offset: (page - 1) * pageSize, ...params }),
        invoke<number>('count_command_log', params),
        invoke<string[]>('list_command_log_operations'),
      ])
      setEntries(results)
      setTotalCount(count)
      setOperations(ops)
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    load()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filter, operationFilter, fromTime, toTime, page, pageSize])

  function handleFilterChange(next: Filter) {
    setFilter(next)
    setPage(1)
  }

  function handleOperationChange(next: string) {
    setOperationFilter(next)
    setPage(1)
  }

  function handleDateRangeChange(from: string, to: string) {
    setFromTime(from)
    setToTime(to)
    setPage(1)
  }

  function handlePageSizeChange(value: number) {
    setPageSize(value)
    setPage(1)
  }

  function handleResetFilters() {
    setFilter('all')
    setOperationFilter('all')
    setFromTime('')
    setToTime('')
    setPage(1)
  }

  const filtersActive = filter !== 'all' || operationFilter !== 'all' || fromTime !== '' || toTime !== ''
  const totalPages = Math.max(1, Math.ceil(totalCount / pageSize))

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 text-sm">
      <div className="flex shrink-0 flex-col gap-3 border-b border-border bg-background pb-4">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-medium">Command Logs</h2>
          {loading && <span className="text-xs text-muted-foreground">Loading…</span>}
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <Select value={filter} onValueChange={(v) => handleFilterChange(v as Filter)}>
            <SelectTrigger className="w-32">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All</SelectItem>
              <SelectItem value="success">Success</SelectItem>
              <SelectItem value="failed">Failed</SelectItem>
            </SelectContent>
          </Select>
          <Select value={operationFilter} onValueChange={handleOperationChange}>
            <SelectTrigger className="w-48 overflow-hidden">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All actions</SelectItem>
              {operations.map((op) => (
                <SelectItem key={op} value={op} title={op} className="truncate">
                  {truncateActionLabel(op)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <DateTimeRangeFilter from={fromTime} to={toTime} onChange={handleDateRangeChange} />
          <Button variant="outline" size="lg" onClick={load}>
            Refresh
          </Button>
          <Button variant="ghost" size="lg" onClick={handleResetFilters} disabled={!filtersActive}>
            Reset all filters
          </Button>
        </div>
      </div>

      {error && <p className="text-sm text-destructive">{error}</p>}

      <div className="min-h-0 flex-1 overflow-auto">
        {entries.length === 0 && !loading ? (
          <p className="text-sm text-muted-foreground">No commands logged yet.</p>
        ) : (
          <div className="flex flex-col gap-4">
            {entries.map((entry) => {
              const args: string[] = JSON.parse(entry.args)
              return (
                <div key={entry.id} className="flex flex-col gap-4 rounded-lg border border-border bg-primary/5 p-3">
                  <div className="flex items-center justify-between gap-2">
                    <span className="font-medium">{entry.operation}</span>
                    <div className="flex items-center gap-2">
                      <Badge variant={entry.success ? 'success' : 'destructive'}>
                        {entry.success ? 'success' : 'failed'}
                      </Badge>
                      <span className="text-xs text-muted-foreground">
                        {new Date(entry.occurred_at).toLocaleString()}
                      </span>
                    </div>
                  </div>
                  <CommandBlock command={`${entry.program} ${args.join(' ')}`} />
                  {entry.stderr && <p className="text-xs text-destructive">{entry.stderr}</p>}
                </div>
              )
            })}
          </div>
        )}
      </div>

      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-t border-border pt-3">
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <span>Rows per page</span>
          <Select value={String(pageSize)} onValueChange={(v) => handlePageSizeChange(Number(v))}>
            <SelectTrigger className="w-20">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {PAGE_SIZE_OPTIONS.map((size) => (
                <SelectItem key={size} value={String(size)}>
                  {size}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <span>· {totalCount} total</span>
        </div>
        <div className="flex items-center gap-1">
          <Button variant="outline" size="sm" onClick={() => setPage((p) => Math.max(1, p - 1))} disabled={page === 1}>
            Prev
          </Button>
          {pageNumbers(page, totalPages).map((p, i) =>
            p === 'ellipsis' ? (
              <span key={`ellipsis-${i}`} className="px-1 text-xs text-muted-foreground">
                …
              </span>
            ) : (
              <Button key={p} variant={p === page ? 'default' : 'ghost'} size="icon-sm" onClick={() => setPage(p)}>
                {p}
              </Button>
            ),
          )}
          <Button
            variant="outline"
            size="sm"
            onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
            disabled={page === totalPages}
          >
            Next
          </Button>
        </div>
      </div>
    </div>
  )
}
