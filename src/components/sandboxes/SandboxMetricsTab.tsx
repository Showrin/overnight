import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Sparkline } from '@/components/charts/Sparkline'
import type { ContainerMetric, Sandbox } from './types'

const POLL_MS = 5000
const HISTORY_SEED_MS = 24 * 60 * 60 * 1000
const HISTORY_MAX_POINTS = 150

function formatKbPerSec(kbPerSec: number): string {
  return kbPerSec >= 1024 ? `${(kbPerSec / 1024).toFixed(1)}MB/s` : `${kbPerSec.toFixed(0)}KB/s`
}

// Copied from HostStatsPanel's StatChart rather than shared — this tab's
// data comes from a different table/shape (ContainerMetric, not
// HostMetric) and lives in a plain tab-content layout rather than a
// standalone Card panel, so the two are similar but not identical.
function StatChart({
  label,
  current,
  caption,
  data,
  max,
  value,
}: {
  label: string
  current: string | null
  caption?: string
  data: number[]
  max?: number
  value?: number
}) {
  return (
    <div className="flex min-w-36 flex-1 flex-col gap-1">
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-xs font-normal text-muted-foreground">{label}</span>
        <span className="text-sm font-medium">{current ?? '—'}</span>
      </div>
      <Sparkline data={data} max={max} value={value} />
      {caption && <span className="text-xs text-muted-foreground">{caption}</span>}
    </div>
  )
}

// Live CPU/memory/network for the SELECTED sandbox, read via `sbx exec`
// /proc reads inside its VM — host `docker stats` can't see into a
// sandbox's isolated microVM at all (see sbx::sample_resource_usage).
//
// Polling is scoped to this component's lifetime, not app-wide: the
// interval starts on mount and is cleared on unmount, so navigating away
// from the Metrics tab (or the detail page entirely) stops the extra
// `sbx exec` round trip for this sandbox instead of running one for every
// sandbox on every tick regardless of which page is open.
export function SandboxMetricsTab({ sandbox }: { sandbox: Sandbox }) {
  const [points, setPoints] = useState<ContainerMetric[]>([])
  const [error, setError] = useState<string | null>(null)
  const isRunning = sandbox.status === 'running'

  useEffect(() => {
    let cancelled = false
    invoke<ContainerMetric[]>('get_sandbox_resource_history', {
      sandboxId: sandbox.id,
      sinceMs: Date.now() - HISTORY_SEED_MS,
    })
      .then((history) => {
        if (!cancelled) setPoints(history.slice(-HISTORY_MAX_POINTS))
      })
      .catch((e) => {
        if (!cancelled) setError(String(e))
      })
    return () => {
      cancelled = true
    }
  }, [sandbox.id])

  useEffect(() => {
    // The backend only samples a running sandbox (there's no /proc to
    // read once it's stopped) — skip polling entirely rather than
    // hitting it every 5s just to get a rejected promise back.
    if (!isRunning) return

    let cancelled = false

    async function poll() {
      try {
        const metric = await invoke<ContainerMetric>('get_sandbox_resource_usage', { id: sandbox.id })
        if (!cancelled) {
          setError(null)
          setPoints((prev) => [...prev, metric].slice(-HISTORY_MAX_POINTS))
        }
      } catch (e) {
        if (!cancelled) setError(String(e))
      }
    }

    poll()
    const interval = setInterval(poll, POLL_MS)
    return () => {
      cancelled = true
      clearInterval(interval)
    }
  }, [sandbox.id, isRunning])

  const current = points.length > 0 ? points[points.length - 1] : null

  return (
    <div className="flex flex-col gap-4 text-sm">
      {error && <p className="text-sm text-destructive">{error}</p>}
      {!isRunning && (
        <p className="text-sm text-muted-foreground">
          Metrics are only sampled while the sandbox is running — showing the last known history.
        </p>
      )}
      <div className="flex flex-wrap justify-between gap-4">
        <StatChart
          label="CPU"
          current={current ? `${current.cpu_percent.toFixed(0)}%` : null}
          data={points.map((m) => m.cpu_percent)}
          max={100}
          value={current?.cpu_percent}
        />
        <StatChart
          label="Memory"
          current={current ? `${(current.memory_mb / 1024).toFixed(2)} GB` : null}
          data={points.map((m) => m.memory_mb)}
        />
        <StatChart
          label="Network"
          current={
            current
              ? `↓${formatKbPerSec(current.network_rx_bytes)} ↑${formatKbPerSec(current.network_tx_bytes)}`
              : null
          }
          data={points.map((m) => m.network_rx_bytes + m.network_tx_bytes)}
        />
      </div>
    </div>
  )
}
