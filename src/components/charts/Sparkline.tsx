const WIDTH = 240
const HEIGHT = 48

export function Sparkline({ data, max }: { data: number[]; max: number }) {
  if (data.length < 2) {
    return <div className="flex h-12 items-center text-xs text-muted-foreground">Collecting data…</div>
  }

  const points = data
    .map((value, i) => {
      const x = (i / (data.length - 1)) * WIDTH
      const y = HEIGHT - (Math.min(value, max) / max) * HEIGHT
      return `${x},${y}`
    })
    .join(' ')

  return (
    <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} className="h-12 w-full text-primary" preserveAspectRatio="none">
      <polyline points={points} fill="none" stroke="currentColor" strokeWidth="1.5" vectorEffect="non-scaling-stroke" />
    </svg>
  )
}
