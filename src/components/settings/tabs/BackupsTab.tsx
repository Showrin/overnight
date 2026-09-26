import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { SettingsRow } from '../SettingsLayout'

interface BackupsTabProps {
  autoBackup: boolean
  setAutoBackup: (enabled: boolean) => void
  backupInterval: number
  setBackupInterval: (minutes: number) => void
  keepCount: number
  setKeepCount: (count: number) => void
}

export function BackupsTab({
  autoBackup,
  setAutoBackup,
  backupInterval,
  setBackupInterval,
  keepCount,
  setKeepCount,
}: BackupsTabProps) {
  return (
    <>
      <SettingsRow
        label="Auto backup"
        description="Periodically back up running sandboxes' .claude and .git folders."
      >
        <Switch checked={autoBackup} onCheckedChange={setAutoBackup} />
      </SettingsRow>

      <SettingsRow label="Backups kept per sandbox" description="Older backups beyond this number are deleted (1–10).">
        <Input
          type="number"
          min={1}
          max={10}
          className="max-w-40"
          value={keepCount}
          disabled={!autoBackup}
          onChange={(e) => setKeepCount(Math.min(10, Math.max(1, Number(e.target.value) || 1)))}
        />
      </SettingsRow>

      <SettingsRow label="Backup interval" description="Default interval in minutes. Each sandbox can override it." divider={false}>
        <Input
          type="number"
          min={1}
          className="max-w-40"
          value={backupInterval}
          disabled={!autoBackup}
          onChange={(e) => setBackupInterval(Number(e.target.value))}
        />
      </SettingsRow>
    </>
  )
}
