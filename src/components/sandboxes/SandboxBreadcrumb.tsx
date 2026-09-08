import { ChevronRight } from 'lucide-react'

export interface BreadcrumbSegment {
  label: string
  onClick?: () => void
}

export function SandboxBreadcrumb({ segments }: { segments: BreadcrumbSegment[] }) {
  return (
    <nav className="flex items-center gap-1.5 text-sm">
      {segments.map((segment, i) => (
        <span key={i} className="flex items-center gap-1.5">
          {i > 0 && <ChevronRight className="size-3.5 shrink-0 text-muted-foreground" />}
          {segment.onClick ? (
            <button
              type="button"
              onClick={segment.onClick}
              className="text-muted-foreground hover:text-foreground hover:underline"
            >
              {segment.label}
            </button>
          ) : (
            <span className="font-medium text-foreground">{segment.label}</span>
          )}
        </span>
      ))}
    </nav>
  )
}
