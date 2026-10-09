import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { ChevronDown, ChevronRight, Loader2, Play, Plus, Trash2, X } from 'lucide-react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { formatRelativeTime } from '@/lib/sandboxDisplay'
import { cn } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'
import { MARKDOWN_CLASS } from './SandboxPlansTab'
import type { BrowserTest, BrowserTestStatus, ChromeTarget, Sandbox } from './types'

const DEFAULT_SANDBOX_PORT = 8080

// Owns its own fetch like SandboxBackupsTab, plus a live refresh from the
// backend's `browser-tests-changed` event.
export function SandboxBrowserTab({ sandbox }: { sandbox: Sandbox }) {
  const [tests, setTests] = useState<BrowserTest[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  async function load() {
    try {
      setTests(await invoke<BrowserTest[]>('list_browser_tests', { sandboxId: sandbox.id }))
      setError(null)
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    setLoading(true)
    load()
    const unlisten = listen<BrowserTest>('browser-tests-changed', (event) => {
      if (event.payload.sandbox_id === sandbox.id) load()
    })
    return () => {
      unlisten.then((fn) => fn())
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sandbox.id])

  async function act(command: string, id: string) {
    try {
      await invoke(command, { id })
      await load()
    } catch (e) {
      setError(String(e))
    }
  }

  return (
    <div className="flex flex-col gap-4 text-sm">
      <ChromeSettings sandbox={sandbox} />

      <div className="flex flex-col gap-2">
        <span className="text-xs font-medium text-foreground">Test runs</span>
        {error && <p className="text-xs text-destructive">{error}</p>}
        {loading ? (
          <Loader2 className="size-4 animate-spin text-muted-foreground" />
        ) : tests.length === 0 ? (
          <p className="text-xs text-muted-foreground">
            No tests yet. With Claude in Chrome on, ask the sandbox agent to browser-test its work (it has a
            <code className="mx-1">browser-test</code>skill for this).
          </p>
        ) : (
          tests.map((test) => <TestRow key={test.id} test={test} onAction={act} />)
        )}
      </div>
    </div>
  )
}

function ChromeSettings({ sandbox }: { sandbox: Sandbox }) {
  const [target, setTarget] = useState<ChromeTarget>(sandbox.chrome_target)
  const [port, setPort] = useState(sandbox.chrome_sandbox_port?.toString() ?? '')
  const [url, setUrl] = useState(sandbox.chrome_external_url ?? '')
  const [hostPrep, setHostPrep] = useState(sandbox.chrome_host_prep)
  const [commands, setCommands] = useState(() => initialCommands(sandbox))
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [saved, setSaved] = useState(false)

  useEffect(() => {
    setTarget(sandbox.chrome_target)
    setPort(sandbox.chrome_sandbox_port?.toString() ?? '')
    setUrl(sandbox.chrome_external_url ?? '')
    setHostPrep(sandbox.chrome_host_prep)
    setCommands(initialCommands(sandbox))
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sandbox.id])

  const dirty =
    target !== sandbox.chrome_target ||
    port !== (sandbox.chrome_sandbox_port?.toString() ?? '') ||
    url !== (sandbox.chrome_external_url ?? '') ||
    hostPrep !== sandbox.chrome_host_prep ||
    nonEmpty(commands).join('\n') !== sandbox.chrome_host_commands.join('\n')

  async function save(enabled: boolean) {
    setSaving(true)
    setError(null)
    setSaved(false)
    try {
      await invoke('save_sandbox_chrome_settings', {
        id: sandbox.id,
        enabled,
        target,
        sandboxPort: port.trim() ? Number(port) : null,
        externalUrl: url.trim() || null,
        hostPrep,
        hostCommands: nonEmpty(commands),
      })
      await useAppStore.getState().loadSandboxes()
      setSaved(true)
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="flex flex-col gap-3 rounded-lg border border-border p-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex flex-col">
          <span className="text-xs font-medium text-foreground">Claude in Chrome</span>
          <span className="text-xs text-muted-foreground">
            Let this sandbox's agent hand test plans to a Claude agent on this machine, which tests the app in your Chrome
            and sends a report back.
          </span>
        </div>
        <Switch checked={sandbox.chrome_enabled} disabled={saving} onCheckedChange={(on) => save(on)} />
      </div>

      <div className="flex flex-col gap-2">
        <span className="text-xs text-muted-foreground">What Chrome opens</span>
        <div className="flex gap-2">
          <TargetOption
            selected={target === 'sandbox'}
            onSelect={() => setTarget('sandbox')}
            title="App in this sandbox"
            body="The dev server the agent runs inside the sandbox, forwarded to this machine."
          />
          <TargetOption
            selected={target === 'external'}
            onSelect={() => setTarget('external')}
            title="Server on this machine"
            body="Your own checkout of the sandbox's branch, served from this machine. You or Overnight start the servers."
          />
        </div>
      </div>

      {target === 'sandbox' ? (
        <label className="flex items-center gap-2 text-xs text-muted-foreground">
          App port inside the sandbox
          <Input
            type="number"
            min={1}
            max={65535}
            className="max-w-28"
            placeholder={String(DEFAULT_SANDBOX_PORT)}
            value={port}
            onChange={(e) => setPort(e.target.value)}
          />
        </label>
      ) : (
        <>
          <label className="flex items-center gap-2 text-xs text-muted-foreground">
            URL
            <Input className="max-w-80" placeholder="http://localhost:3000" value={url} onChange={(e) => setUrl(e.target.value)} />
          </label>
          <HostPrepSettings sandbox={sandbox} enabled={hostPrep} setEnabled={setHostPrep} commands={commands} setCommands={setCommands} />
        </>
      )}

      <div className="flex items-center gap-2">
        <Button size="sm" variant="outline" className="w-fit" disabled={!dirty || saving} onClick={() => save(sandbox.chrome_enabled)}>
          {saving && <Loader2 className="size-3.5 animate-spin" />}
          Save
        </Button>
        {saved && !dirty && <span className="text-xs text-muted-foreground">Saved</span>}
      </div>

      {sandbox.chrome_enabled && sandbox.status !== 'running' && (
        <p className="text-xs text-muted-foreground">The sandbox is stopped; it gets set up for browser testing when it starts.</p>
      )}
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  )
}

function HostPrepSettings({
  sandbox,
  enabled,
  setEnabled,
  commands,
  setCommands,
}: {
  sandbox: Sandbox
  enabled: boolean
  setEnabled: (on: boolean) => void
  commands: string[]
  setCommands: (commands: string[]) => void
}) {
  const update = (index: number, value: string) => setCommands(commands.map((c, i) => (i === index ? value : c)))
  const remove = (index: number) => {
    const rest = commands.filter((_, i) => i !== index)
    setCommands(rest.length ? rest : [''])
  }

  return (
    <div className="flex flex-col gap-2 rounded-md bg-muted/40 p-2">
      <div className="flex items-center justify-between gap-2">
        <div className="flex flex-col">
          <span className="text-xs font-medium text-foreground">Prepare this machine automatically</span>
          <span className="text-xs text-muted-foreground">
            {sandbox.mode === 'clone'
              ? 'Before each test, Overnight fetches the branch from the sandbox and checks it out in the project folder, runs the commands below, and waits for the URL. Afterwards it stops the servers and switches back to your branch. It won’t touch a folder with uncommitted changes.'
              : 'This sandbox is mounted on the project folder, so its branch is already checked out here. Before each test, Overnight runs the commands below and waits for the URL, then stops the servers afterwards.'}
          </span>
        </div>
        <Switch checked={enabled} onCheckedChange={setEnabled} />
      </div>
      {enabled ? (
        <div className="flex flex-col gap-1.5">
          <span className="text-xs text-muted-foreground">
            Server commands. Each one runs in its own shell from the project folder, and all of them run at once.
          </span>
          {commands.map((command, i) => (
            <div key={i} className="flex items-center gap-2">
              <span className="w-16 shrink-0 text-xs text-muted-foreground">Server {i + 1}</span>
              <Input
                className="h-8 font-mono text-xs"
                spellCheck={false}
                placeholder={i === 0 ? 'pnpm dev' : 'cd api && pnpm start'}
                value={command}
                onChange={(e) => update(i, e.target.value)}
              />
              <Button
                size="sm"
                variant="ghost"
                title="Remove this server"
                disabled={commands.length === 1 && !command}
                onClick={() => remove(i)}
              >
                <X className="size-3.5" />
              </Button>
            </div>
          ))}
          <Button size="sm" variant="outline" className="w-fit" onClick={() => setCommands([...commands, ''])}>
            <Plus className="size-3.5" />
            Add server
          </Button>
        </div>
      ) : (
        <span className="text-xs text-muted-foreground">Off: each test waits for you to start the servers and press Start.</span>
      )}
    </div>
  )
}

// Always at least one (possibly empty) row to type into.
function initialCommands(sandbox: Sandbox): string[] {
  return sandbox.chrome_host_commands.length ? [...sandbox.chrome_host_commands] : ['']
}

function nonEmpty(commands: string[]): string[] {
  return commands.map((c) => c.trim()).filter(Boolean)
}

function TargetOption({ selected, onSelect, title, body }: { selected: boolean; onSelect: () => void; title: string; body: string }) {
  return (
    <button
      type="button"
      onClick={onSelect}
      aria-pressed={selected}
      className={cn(
        'flex flex-1 flex-col gap-0.5 rounded-md border p-2 text-left transition-colors',
        selected ? 'border-primary bg-primary/5' : 'border-border hover:bg-muted/50',
      )}
    >
      <span className="text-xs font-medium text-foreground">{title}</span>
      <span className="text-xs text-muted-foreground">{body}</span>
    </button>
  )
}

const STATUS_LABEL: Record<BrowserTestStatus, string> = {
  awaiting_host: 'Waiting for you',
  queued: 'Queued',
  running: 'Testing',
  done: 'Done',
  failed: 'Failed',
  cancelled: 'Cancelled',
}

function TestRow({ test, onAction }: { test: BrowserTest; onAction: (command: string, id: string) => void }) {
  const [open, setOpen] = useState(test.status === 'awaiting_host')
  // null = show the report once there is one, else the plan.
  const [chosenView, setView] = useState<'report' | 'plan' | null>(null)
  const view = chosenView ?? (test.report ? 'report' : 'plan')
  const pending = test.status === 'awaiting_host' || test.status === 'queued' || test.status === 'running'

  return (
    <div className={cn('flex flex-col rounded-lg border border-border', test.status === 'awaiting_host' && 'ring-2 ring-primary/40')}>
      <div className="flex items-center gap-2 p-2">
        <button type="button" onClick={() => setOpen(!open)} className="flex min-w-0 flex-1 items-center gap-2 text-left">
          {open ? <ChevronDown className="size-3.5 shrink-0" /> : <ChevronRight className="size-3.5 shrink-0" />}
          <Badge variant={test.status === 'failed' ? 'destructive' : test.status === 'done' ? 'secondary' : 'outline'}>
            {test.status === 'running' && <Loader2 className="size-3 animate-spin" />}
            {STATUS_LABEL[test.status]}
          </Badge>
          {test.verdict && (
            <Badge variant={test.verdict === 'pass' ? 'default' : 'destructive'}>{test.verdict.toUpperCase()}</Badge>
          )}
          <span className="truncate text-xs text-muted-foreground">
            {test.status === 'running' && test.progress ? `${test.progress} · ` : ''}
            {test.branch ? `${test.branch} · ` : ''}
            {formatRelativeTime(test.created_at)}
            {test.target_url ? ` · ${test.target_url}` : ''}
          </span>
        </button>
        {test.status === 'awaiting_host' && (
          <Button size="sm" onClick={() => onAction('start_browser_test', test.id)}>
            <Play className="size-3.5" />
            Start
          </Button>
        )}
        {pending ? (
          <Button size="sm" variant="ghost" onClick={() => onAction('cancel_browser_test', test.id)}>
            <X className="size-3.5" />
            Cancel
          </Button>
        ) : (
          <Button size="sm" variant="ghost" title="Delete this test" onClick={() => onAction('delete_browser_test', test.id)}>
            <Trash2 className="size-3.5" />
          </Button>
        )}
      </div>

      {open && (
        <div className="flex flex-col gap-3 border-t border-border p-3">
          {test.status === 'awaiting_host' && (
            <p className="text-xs text-foreground">
              Check out {test.branch ? <code>{test.branch}</code> : 'the sandbox’s branch'} on this machine, start its
              servers, then press Start.
            </p>
          )}
          {test.error && <p className="text-xs text-destructive">{test.error}</p>}
          <div className="flex gap-3 text-xs">
            {test.report && (
              <button type="button" onClick={() => setView('report')} className={cn(view === 'report' ? 'font-medium text-foreground' : 'text-muted-foreground')}>
                Report
              </button>
            )}
            <button type="button" onClick={() => setView('plan')} className={cn(view === 'plan' ? 'font-medium text-foreground' : 'text-muted-foreground')}>
              Test plan
            </button>
          </div>
          <div className={MARKDOWN_CLASS}>
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{view === 'report' && test.report ? test.report : test.doc}</ReactMarkdown>
          </div>
        </div>
      )}
    </div>
  )
}
