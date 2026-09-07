import { useId } from 'react'
import { usageBorderClass, usageColorClass } from '@/lib/usageColor'

const WIDTH = 240
const HEIGHT = 80

export function Sparkline({ data, max, value }: { data: number[]; max?: number; value?: number }) {
  const patternId = useId().replace(/:/g, '')

  if (data.length < 2) {
    return <div className="flex h-12 items-center text-xs text-muted-foreground">Collecting data…</div>
  }

  // For fixed-range metrics (percentages), pass max explicitly. For
  // open-ended ones (e.g. network KB/s) omit it to auto-scale off the
  // series' own peak, so the line stays readable as throughput varies.
  const effectiveMax = max ?? Math.max(...data, 1)

  const points = data
    .map((v, i) => {
      const x = (i / (data.length - 1)) * WIDTH
      const y = HEIGHT - (Math.min(v, effectiveMax) / effectiveMax) * HEIGHT
      return `${x},${y}`
    })
    .join(' ')

  const lineColorClass = value !== undefined ? usageColorClass(value) : 'text-primary'
  const borderColorClass = value !== undefined ? usageBorderClass(value) : 'border-primary/20'

  return (
    <svg
      viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
      className={`h-20 w-full overflow-hidden rounded-sm border transition-colors duration-300 ${lineColorClass} ${borderColorClass}`}
      preserveAspectRatio="none"
    >
      <defs>
        <pattern id={patternId} width="12" height="12" patternUnits="userSpaceOnUse">
          <circle cx="1" cy="1" r="1" fill="currentColor" opacity="0.2" />
        </pattern>
      </defs>
      <rect width={WIDTH} height={HEIGHT} fill={`url(#${patternId})`} className="text-border" />
      <polyline points={points} fill="none" stroke="currentColor" strokeWidth="1.5" vectorEffect="non-scaling-stroke" />
    </svg>
  )
}
