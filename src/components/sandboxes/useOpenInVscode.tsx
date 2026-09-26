import { useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Loader2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent } from '@/components/ui/dialog'

export function useOpenInVscode() {
  const [open, setOpen] = useState(false)
  const [settingUp, setSettingUp] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const answer = useRef<((confirmed: boolean) => void) | null>(null)

  function close(confirmed: boolean) {
    setOpen(false)
    setError(null)
    answer.current?.(confirmed)
    answer.current = null
  }

  async function setUp() {
    setSettingUp(true)
    setError(null)
    try {
      await invoke('setup_sandbox_ssh')
      close(true)
    } catch (e) {
      setError(String(e))
    } finally {
      setSettingUp(false)
    }
  }

  async function openInVscode(id: string) {
    if (!(await invoke<boolean>('is_ssh_setup'))) {
      const confirmed = await new Promise<boolean>((resolve) => {
        answer.current = resolve
        setOpen(true)
      })
      if (!confirmed) return
    }
    await invoke('open_sandbox_vscode', { id })
  }

  const sshSetupDialog = (
    <Dialog open={open} onOpenChange={(next) => !next && !settingUp && close(false)}>
      <DialogContent title="Set up SSH?">
        <Card className="w-full">
          <CardHeader>
            <CardTitle>Set up SSH?</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <p className="text-sm text-muted-foreground">
              VS Code connects to sandboxes over SSH, which isn't set up yet. Run <code>sbx setup ssh</code> now?
            </p>
            {error && <p className="text-sm text-destructive">{error}</p>}
            <div className="flex justify-end gap-2">
              <Button variant="ghost" disabled={settingUp} onClick={() => close(false)}>
                Cancel
              </Button>
              <Button disabled={settingUp} onClick={setUp}>
                {settingUp && <Loader2 className="size-3.5 animate-spin" />}
                {settingUp ? 'Setting up…' : 'Set up'}
              </Button>
            </div>
          </CardContent>
        </Card>
      </DialogContent>
    </Dialog>
  )

  return { openInVscode, sshSetupDialog }
}
