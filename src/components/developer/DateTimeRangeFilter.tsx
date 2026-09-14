import { useState } from 'react'
import { CalendarClock } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'

function formatBound(value: string): string {
  return new Date(value).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })
}

function summarize(from: string, to: string): string {
  if (from && to) return `${formatBound(from)} – ${formatBound(to)}`
  if (from) return `From ${formatBound(from)}`
  if (to) return `Until ${formatBound(to)}`
  return 'All time'
}

// Popover-based date-time range picker, shared by the Daemon Logs and
// Command Logs pages so their filter bars look and behave the same way.
// Edits are drafted inside the popover and only committed on Apply, so
// picking a "from" value doesn't refetch before "to" is also set.
export function DateTimeRangeFilter({
  from,
  to,
  onChange,
}: {
  from: string
  to: string
  onChange: (from: string, to: string) => void
}) {
  const [open, setOpen] = useState(false)
  const [draftFrom, setDraftFrom] = useState(from)
  const [draftTo, setDraftTo] = useState(to)

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (next) {
          setDraftFrom(from)
          setDraftTo(to)
        }
      }}
    >
      <PopoverTrigger asChild>
        <Button variant="outline" size="lg" className="gap-1.5">
          <CalendarClock className="size-3.5" />
          {summarize(from, to)}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="start" className="flex w-72 flex-col gap-3 p-3">
        <div className="flex flex-col gap-1">
          <Label className="text-xs text-muted-foreground">From</Label>
          <Input type="datetime-local" value={draftFrom} onChange={(e) => setDraftFrom(e.target.value)} />
        </div>
        <div className="flex flex-col gap-1">
          <Label className="text-xs text-muted-foreground">To</Label>
          <Input type="datetime-local" value={draftTo} onChange={(e) => setDraftTo(e.target.value)} />
        </div>
        <div className="flex justify-end gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              setDraftFrom('')
              setDraftTo('')
              onChange('', '')
              setOpen(false)
            }}
          >
            Clear
          </Button>
          <Button
            size="sm"
            onClick={() => {
              onChange(draftFrom, draftTo)
              setOpen(false)
            }}
          >
            Apply
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  )
}
