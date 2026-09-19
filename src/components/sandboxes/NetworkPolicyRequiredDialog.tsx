import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

export function NetworkPolicyRequiredDialog({
  onGoToSettings,
  onCancel,
}: {
  onGoToSettings: () => void
  onCancel: () => void
}) {
  return (
    <Card className="w-full">
      <CardHeader>
        <CardTitle>Network policy required</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <p className="rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
          sbx needs a network policy set up on this machine before you can create a sandbox. Without selecting
          one in Settings, you can't create a sandbox.
        </p>
        <div className="flex justify-end gap-2">
          <Button variant="outline" onClick={onCancel}>
            Cancel
          </Button>
          <Button onClick={onGoToSettings}>Go to Settings</Button>
        </div>
      </CardContent>
    </Card>
  )
}
