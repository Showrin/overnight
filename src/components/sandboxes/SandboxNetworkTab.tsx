import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Search } from 'lucide-react'
import { NetworkRuleEditor } from '@/components/settings/NetworkRuleEditor'
import { SettingsRow } from '@/components/settings/SettingsLayout'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import type { NetworkRuleDecision, PolicyRule } from '@/lib/networkPolicy'
import { SANDBOX_NETWORK_PRESET_OVERRIDE_LABELS, SANDBOX_NETWORK_PRESET_OVERRIDES } from '@/lib/networkPolicy'
import { formatNetworkPolicyLabel } from '@/lib/sandboxDisplay'
import { useAppStore } from '@/store/useAppStore'
import type { Sandbox } from './types'

const DEFAULT_NETWORK_OVERRIDE = '__global_default__'

// Same preset override + per-sandbox rule editor the sandbox card offers
// behind its "N rule(s)" toggle, laid out as a full detail-page tab.
export function SandboxNetworkTab({ sandbox }: { sandbox: Sandbox }) {
  const networkPolicyPreset = useAppStore((s) => s.networkPolicyPreset)
  const loadSandboxes = useAppStore((s) => s.loadSandboxes)
  const openSettings = useAppStore((s) => s.openSettings)
  const [rules, setRules] = useState<PolicyRule[]>([])
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [search, setSearch] = useState('')

  async function refreshRules() {
    setError(null)
    try {
      setRules(await invoke<PolicyRule[]>('get_sandbox_network_rules', { id: sandbox.id }))
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    refreshRules()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sandbox.id])

  async function addRule(decision: NetworkRuleDecision, host: string) {
    await invoke('add_sandbox_network_rule', { id: sandbox.id, decision, host })
    await refreshRules()
  }

  async function removeRule(host: string) {
    await invoke('remove_sandbox_network_rule', { id: sandbox.id, host })
    await refreshRules()
  }

  async function changePreset(value: string) {
    const preset = value === DEFAULT_NETWORK_OVERRIDE ? null : value
    setSaving(true)
    setError(null)
    try {
      await invoke('set_sandbox_network_preset_override', { id: sandbox.id, preset })
      await refreshRules()
      await loadSandboxes()
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }

  const query = search.trim().toLowerCase()
  const visibleRules = query ? rules.filter((rule) => rule.host.toLowerCase().includes(query)) : rules

  return (
    <div className="flex flex-col">
      <SettingsRow
        label="Network policy"
        description={
          <>
            Currently {formatNetworkPolicyLabel(sandbox.network_preset_override, networkPolicyPreset)}. Only Open or
            Locked Down can be scoped to a single sandbox — Balanced is a machine-wide baseline set in{' '}
            <button type="button" className="underline hover:text-foreground" onClick={() => openSettings('network')}>
              Settings › Network
            </button>
            .
          </>
        }
      >
        <Select
          value={sandbox.network_preset_override ?? DEFAULT_NETWORK_OVERRIDE}
          onValueChange={changePreset}
          disabled={saving}
        >
          <SelectTrigger className="max-w-sm">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={DEFAULT_NETWORK_OVERRIDE}>Use global default</SelectItem>
            {SANDBOX_NETWORK_PRESET_OVERRIDES.map((preset) => (
              <SelectItem key={preset} value={preset}>
                {SANDBOX_NETWORK_PRESET_OVERRIDE_LABELS[preset]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </SettingsRow>

      <SettingsRow
        label="Sandbox rules"
        description="Allow or deny specific hosts for this sandbox only. Applied as soon as you add or remove them."
        divider={false}
      >
        {loading ? (
          <p className="text-xs text-muted-foreground">Loading…</p>
        ) : (
          <NetworkRuleEditor
            rules={visibleRules}
            onAdd={addRule}
            onRemove={removeRule}
            filterSlot={
              rules.length > 0 && (
                <div className="relative">
                  <Search
                    className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
                    strokeWidth={1.5}
                  />
                  <Input
                    className="pl-8"
                    placeholder="Search hosts"
                    value={search}
                    onChange={(e) => setSearch(e.target.value)}
                  />
                </div>
              )
            }
          />
        )}
        {error && <p className="text-xs text-destructive">{error}</p>}
      </SettingsRow>
    </div>
  )
}
