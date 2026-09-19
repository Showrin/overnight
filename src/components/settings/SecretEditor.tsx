import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import {
  KNOWN_SECRET_SERVICES,
  type Secret,
  type SecretSource,
  type SecretTarget,
  isValidHost,
  isValidSecretEnv,
  parseHosts,
  secretKey,
} from '@/lib/secrets'
import { MaskedValueInput } from './MaskedValueInput'

type TargetKind = SecretTarget['kind']
type SourceKind = SecretSource['kind']

function describeTarget(target: SecretTarget): { label: string; sublabel?: string } {
  return target.kind === 'service' ? { label: target.service } : { label: target.env, sublabel: target.hosts.join(', ') }
}

function describeSource(source: SecretSource): string {
  switch (source.kind) {
    case 'value':
      return 'Value'
    case 'reference':
      return `Ref: ${source.reference}`
    case 'command':
      return `Command: ${source.command}`
  }
}

/**
 * Target/source row editor covering the full `sbx secret set` /
 * `set-custom` surface — add a row, remove a row. Purely controlled,
 * same contract as EnvVarEditor. A literal value is masked by default
 * (see MaskedValueInput); a reference or command isn't (they're pointers
 * to a secret, not the secret itself). `readOnly` hides the add/remove
 * controls — used for a scope the caller can view but not edit from here
 * (e.g. global secrets shown on a sandbox's own tab). Removing a row is
 * confirmed first: a secret already registered with a sandbox isn't
 * necessarily cleared there by the same removal call (see
 * commands.rs::remove_secrets_from_sbx's doc comment).
 */
