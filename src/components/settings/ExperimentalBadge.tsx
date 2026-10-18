import { FlaskConical } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'

/** `compact` shows the icon only (nav items); otherwise icon + label (titles). */
export function ExperimentalBadge({ compact = false, className }: { compact?: boolean; className?: string }) {
  return (
    <Badge
      variant="outline-warning"
      title="Experimental"
      aria-label={compact ? 'Experimental' : undefined}
      className={cn(compact && 'px-1', className)}
    >
      <FlaskConical />
      {!compact && 'Experimental'}
    </Badge>
  )
}
