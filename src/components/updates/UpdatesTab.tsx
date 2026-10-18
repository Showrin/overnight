import { CheckCircle2, Loader2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { SettingsRow } from '@/components/settings/SettingsLayout'
import { formatRelativeTime } from '@/lib/sandboxDisplay'
import { useUpdater } from '@/store/useUpdater'
import { UpdateChangelog } from './UpdateChangelog'
import { UpdateProgress } from './UpdateProgress'

export function UpdatesTab() {
  const currentVersion = useUpdater((s) => s.currentVersion)
  const status = useUpdater((s) => s.status)
  const update = useUpdater((s) => s.update)
  const changelog = useUpdater((s) => s.changelog)
  const error = useUpdater((s) => s.error)
  const lastCheckedAt = useUpdater((s) => s.lastCheckedAt)
  const checkForUpdate = useUpdater((s) => s.checkForUpdate)
  const installUpdate = useUpdater((s) => s.installUpdate)

  const installing = status === 'downloading' || status === 'installing'
  const busy = installing || status === 'checking'

  return (
    <>
      <SettingsRow
        label="Version"
        description={
          lastCheckedAt
            ? `Last checked ${formatRelativeTime(lastCheckedAt)}.`
            : 'Checks for updates on launch and every 6 hours.'
        }
      >
        <div className="flex flex-wrap items-center gap-3">
          <span className="font-mono text-sm text-foreground">v{currentVersion ?? '…'}</span>
          <Button variant="outline" size="sm" disabled={busy} onClick={() => checkForUpdate()}>
            {status === 'checking' && <Loader2 className="animate-spin" />}
            {status === 'checking' ? 'Checking…' : 'Check for updates'}
          </Button>
          {status === 'up-to-date' && (
            <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
              <CheckCircle2 className="size-3.5 text-emerald-500" />
              You're on the latest version.
            </span>
          )}
        </div>
        {status === 'error' && error && <p className="text-xs text-destructive">{error}</p>}
      </SettingsRow>

      {update && (
        <SettingsRow
          label={`v${update.version} available`}
          description="Downloads, installs and restarts the app."
          divider={false}
        >
          <UpdateProgress />
          <div>
            <Button size="sm" disabled={busy} onClick={installUpdate}>
              {installing ? 'Updating…' : 'Update and restart'}
            </Button>
          </div>
          <div className="mt-2 rounded-lg border border-border p-4">
            <UpdateChangelog entries={changelog} />
          </div>
        </SettingsRow>
      )}
    </>
  )
}
