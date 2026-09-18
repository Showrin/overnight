import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent } from '@/components/ui/dialog'

/**
 * Shown once a credentials save (global, project, or sandbox-scoped)
 * completes — the two caveats that aren't obvious from the UI alone:
 * already-open terminals don't pick up the change, and a stopped sandbox
 * only gets it on its next start.
 */
export function EnvVarsSavedDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent title="Credentials saved">
        <Card className="w-full">
          <CardHeader>
            <CardTitle>Credentials saved</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <p className="text-sm text-muted-foreground">
              Already-open terminals won't see the new values — open a new terminal session in the sandbox to access them.
            </p>
            <p className="text-sm text-muted-foreground">
              If a sandbox is currently stopped, this change takes effect the next time it's started, not retroactively.
            </p>
            <div className="flex justify-end">
              <Button onClick={() => onOpenChange(false)}>Got it</Button>
            </div>
          </CardContent>
        </Card>
      </DialogContent>
    </Dialog>
  )
}
