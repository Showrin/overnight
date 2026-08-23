import { Sparkline } from '@/components/charts/Sparkline'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useAppStore } from '@/store/useAppStore'

function StatChart({ label, current, unit, data, max }: { label: string; current: number | null; unit: string; data: number[]; max: number }) {
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-baseline justify-between">
        <span className="text-xs font-normal text-muted-foreground">{label}</span>
        <span className="text-sm font-medium">{current != null ? `${current.toFixed(0)}${unit}` : '—'}</span>
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
          unit="%"
          current={hostStats?.cpu_percent ?? null}
          data={hostStatsHistory.map((m) => m.cpu_percent)}
          max={100}
        />
        <StatChart
          label="Memory"
          unit="%"
          current={hostStats?.memory_percent ?? null}
          data={hostStatsHistory.map((m) => m.memory_percent)}
          max={100}
        />
        {hostStats && (
          <span className="text-xs text-muted-foreground">
            {(hostStats.memory_used_mb / 1024).toFixed(1)} / {(hostStats.memory_total_mb / 1024).toFixed(1)} GB
          </span>
        )}
      </CardContent>
    </Card>
  )
}
