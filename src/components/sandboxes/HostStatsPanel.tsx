import { Sparkline } from '@/components/charts/Sparkline'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useAppStore } from '@/store/useAppStore'

function formatKbPerSec(kbPerSec: number): string {
  return kbPerSec >= 1024 ? `${(kbPerSec / 1024).toFixed(1)}MB/s` : `${kbPerSec.toFixed(0)}KB/s`
}

function StatChart({
  label,
  current,
  data,
  max,
}: {
  label: string
  current: string | null
  data: number[]
  max?: number
}) {
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-baseline justify-between">
        <span className="text-xs font-normal text-muted-foreground">{label}</span>
        <span className="text-sm font-medium">{current ?? '—'}</span>
      </div>
      <Sparkline data={data} max={max} />
    </div>
  )
}

export function HostStatsPanel() {
  const hostStats = useAppStore((s) => s.hostStats)
  const hostStatsHistory = useAppStore((s) => s.hostStatsHistory)

  return (
    <Card className="h-fit">
      <CardHeader>
        <CardTitle>This PC</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <StatChart
          label="CPU"
          current={hostStats ? `${hostStats.cpu_percent.toFixed(0)}%` : null}
          data={hostStatsHistory.map((m) => m.cpu_percent)}
          max={100}
        />
        <StatChart
          label="Memory"
          current={hostStats ? `${hostStats.memory_percent.toFixed(0)}%` : null}
          data={hostStatsHistory.map((m) => m.memory_percent)}
          max={100}
        />
        {hostStats && (
          <span className="-mt-3 text-xs text-muted-foreground">
            {(hostStats.memory_used_mb / 1024).toFixed(1)} / {(hostStats.memory_total_mb / 1024).toFixed(1)} GB
          </span>
        )}
        <StatChart
          label="Disk"
          current={hostStats ? `${hostStats.disk_percent.toFixed(0)}%` : null}
          data={hostStatsHistory.map((m) => m.disk_percent)}
          max={100}
        />
        {hostStats && (
          <span className="-mt-3 text-xs text-muted-foreground">
            {(hostStats.disk_used_mb / 1024).toFixed(0)} / {(hostStats.disk_total_mb / 1024).toFixed(0)} GB
          </span>
        )}
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
