import { useEffect, useState } from 'react'
import { Search } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import type { NetworkPolicySettings, NetworkRuleDecision } from '@/lib/networkPolicy'
import { NETWORK_POLICY_PRESET_LABELS, NETWORK_POLICY_PRESETS } from '@/lib/networkPolicy'
import { NetworkRuleEditor } from '../NetworkRuleEditor'
import { SettingsRow } from '../SettingsLayout'

interface NetworkTabProps {
  networkPreset: string
  setNetworkPreset: (preset: string) => void
  networkInitialized: boolean
  savingNetwork: boolean

  loadingNetworkPolicy: boolean
  networkRules: NetworkPolicySettings['rules']
  onAddNetworkRule: (decision: NetworkRuleDecision, host: string) => Promise<void>
  onRemoveNetworkRule: (host: string) => Promise<void>

  confirmingPresetChange: boolean
  setConfirmingPresetChange: (confirming: boolean) => void
  onConfirmNetworkPresetChange: () => void

  highlightPreset: boolean
  onHighlightPresetShown: () => void
}

export function NetworkTab({
  networkPreset,
  setNetworkPreset,
  networkInitialized,
  savingNetwork,
  loadingNetworkPolicy,
  networkRules,
  onAddNetworkRule,
  onRemoveNetworkRule,
  confirmingPresetChange,
  setConfirmingPresetChange,
  onConfirmNetworkPresetChange,
  highlightPreset,
  onHighlightPresetShown,
}: NetworkTabProps) {
  const [ruleSearch, setRuleSearch] = useState('')

  // Highlight is a one-shot cue from the "network policy required" dialog —
  // clears itself so it doesn't linger if the user stays on this tab.
  useEffect(() => {
    if (!highlightPreset) return
    const timer = setTimeout(onHighlightPresetShown, 2500)
    return () => clearTimeout(timer)
  }, [highlightPreset, onHighlightPresetShown])

  const query = ruleSearch.trim().toLowerCase()
  const visibleRules = query ? networkRules.filter((rule) => rule.host.toLowerCase().includes(query)) : networkRules

  return (
    <>
      <SettingsRow
        label="Network policy preset"
        description={
          networkInitialized
            ? 'Changing this may interrupt currently running sandboxes — sbx applies network policy presets machine-wide.'
            : 'sbx has no network policy configured on this machine yet. Choose a preset to enable creating sandboxes.'
        }
        className={highlightPreset ? 'rounded-lg ring-2 ring-primary' : undefined}
      >
        <Select value={networkPreset} onValueChange={setNetworkPreset}>
          <SelectTrigger className="max-w-sm">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {NETWORK_POLICY_PRESETS.map((preset) => (
              <SelectItem key={preset} value={preset}>
                {NETWORK_POLICY_PRESET_LABELS[preset]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </SettingsRow>

      <SettingsRow
        label="Custom rules"
        description="Allow or deny specific hosts for every sandbox. Applied as soon as you add or remove them."
      >
        {loadingNetworkPolicy ? (
          <p className="text-xs text-muted-foreground">Loading…</p>
        ) : (
          <NetworkRuleEditor
            rules={visibleRules}
            onAdd={onAddNetworkRule}
            onRemove={onRemoveNetworkRule}
            filterSlot={
              networkRules.length > 0 && (
                <div className="relative">
                  <Search
                    className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
                    strokeWidth={1.5}
                  />
                  <Input
                    className="pl-8"
                    placeholder="Search hosts"
                    value={ruleSearch}
                    onChange={(e) => setRuleSearch(e.target.value)}
                  />
                </div>
              )
            }
          />
        )}
      </SettingsRow>

      <Dialog open={confirmingPresetChange} onOpenChange={setConfirmingPresetChange}>
        <DialogContent title="Change network policy?">
          <Card className="w-full">
            <CardHeader>
              <CardTitle>Change network policy?</CardTitle>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
              <p className="text-sm text-muted-foreground">
                Changing the network policy resets sbx's network daemon and stops every currently
                running sandbox on this machine. This can't be undone.
              </p>
              <div className="flex justify-end gap-2">
                <Button variant="outline" onClick={() => setConfirmingPresetChange(false)}>
                  Cancel
                </Button>
                <Button variant="destructive" onClick={onConfirmNetworkPresetChange} disabled={savingNetwork}>
                  {savingNetwork ? 'Applying…' : 'Reset & apply'}
                </Button>
              </div>
            </CardContent>
          </Card>
        </DialogContent>
      </Dialog>
    </>
  )
}
