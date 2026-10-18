import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Loader2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

const CONFIRM_WORD = 'purge'
// Older logs are already dropped by the daily cleanup, so earlier dates do nothing.
const MAX_DAYS_BACK = 30

function toDateInput(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
}

function daysAgo(days: number): Date {
  const d = new Date()
  d.setDate(d.getDate() - days)
  return d
}

// Start of the chosen day in local time.
function startOfDayMs(value: string): number {
  const [y, m, d] = value.split('-').map(Number)
  return new Date(y, m - 1, d).getTime()
}

export function PurgeCommandLogDialog({
  open,
  onOpenChange,
  onPurged,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onPurged: (removed: number, date: string) => void
}) {
  const minDate = toDateInput(daysAgo(MAX_DAYS_BACK))
  const maxDate = toDateInput(new Date())
  const [typed, setTyped] = useState('')
  const [date, setDate] = useState(minDate)
  const [purging, setPurging] = useState(false)
  const [error, setError] = useState<string | null>(null)

  function handleOpenChange(next: boolean) {
    if (purging) return
    if (!next) {
      setTyped('')
      setDate(minDate)
      setError(null)
    }
    onOpenChange(next)
  }

  async function purge() {
    setPurging(true)
    setError(null)
    try {
      const removed = await invoke<number>('purge_command_log_before', { beforeMs: startOfDayMs(date) })
      setTyped('')
      setDate(minDate)
      onOpenChange(false)
      onPurged(removed, date)
    } catch (e) {
      setError(String(e))
    } finally {
      setPurging(false)
    }
  }

  const dateValid = date !== '' && date >= minDate && date <= maxDate
  const canPurge = typed === CONFIRM_WORD && dateValid && !purging

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent title="Purge old command logs?">
        <Card className="w-full">
          <CardHeader>
            <CardTitle>Purge old command logs?</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <p className="text-sm text-muted-foreground">
              This permanently deletes every command log entry before the chosen day. It can't be undone.
            </p>
            <div className="flex flex-col gap-2">
              <Label htmlFor="purge-date">Delete logs before</Label>
              <Input
                id="purge-date"
                type="date"
                value={date}
                min={minDate}
                max={maxDate}
                onChange={(e) => setDate(e.target.value)}
              />
              <span className="text-xs text-muted-foreground">Up to {MAX_DAYS_BACK} days back.</span>
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="purge-confirm">
                Type <span className="font-mono font-semibold text-foreground">{CONFIRM_WORD}</span> to confirm
              </Label>
              <Input
                id="purge-confirm"
                value={typed}
                autoComplete="off"
                autoFocus
                onChange={(e) => setTyped(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && canPurge) purge()
                }}
              />
            </div>
            {error && <p className="text-sm text-destructive">{error}</p>}
            <div className="flex justify-end gap-2">
              <Button variant="ghost" onClick={() => handleOpenChange(false)} disabled={purging}>
                Cancel
              </Button>
              <Button variant="destructive" onClick={purge} disabled={!canPurge}>
                {purging && <Loader2 className="size-3.5 animate-spin" />}
                Purge
              </Button>
            </div>
          </CardContent>
        </Card>
      </DialogContent>
    </Dialog>
  )
}