export function SecretEditor({
  secrets,
  onChange,
  readOnly = false,
}: {
  secrets: Secret[]
  onChange: (secrets: Secret[]) => void
  readOnly?: boolean
}) {
  const [targetKind, setTargetKind] = useState<TargetKind>('custom')
  const [service, setService] = useState('')
  const [env, setEnv] = useState('')
  const [hostsInput, setHostsInput] = useState('')

  const [sourceKind, setSourceKind] = useState<SourceKind>('value')
  const [value, setValue] = useState('')
  const [reference, setReference] = useState('')
  const [command, setCommand] = useState('')
  const [useRefresh, setUseRefresh] = useState(false)
  const [refresh, setRefresh] = useState('')

  const [error, setError] = useState<string | null>(null)
  const [pendingRemoval, setPendingRemoval] = useState<string | null>(null)

  function buildSource(): SecretSource | null {
    if (sourceKind === 'value') {
      // A service secret's value is optional — an empty one means the
      // credential is already managed some other way (set directly via
      // sbx, OAuth, ...) and this entry exists only so the app can track
      // and remove it. A custom secret still needs a real value: nothing
      // else can supply it.
      if (targetKind === 'custom' && !value.trim()) return null
      return { kind: 'value', value }
    }
    const refreshValue = useRefresh && refresh.trim() ? refresh.trim() : null
    if (sourceKind === 'reference') {
      if (!reference.trim()) return null
      return { kind: 'reference', reference: reference.trim(), refresh: refreshValue }
    }
    if (!command.trim()) return null
    return { kind: 'command', command: command.trim(), refresh: refreshValue }
  }

  function buildTarget(): SecretTarget | null {
    if (targetKind === 'service') {
      return service ? { kind: 'service', service } : null
    }
    const trimmedEnv = env.trim()
    if (!trimmedEnv || !isValidSecretEnv(trimmedEnv)) return null
    const hosts = parseHosts(hostsInput)
    if (hosts.length === 0 || hosts.some((h) => !isValidHost(h))) return null
    return { kind: 'custom', env: trimmedEnv, hosts, placeholder: null }
  }

  function addRow() {
    const target = buildTarget()
    if (!target) {
      setError(
        targetKind === 'service'
          ? 'Choose a service.'
          : 'Enter a valid env var name and at least one host, with no whitespace.',
      )
      return
    }
    const source = buildSource()
    if (!source) {
      setError('Enter a value, reference, or command.')
      return
    }
    const key = target.kind === 'service' ? target.service : target.env
    if (secrets.some((s) => secretKey(s) === key)) {
      setError(`"${key}" is already set.`)
      return
    }
    setError(null)
    onChange([...secrets, { target, source }])
    setService('')
    setEnv('')
    setHostsInput('')
    setValue('')
    setReference('')
    setCommand('')
    setUseRefresh(false)
    setRefresh('')
  }

  function confirmRemoveRow() {
    if (!pendingRemoval) return
    onChange(secrets.filter((s) => secretKey(s) !== pendingRemoval))
    setPendingRemoval(null)
  }

  return (
    <div className="flex flex-col gap-3">
      {!readOnly && (
        <div className="flex flex-col gap-2 rounded-lg border border-border p-3">
          <div className="flex flex-wrap gap-2">
            <Select value={targetKind} onValueChange={(v) => setTargetKind(v as TargetKind)}>
              <SelectTrigger className="w-32">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="custom">Custom</SelectItem>
                <SelectItem value="service">Service</SelectItem>
              </SelectContent>
            </Select>

            {targetKind === 'service' ? (
              <Select value={service} onValueChange={setService}>
                <SelectTrigger className="w-40">
                  <SelectValue placeholder="Choose a service" />
                </SelectTrigger>
                <SelectContent>
                  {KNOWN_SECRET_SERVICES.map((s) => (
                    <SelectItem key={s} value={s}>
                      {s}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            ) : (
              <>
                <Input
                  className="h-8 min-w-0 flex-1 font-mono"
                  placeholder="ENV_VAR"
                  value={env}
                  onChange={(e) => setEnv(e.target.value)}
                />
                <Input
                  className="h-8 min-w-0 flex-1"
                  placeholder="host(s), comma-separated"
                  value={hostsInput}
                  onChange={(e) => setHostsInput(e.target.value)}
                />
              </>
            )}
          </div>

          <div className="flex items-center gap-2">
            <Select value={sourceKind} onValueChange={(v) => setSourceKind(v as SourceKind)}>
              <SelectTrigger className="w-32 shrink-0">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="value">Value</SelectItem>
                <SelectItem value="reference">Reference</SelectItem>
                <SelectItem value="command">Command</SelectItem>
              </SelectContent>
            </Select>

            {sourceKind === 'value' && (
              <Input
                className="h-8 min-w-0 flex-1"
                placeholder={targetKind === 'service' ? 'value (optional — leave blank if managed elsewhere)' : 'value'}
                type="password"
                value={value}
                onChange={(e) => setValue(e.target.value)}
              />
            )}
            {sourceKind === 'reference' && (
              <Input
                className="h-8 min-w-0 flex-1"
                placeholder="op://Work/Item/field or an AWS Secrets Manager ARN"
                value={reference}
                onChange={(e) => setReference(e.target.value)}
              />
            )}
            {sourceKind === 'command' && (
              <Input
                className="h-8 min-w-0 flex-1"
                placeholder="shell command"
                value={command}
                onChange={(e) => setCommand(e.target.value)}
              />
            )}
          </div>

          {sourceKind !== 'value' && (
            <div className="flex items-center gap-2">
              <label className="flex shrink-0 items-center gap-1.5 text-xs text-muted-foreground">
                <input type="checkbox" checked={useRefresh} onChange={(e) => setUseRefresh(e.target.checked)} />
                Cache for
              </label>
              {useRefresh && (
                <Input className="h-8 max-w-24" placeholder="30m" value={refresh} onChange={(e) => setRefresh(e.target.value)} />
              )}
            </div>
          )}

          <div className="flex justify-end">
            <Button size="sm" type="button" onClick={addRow}>
              Add
            </Button>
          </div>
        </div>
      )}
      {!readOnly && error && <p className="text-xs text-destructive">{error}</p>}

      {secrets.length === 0 ? (
        <p className="text-xs text-muted-foreground">No secrets set.</p>
      ) : (
        <ul className="flex flex-col gap-1.5">
          {secrets.map((s) => {
            const { label, sublabel } = describeTarget(s.target)
            const key = secretKey(s)
            return (
              <li key={key} className="flex items-center gap-2">
                <div className="flex w-40 shrink-0 flex-col">
                  <span className="truncate font-mono text-sm">{label}</span>
                  {sublabel && <span className="truncate text-xs text-muted-foreground/70">{sublabel}</span>}
                </div>
                {s.source.kind === 'value' ? (
                  <MaskedValueInput className="h-8" value={s.source.value} onChange={() => {}} readOnly />
                ) : (
                  <span className="min-w-0 flex-1 truncate text-sm text-muted-foreground">{describeSource(s.source)}</span>
                )}
                {!readOnly && (
                  <button
                    type="button"
                    onClick={() => setPendingRemoval(key)}
                    className="shrink-0 text-xs text-muted-foreground hover:text-destructive"
                  >
                    Remove
                  </button>
                )}
              </li>
            )
          })}
        </ul>
      )}

      <Dialog open={pendingRemoval !== null} onOpenChange={(open) => !open && setPendingRemoval(null)}>
        <DialogContent title="Remove secret?">
          <Card className="w-full">
            <CardHeader>
              <CardTitle>Remove secret?</CardTitle>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
              <p className="text-sm text-muted-foreground">
                Saving after this deletes <span className="font-mono">{pendingRemoval}</span> from the secret store, and from
                every sandbox that's running right now. A sandbox that's currently stopped keeps the old value — recreate it to
                be sure the secret is gone.
              </p>
              <div className="flex justify-end gap-2">
                <Button variant="outline" onClick={() => setPendingRemoval(null)}>
                  Cancel
                </Button>
                <Button variant="destructive" onClick={confirmRemoveRow}>
                  Remove
                </Button>
              </div>
            </CardContent>
          </Card>
        </DialogContent>
      </Dialog>
    </div>
  )
}
