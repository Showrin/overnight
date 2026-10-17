import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { ComposeOverrideEditor, composeOverrideError } from '@/components/settings/ComposeOverrideEditor'
import { SettingsSection } from '@/components/settings/SettingsLayout'
import { Button } from '@/components/ui/button'
import { useAppStore } from '@/store/useAppStore'
import type { Sandbox } from './types'

export function SandboxComposeOverrideSection({ sandbox }: { sandbox: Sandbox }) {
  const loadSandboxes = useAppStore((s) => s.loadSandboxes)
  const projectOverride = useAppStore((s) => s.projects.find((p) => p.id === sandbox.project_id)?.compose_override ?? null)
  const saved = sandbox.compose_override ?? ''
  const [value, setValue] = useState(saved)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const dirty = value !== saved

  async function save() {
    setSaving(true)
    setError(null)
    try {
      await invoke('save_sandbox_compose_override', { id: sandbox.id, content: value.trim() ? value : null })
      await loadSandboxes()
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }

  return (
    <SettingsSection title="Docker Compose Override">
      <div className="flex flex-col gap-2 py-4">
        <p className="text-xs text-muted-foreground">
          Written next to the workspace compose file as docker-compose.override.yml (or compose.override.yaml) and
          excluded from git. Overrides the project value. Leave empty to use the project override.
        </p>
        <ComposeOverrideEditor value={value} onChange={setValue} placeholder={projectOverride ?? undefined} />
        {error && <p className="text-xs break-all text-destructive">{error}</p>}
        <div className="flex justify-end gap-2">
          <Button variant="outline" size="sm" onClick={() => setValue(saved)} disabled={!dirty || saving}>
            Cancel
          </Button>
          <Button size="sm" onClick={save} disabled={!dirty || saving || composeOverrideError(value) != null}>
            {saving ? 'Saving…' : 'Save override'}
          </Button>
        </div>
      </div>
    </SettingsSection>
  )
}
