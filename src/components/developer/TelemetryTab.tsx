import { useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { DateTimeRangeFilter } from './DateTimeRangeFilter'
import type { DaemonLogResult } from './types'

type Level = 'error' | 'warn' | 'info'
type LevelFilter = 'all' | Level

function detectLevel(line: string): Level | null {
  if (/\berror\b/i.test(line)) return 'error'
  if (/\bwarn(ing)?\b/i.test(line)) return 'warn'
  if (/\binfo\b/i.test(line)) return 'info'
  return null
}

// Best-effort — daemon log formats vary. Recognizes this app's own
// "[YYYY-MM-DD][HH:MM:SS]" format plus a plain "YYYY-MM-DD HH:MM:SS"/ISO
// timestamp appearing anywhere in the line.
function extractTimestamp(line: string): number | null {
  const bracketed = line.match(/^\[(\d{4}-\d{2}-\d{2})\]\[(\d{2}:\d{2}:\d{2})\]/)
  if (bracketed) {
    const t = Date.parse(`${bracketed[1]}T${bracketed[2]}`)
    return Number.isNaN(t) ? null : t
  }
  const plain = line.match(/(\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2})/)
  if (plain) {
    const t = Date.parse(plain[1].replace(' ', 'T'))
    return Number.isNaN(t) ? null : t
  }
  return null
}

const LEVEL_CLASS: Record<Level, string> = {
  error: 'text-destructive',
  warn: 'text-warning',
  info: 'text-gray-400',
}

export function TelemetryTab() {
  const [path, setPath] = useState('')
  const [result, setResult] = useState<DaemonLogResult | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [levelFilter, setLevelFilter] = useState<LevelFilter>('all')
  const [fromTime, setFromTime] = useState('')
  const [toTime, setToTime] = useState('')

  async function refresh(pathToRead: string) {
    setLoading(true)
    setError(null)
    try {
      setResult(await invoke<DaemonLogResult>('read_daemon_log', { path: pathToRead }))
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    invoke<string>('get_daemon_log_path').then((saved) => {
      setPath(saved)
      refresh(saved)
    })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  async function handleBrowse() {
    const selection = await open({ directory: false, multiple: false })
    if (typeof selection === 'string') setPath(selection)
  }

  async function handleSaveAndRefresh() {
    await invoke('save_daemon_log_path', { path })
    await refresh(path)
  }

  function handleDateRangeChange(from: string, to: string) {
    setFromTime(from)
    setToTime(to)
  }

  function handleResetFilters() {
    setLevelFilter('all')
    setFromTime('')
    setToTime('')
  }

  const lines = useMemo(() => (result?.content ? result.content.split('\n') : []), [result])
  const fromTs = fromTime ? new Date(fromTime).getTime() : null
  const toTs = toTime ? new Date(toTime).getTime() : null

  const filteredLines = useMemo(() => {
    return lines.filter((line) => {
      if (levelFilter !== 'all' && detectLevel(line) !== levelFilter) return false
      if (fromTs !== null || toTs !== null) {
        const ts = extractTimestamp(line)
        if (ts !== null) {
          if (fromTs !== null && ts < fromTs) return false
          if (toTs !== null && ts > toTs) return false
        }
      }
      return true
    })
  }, [lines, levelFilter, fromTs, toTs])

  const filtersActive = levelFilter !== 'all' || fromTime !== '' || toTime !== ''

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 text-sm">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-medium">Daemon Logs</h2>
        {loading && <span className="text-xs text-muted-foreground">Loading…</span>}
      </div>

      <p className="text-xs text-muted-foreground">
        Tails the last portion of the sbx daemon's log file. There's no single fixed path across
        installs, so point this at the right file (or a directory of log files — the newest one
        is used) for your machine.
      </p>

      <div className="flex flex-wrap gap-2">
        <Input
          value={path}
          onChange={(e) => setPath(e.target.value)}
          placeholder="Path to daemon log file or directory"
          className="min-w-48 flex-1"
        />
        <Button variant="outline" onClick={handleBrowse}>
          Browse
        </Button>
        <Button onClick={handleSaveAndRefresh}>Save &amp; Refresh</Button>
        <Button variant="outline" onClick={() => refresh(path)}>
          Refresh
        </Button>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <Select value={levelFilter} onValueChange={(v) => setLevelFilter(v as LevelFilter)}>
          <SelectTrigger className="w-32">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All levels</SelectItem>
            <SelectItem value="error">Error</SelectItem>
            <SelectItem value="warn">Warn</SelectItem>
            <SelectItem value="info">Info</SelectItem>
          </SelectContent>
        </Select>
        <DateTimeRangeFilter from={fromTime} to={toTime} onChange={handleDateRangeChange} />
        <Button variant="ghost" size="lg" onClick={handleResetFilters} disabled={!filtersActive}>
          Reset all filters
        </Button>
      </div>

      {error && <p className="text-sm text-destructive">{error}</p>}

      {result && !result.exists && <p className="text-sm text-muted-foreground">No log file found at this path.</p>}

      {result?.exists && (
        <div className="flex min-h-0 flex-1 flex-col gap-2">
          {result.truncated && (
            <p className="text-xs text-muted-foreground">Showing the last portion of a large file.</p>
          )}
          {result.resolved_path && result.resolved_path !== path && (
            <p className="text-xs text-muted-foreground">Reading: {result.resolved_path}</p>
          )}
          <div className="min-h-0 flex-1 overflow-auto rounded-lg border border-border bg-primary/5 p-3 font-mono text-xs">
            {filteredLines.length === 0 ? (
              <span className="text-muted-foreground">(no matching lines)</span>
            ) : (
              <div className="flex flex-col gap-6">
                {filteredLines.map((line, i) => {
                  const level = detectLevel(line)
                  return (
                    <div key={i} className={`whitespace-pre-wrap break-words ${level ? LEVEL_CLASS[level] : ''}`}>
                      {line || ' '}
                    </div>
                  )
                })}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
