import { Sparkline } from '@/components/charts/Sparkline'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useAppStore } from '@/store/useAppStore'

function formatKbPerSec(kbPerSec: number): string {
  return kbPerSec >= 1024 ? `${(kbPerSec / 1024).toFixed(1)}MB/s` : `${kbPerSec.toFixed(0)}KB/s`
}

function StatChart({
  label,
  current,
  caption,
  data,
  max,
}: {
  label: string
  current: string | null
  caption?: string
  data: number[]
  max?: number
}) {
  return (
    <div className="flex min-w-36 flex-1 flex-col gap-1">
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-xs font-normal text-muted-foreground">{label}</span>
        <span className="text-sm font-medium">{current ?? '—'}</span>
      </div>
      <Sparkline data={data} max={max} />
      {caption && <span className="text-xs text-muted-foreground">{caption}</span>}
    </div>
  )
}

/**
 * `layout="sidebar"` (default) stacks charts in the sticky right column.
 * `layout="bar"` lays them out side by side, wrapping to a new row on
 * narrow screens — used when this panel is pinned to the bottom of the
 * page instead (see SandboxesScreen, below the 820px breakpoint).
 */
export function HostStatsPanel({ layout = 'sidebar' }: { layout?: 'sidebar' | 'bar' }) {
  const hostStats = useAppStore((s) => s.hostStats)
  const hostStatsHistory = useAppStore((s) => s.hostStatsHistory)

  return (
    <Card className={layout === 'bar' ? 'w-full' : 'h-fit'}>
      <CardHeader>
        <CardTitle>This PC</CardTitle>
      </CardHeader>
      <CardContent className={layout === 'bar' ? 'flex flex-wrap justify-between gap-4' : 'flex flex-col gap-4'}>
        <StatChart
          label="CPU"
          current={hostStats ? `${hostStats.cpu_percent.toFixed(0)}%` : null}
          data={hostStatsHistory.map((m) => m.cpu_percent)}
          max={100}
        />
        <StatChart
          label="Memory"
          current={hostStats ? `${hostStats.memory_percent.toFixed(0)}%` : null}
          caption={hostStats ? `${(hostStats.memory_used_mb / 1024).toFixed(1)} / ${(hostStats.memory_total_mb / 1024).toFixed(1)} GB` : undefined}
          data={hostStatsHistory.map((m) => m.memory_percent)}
          max={100}
        />
        <StatChart
          label="Disk"
          current={hostStats ? `${hostStats.disk_percent.toFixed(0)}%` : null}
          caption={hostStats ? `${(hostStats.disk_used_mb / 1024).toFixed(0)} / ${(hostStats.disk_total_mb / 1024).toFixed(0)} GB` : undefined}
          data={hostStatsHistory.map((m) => m.disk_percent)}
          max={100}
        />
        <StatChart
          label="Network"
          current={
            hostStats
              ? `↓${formatKbPerSec(hostStats.network_rx_kb_per_sec)} ↑${formatKbPerSec(hostStats.network_tx_kb_per_sec)}`
              : null
          }
          data={hostStatsHistory.map((m) => m.network_rx_kb_per_sec + m.network_tx_kb_per_sec)}
        />
      </CardContent>
    </Card>
  )
}
