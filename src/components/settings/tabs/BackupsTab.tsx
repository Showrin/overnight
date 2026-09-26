import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { SettingsRow } from '../SettingsLayout'

interface BackupsTabProps {
  autoBackup: boolean
  setAutoBackup: (enabled: boolean) => void
  backupInterval: number
  setBackupInterval: (minutes: number) => void
}

export function BackupsTab({ autoBackup, setAutoBackup, backupInterval, setBackupInterval }: BackupsTabProps) {
  return (
    <>
      <SettingsRow
        label="Auto backup"
        description="Periodically back up running sandboxes' .claude and .git folders."
      >
        <Switch checked={autoBackup} onCheckedChange={setAutoBackup} />
      </SettingsRow>

      <SettingsRow label="Backup interval" description="How often (in minutes) auto backup runs." divider={false}>
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
