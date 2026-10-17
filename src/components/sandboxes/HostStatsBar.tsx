import { usageBorderClass, usageColorClass } from '@/lib/usageColor'
import { cn } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'

function formatKbPerSec(kbPerSec: number): string {
  return kbPerSec >= 1024 ? `${(kbPerSec / 1024).toFixed(1)}MB/s` : `${kbPerSec.toFixed(0)}KB/s`
}

function Meter({ label, percent, value, usage }: { label: string; percent: number | null; value: string; usage: boolean }) {
  const width = percent == null ? 0 : Math.min(100, Math.max(0, percent))
  // Same color rules as the Sparkline charts on the sandbox list.
  const color = usage && percent != null ? usageColorClass(percent) : 'text-primary'
  const border = usage && percent != null ? usageBorderClass(percent) : 'border-primary/20'
  return (
    <div className="flex shrink-0 items-center gap-2">
      <span className="text-muted-foreground">{label}</span>
      <div className={cn('relative h-4 w-20 overflow-hidden rounded-[2px] border transition-colors duration-300', color, border)}>
        <div
          className="absolute inset-0 text-border"
          style={{
            backgroundImage: 'radial-gradient(circle, color-mix(in oklch, currentColor 20%, transparent) 1px, transparent 1px)',
            backgroundSize: '6px 6px',
          }}
        />
        <div
          className="absolute inset-y-0 left-0 border-r-[1.5px] border-current transition-[width] duration-300"
          style={{
            width: `${width}%`,
            backgroundImage: 'linear-gradient(to left, transparent,color-mix(in oklch, currentColor 35%, transparent))',
            borderRightWidth: width > 0 ? undefined : 0,
          }}
        />
      </div>
      <span className={cn('tabular-nums', color)}>{value}</span>
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
          usage
          percent={stats?.cpu_percent ?? null}
          value={stats ? `${stats.cpu_percent.toFixed(0)}%` : '—'}
        />
        <Meter
          label="Memory"
          usage
          percent={stats?.memory_percent ?? null}
          value={stats ? `${(stats.memory_used_mb / 1024).toFixed(1)}/${(stats.memory_total_mb / 1024).toFixed(1)} GB` : '—'}
        />
        <Meter
          label="Disk"
          usage
          percent={stats?.disk_percent ?? null}
          value={stats ? `${(stats.disk_used_mb / 1024).toFixed(0)}/${(stats.disk_total_mb / 1024).toFixed(0)} GB` : '—'}
        />
        <Meter
          label="Network"
          usage={false}
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
