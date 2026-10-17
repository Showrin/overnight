import { cn } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'

function formatKbPerSec(kbPerSec: number): string {
  return kbPerSec >= 1024 ? `${(kbPerSec / 1024).toFixed(1)}MB/s` : `${kbPerSec.toFixed(0)}KB/s`
}

function Meter({ label, percent, value, className }: { label: string; percent: number | null; value: string; className: string }) {
  const width = percent == null ? 0 : Math.min(100, Math.max(0, percent))
  return (
    <div className="flex shrink-0 items-center gap-2">
      <span className="text-muted-foreground">{label}</span>
      <div className={cn('h-2.5 w-16 overflow-hidden rounded-sm border border-current', className)}>
        <div className="h-full bg-current" style={{ width: `${width}%` }} />
      </div>
      <span className={cn('tabular-nums', className)}>{value}</span>
    </div>
  )
}

/** One-line host monitor pinned to the page bottom. Same height as the sidebar footer. */
export function HostStatsBar() {
  const stats = useAppStore((s) => s.hostStats)
  const history = useAppStore((s) => s.hostStatsHistory)
  // Network has no fixed ceiling, so the meter is relative to the recent peak.
  const peak = Math.max(1, ...history.map((m) => m.network_rx_kb_per_sec + m.network_tx_kb_per_sec))
  const network = stats ? stats.network_rx_kb_per_sec + stats.network_tx_kb_per_sec : null

  return (
    <div className="flex shrink-0 items-center overflow-hidden border-t border-sidebar-border px-4 py-1.5 text-xs">
      <div className="flex h-7 min-w-0 items-center gap-6 overflow-hidden">
        <Meter
          label="CPU"
          className="text-red-400"
          percent={stats?.cpu_percent ?? null}
          value={stats ? `${stats.cpu_percent.toFixed(0)}%` : '—'}
        />
        <Meter
          label="Memory"
          className="text-green-500"
          percent={stats?.memory_percent ?? null}
          value={stats ? `${(stats.memory_used_mb / 1024).toFixed(1)}/${(stats.memory_total_mb / 1024).toFixed(1)} GB` : '—'}
        />
        <Meter
          label="Disk"
          className="text-orange-500"
          percent={stats?.disk_percent ?? null}
          value={stats ? `${(stats.disk_used_mb / 1024).toFixed(0)}/${(stats.disk_total_mb / 1024).toFixed(0)} GB` : '—'}
        />
        <Meter
          label="Network"
          className="text-violet-400"
          percent={network == null ? null : (network / peak) * 100}
          value={
            stats
              ? `↓${formatKbPerSec(stats.network_rx_kb_per_sec)} ↑${formatKbPerSec(stats.network_tx_kb_per_sec)}`
              : '—'
          }
        />
      </div>
    </div>
  )
}
