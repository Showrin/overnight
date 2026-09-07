import { useId, useLayoutEffect, useState } from 'react'
import { usageBorderClass, usageColorClass } from '@/lib/usageColor'

const DEFAULT_HEIGHT = 80

export function Sparkline({
  data,
  max,
  value,
  height = DEFAULT_HEIGHT,
}: {
  data: number[]
  max?: number
  value?: number
  height?: number
}) {
  const patternId = useId().replace(/:/g, '')
  // A callback ref (stored as state) rather than a plain useRef: this
  // component renders a placeholder <div> until `data` has 2+ points, then
  // switches to the real <svg> — so the svg node may not exist yet on this
  // component's first mount. A useRef + useLayoutEffect(fn, []) would only
  // ever see that first, still-null ref and never re-measure once the real
  // svg later appears. Storing the node in state makes its arrival a
  // dependency change, so the effect below re-runs exactly when there's
  // finally something to measure.
  const [svgEl, setSvgEl] = useState<SVGSVGElement | null>(null)
  const [width, setWidth] = useState(0)

  // Tracks the SVG's own rendered pixel width rather than a hardcoded
  // constant, so the viewBox's internal coordinate system matches real
  // screen pixels 1:1 — otherwise the dot grid (patternUnits="userSpaceOnUse")
  // gets stretched non-uniformly by preserveAspectRatio="none" and renders
  // as ellipses instead of circles. useLayoutEffect (not useEffect) measures
  // synchronously before paint, so the first real frame already has the
  // right width.
  useLayoutEffect(() => {
    if (!svgEl) return
    const observer = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width))
    observer.observe(svgEl)
    setWidth(svgEl.getBoundingClientRect().width)
    return () => observer.disconnect()
  }, [svgEl])

  if (data.length < 2) {
    return (
      <div className="flex items-center text-xs text-muted-foreground" style={{ height }}>
        Collecting data…
      </div>
    )
  }

  // For fixed-range metrics (percentages), pass max explicitly. For
  // open-ended ones (e.g. network KB/s) omit it to auto-scale off the
  // series' own peak, so the line stays readable as throughput varies.
  const effectiveMax = max ?? Math.max(...data, 1)

  const linePoints = data.map((v, i) => ({
    x: (i / (data.length - 1)) * width,
    y: height - (Math.min(v, effectiveMax) / effectiveMax) * height,
  }))
  const polylinePoints = linePoints.map((p) => `${p.x},${p.y}`).join(' ')
  // Same points, closed into a shape by dropping down to the bottom edge
  // under the last and first points — fills the "area under the line"
  // rather than just outlining it.
  const areaPoints = `${polylinePoints} ${linePoints[linePoints.length - 1].x},${height} ${linePoints[0].x},${height}`

  const lineColorClass = value !== undefined ? usageColorClass(value) : 'text-primary'
  const borderColorClass = value !== undefined ? usageBorderClass(value) : 'border-primary/20'
  const gradientId = `${patternId}-gradient`

  return (
    <svg
      ref={setSvgEl}
      viewBox={`0 0 ${width} ${height}`}
      style={{ height }}
      className={`w-full overflow-hidden rounded-sm border transition-colors duration-300 ${lineColorClass} ${borderColorClass}`}
      preserveAspectRatio="none"
    >
      <defs>
        <pattern id={patternId} width="12" height="12" patternUnits="userSpaceOnUse">
          <circle cx="1" cy="1" r="1" fill="currentColor" opacity="0.2" />
        </pattern>
        {/* Densest right under the line, fading to fully transparent at the
            bottom — stopColor="currentColor" ties it to the same per-instance
            line color as the polyline/border below, no separate color logic. */}
        <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="currentColor" stopOpacity="0.35" />
          <stop offset="100%" stopColor="currentColor" stopOpacity="0" />
        </linearGradient>
      </defs>
      <rect width={width} height={height} fill={`url(#${patternId})`} className="text-border" />
      <polygon points={areaPoints} fill={`url(#${gradientId})`} stroke="none" />
      <polyline
        points={polylinePoints}
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  )
}
