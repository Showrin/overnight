import { FlaskConical } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'

/** `compact` shows a bare icon (nav items, tabs); otherwise an icon + label badge (titles). */
export function ExperimentalBadge({ compact = false, className }: { compact?: boolean; className?: string }) {
  if (compact) {
    return (
      <span title="Experimental" aria-label="Experimental" className={cn('inline-flex shrink-0 text-info', className)}>
        <FlaskConical className="size-3.5" />
      </span>
    )
  }
  return (
    <Badge variant="outline-info" title="Experimental" className={className}>
      <FlaskConical />
      Experimental
    </Badge>
  )
}
