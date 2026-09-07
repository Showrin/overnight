import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Sparkline } from '@/components/charts/Sparkline'
import { useAppStore } from '@/store/useAppStore'
import type { ContainerMetric, HostMetric } from './types'

const WINDOW_MS = 90 * 24 * 60 * 60 * 1000

function formatKbPerSec(kbPerSec: number): string {
  return kbPerSec >= 1024 ? `${(kbPerSec / 1024).toFixed(1)}MB/s` : `${kbPerSec.toFixed(0)}KB/s`
}

function StatChart({
  label,
  current,
  data,
  max,
  value,
}: {
  label: string
  current: string | null
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
    </div>
  )
}

// 3 months of host + per-sandbox history — distinct from the live Metrics
// tab, which only holds the last ~12 minutes at a 5s poll. One-shot fetch,
// no polling: this is a look-back view, not a live one.
export function PerformanceMonitorTab() {
  const sandboxes = useAppStore((s) => s.sandboxes)
  const sandboxIds = sandboxes.map((sb) => sb.id).join(',')
  const [hostHistory, setHostHistory] = useState<HostMetric[]>([])
  const [sandboxHistory, setSandboxHistory] = useState<Record<string, ContainerMetric[]>>({})
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    setError(null)
    const sinceMs = Date.now() - WINDOW_MS

    Promise.all([
      invoke<HostMetric[]>('get_host_stats_history', { sinceMs }),
      Promise.all(
        sandboxes.map((sb) =>
          invoke<ContainerMetric[]>('get_sandbox_resource_history', { sandboxId: sb.id, sinceMs }).then(
            (history) => [sb.id, history] as const,
          ),
        ),
      ),
    ])
      .then(([host, perSandbox]) => {
        if (cancelled) return
        setHostHistory(host)
        setSandboxHistory(Object.fromEntries(perSandbox))
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sandboxIds])

  const lastHost = hostHistory[hostHistory.length - 1]

  return (
    <div className="flex flex-col gap-6 text-sm">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-medium">Host — last 90 days</h2>
        {loading && <span className="text-xs text-muted-foreground">Loading…</span>}
      </div>

      {error && <p className="text-sm text-destructive">{error}</p>}

      <div className="grid grid-cols-2 gap-4">
        <StatChart
          label="CPU"
          current={lastHost ? `${lastHost.cpu_percent.toFixed(0)}%` : null}
          data={hostHistory.map((m) => m.cpu_percent)}
          max={100}
          value={lastHost?.cpu_percent}
        />
        <StatChart
          label="Memory"
          current={lastHost ? `${lastHost.memory_percent.toFixed(0)}%` : null}
          data={hostHistory.map((m) => m.memory_percent)}
          max={100}
          value={lastHost?.memory_percent}
        />
        <StatChart
          label="Disk"
          current={lastHost ? `${lastHost.disk_percent.toFixed(0)}%` : null}
          data={hostHistory.map((m) => m.disk_percent)}
          max={100}
          value={lastHost?.disk_percent}
        />
        <StatChart
          label="Network"
          current={
            lastHost
              ? `↓${formatKbPerSec(lastHost.network_rx_kb_per_sec)} ↑${formatKbPerSec(lastHost.network_tx_kb_per_sec)}`
              : null
          }
          data={hostHistory.map((m) => m.network_rx_kb_per_sec + m.network_tx_kb_per_sec)}
        />
      </div>

      <h2 className="text-sm font-medium">Sandboxes</h2>
      {sandboxes.length === 0 ? (
        <p className="text-sm text-muted-foreground">No sandboxes yet.</p>
      ) : (
        <div className="flex flex-col gap-6">
          {sandboxes.map((sb) => {
            const history = sandboxHistory[sb.id] ?? []
            const last = history[history.length - 1]
            return (
              <div key={sb.id} className="flex flex-col gap-2">
                <span className="text-xs text-muted-foreground">{sb.name ?? sb.sbx_name ?? sb.id}</span>
                <StatChart
                  label="CPU"
                  current={last ? `${last.cpu_percent.toFixed(0)}%` : null}
                  data={history.map((m) => m.cpu_percent)}
                  max={100}
                  value={last?.cpu_percent}
                />
                <StatChart
                  label="Memory"
                  current={last ? `${(last.memory_mb / 1024).toFixed(2)} GB` : null}
                  data={history.map((m) => m.memory_mb)}
                />
                <StatChart
                  label="Network"
                  current={
                    last ? `↓${formatKbPerSec(last.network_rx_bytes)} ↑${formatKbPerSec(last.network_tx_bytes)}` : null
                  }
                  data={history.map((m) => m.network_rx_bytes + m.network_tx_bytes)}
                />
              </div>
            )
          })}
        </div>
      )}
    </div>
  )
}
