import { useState } from 'react'
import { Eye, EyeOff } from 'lucide-react'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'

/**
 * A single-line value hidden behind •••• by default, with an eye toggle
 * to reveal it — shared by every place a credential value is shown
 * (global/project/sandbox credential lists).
 */
export function MaskedValueInput({
  value,
  onChange,
  readOnly = false,
  className,
}: {
  value: string
  onChange: (value: string) => void
  readOnly?: boolean
  className?: string
}) {
  const [revealed, setRevealed] = useState(false)

  return (
    <div className="relative min-w-0 flex-1">
      <Input
        type={revealed ? 'text' : 'password'}
        value={value}
        readOnly={readOnly}
        onChange={(e) => onChange(e.target.value)}
        className={cn('pr-8', readOnly && 'cursor-default bg-muted/30', className)}
      />
      <button
        type="button"
        tabIndex={-1}
        onClick={() => setRevealed((r) => !r)}
        aria-label={revealed ? 'Hide value' : 'Show value'}
        className="absolute top-1/2 right-2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
      >
        {revealed ? <EyeOff className="size-3.5" strokeWidth={1.5} /> : <Eye className="size-3.5" strokeWidth={1.5} />}
      </button>
    </div>
  )
}
