import { useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import CodeMirror from '@uiw/react-codemirror'
import { yaml } from '@codemirror/lang-yaml'
import { Plus, Trash2, Upload } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { mergeFiles, parseHooks, styleName, type AgentConfig, type NamedFile } from '@/lib/agentConfig'
import { SettingsActions, SettingsSection } from '../SettingsLayout'

const extensions = [yaml()]
const NO_STYLE = '__default__'

function FileList({
  files,
  selected,
  onSelect,
  onRemove,
}: {
  files: NamedFile[]
  selected: string | null
  onSelect: (name: string) => void
  onRemove: (name: string) => void
}) {
  if (files.length === 0) return <p className="text-xs text-muted-foreground">None yet.</p>
  return (
    <ul className="flex flex-col gap-1">
      {files.map((f) => (
        <li
          key={f.name}
          className={`flex items-center justify-between gap-2 rounded-md border px-2 py-1 text-sm ${f.name === selected ? 'border-primary' : 'border-border'}`}
        >
          <button type="button" className="flex-1 truncate text-left" onClick={() => onSelect(f.name)}>
            {f.name}
          </button>
          <Button size="icon-sm" variant="ghost" aria-label={`Remove ${f.name}`} onClick={() => onRemove(f.name)}>
            <Trash2 className="size-3.5" />
          </Button>
        </li>
      ))}
    </ul>
  )
}

function FileEditor({ file, onChange }: { file: NamedFile; onChange: (content: string) => void }) {
  const isDark = document.documentElement.classList.contains('dark')
  return (
    <CodeMirror
      value={file.content}
      height="200px"
      theme={isDark ? 'dark' : 'light'}
      basicSetup={{ lineNumbers: true, foldGutter: false, tabSize: 2 }}
      onChange={onChange}
      className="overflow-hidden rounded-lg border border-border font-mono text-sm"
    />
  )
}

export function AgentConfigTab() {
  const [saved, setSaved] = useState<AgentConfig | null>(null)
  const [hooksText, setHooksText] = useState('')
  const [hookFiles, setHookFiles] = useState<NamedFile[]>([])
  const [styles, setStyles] = useState<NamedFile[]>([])
  const [outputStyle, setOutputStyle] = useState<string | null>(null)
  const [selectedHook, setSelectedHook] = useState<string | null>(null)
  const [selectedStyle, setSelectedStyle] = useState<string | null>(null)
  const [newStyleName, setNewStyleName] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const isDark = document.documentElement.classList.contains('dark')

  function reset(config: AgentConfig) {
    setHooksText(Object.keys(config.hooks).length ? JSON.stringify(config.hooks, null, 2) : '')
    setHookFiles(config.hook_files)
    setStyles(config.output_styles)
    setOutputStyle(config.output_style)
  }

  useEffect(() => {
    invoke<AgentConfig>('get_agent_config')
      .then((config) => {
        setSaved(config)
        reset(config)
      })
      .catch((e) => setError(String(e)))
  }, [])

  const parsed = useMemo(() => parseHooks(hooksText), [hooksText])
  const current: AgentConfig | null = parsed.hooks
    ? { hooks: parsed.hooks, hook_files: hookFiles, output_styles: styles, output_style: outputStyle }
    : null
  const dirty = saved != null && (current == null || JSON.stringify(current) !== JSON.stringify(saved))

  async function importFiles(kind: 'hooks' | 'styles') {
    const selection = await open({
      multiple: true,
      filters: kind === 'styles' ? [{ name: 'Markdown', extensions: ['md'] }] : undefined,
    })
    const paths = Array.isArray(selection) ? selection : typeof selection === 'string' ? [selection] : []
    if (paths.length === 0) return
    try {
      const files = await invoke<NamedFile[]>('import_agent_files', { paths })
      if (kind === 'hooks') {
        setHookFiles((prev) => mergeFiles(prev, files))
        setSelectedHook(files[0]?.name ?? null)
      } else {
        const named = files.map((f) => ({ ...f, name: styleName(f.name) }))
        setStyles((prev) => mergeFiles(prev, named))
        setSelectedStyle(named[0]?.name ?? null)
      }
    } catch (e) {
      setError(String(e))
    }
  }

  function addStyle() {
    const name = newStyleName.trim()
    if (!name || /[\\/]/.test(name)) return
    setStyles((prev) => mergeFiles(prev, [{ name, content: `---\nname: ${name}\ndescription: \n---\n\n` }]))
    setSelectedStyle(name)
    setNewStyleName('')
  }

  function removeStyle(name: string) {
    setStyles((prev) => prev.filter((s) => s.name !== name))
    if (outputStyle === name) setOutputStyle(null)
    if (selectedStyle === name) setSelectedStyle(null)
  }

  async function save() {
    if (!current) return
    setSaving(true)
    setError(null)
    try {
      await invoke('save_agent_config', { config: current })
      setSaved(current)
    } catch (e) {
      setError(String(e))
      // The config is saved even when pushing to a sandbox fails.
      if (String(e).startsWith('saved,')) setSaved(current)
    } finally {
      setSaving(false)
    }
  }

  const hookFile = hookFiles.find((f) => f.name === selectedHook)
  const styleFile = styles.find((f) => f.name === selectedStyle)

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-auto px-6 pt-4 pb-6">
      <div className="flex items-start justify-between gap-4">
        <p className="text-sm text-muted-foreground">
          Claude hooks and output styles. Pushed into every new Claude sandbox and into running ones when saved.
        </p>
        <SettingsActions
          dirty={dirty}
          canSave={dirty && current != null}
          saving={saving}
          error={error}
          onSave={save}
          onCancel={() => {
            if (saved) reset(saved)
          }}
        />
      </div>
      {error && <p className="text-sm break-all text-destructive">{error}</p>}

      <SettingsSection title="Hooks" divider>
        <div className="flex flex-col gap-2 py-4">
          <span className="text-xs text-muted-foreground/70">settings.json "hooks" block (JSON)</span>
          <CodeMirror
            value={hooksText}
            height="200px"
            theme={isDark ? 'dark' : 'light'}
            extensions={extensions}
            placeholder={'{\n  "PreToolUse": [\n    { "matcher": "Bash", "hooks": [{ "type": "command", "command": "~/.claude/hooks/guard.sh" }] }\n  ]\n}'}
            basicSetup={{ lineNumbers: true, foldGutter: true, tabSize: 2 }}
            onChange={setHooksText}
            className="overflow-hidden rounded-lg border border-border font-mono text-sm"
          />
          {parsed.error && <p className="font-mono text-xs text-destructive">{parsed.error}</p>}
        </div>
        <div className="flex flex-col gap-2 py-4">
          <div className="flex items-center justify-between">
            <span className="text-xs text-muted-foreground/70">Hook scripts (copied to ~/.claude/hooks/)</span>
            <Button size="sm" variant="outline" onClick={() => importFiles('hooks')}>
              <Upload className="size-3.5" />
              Upload scripts
            </Button>
          </div>
          <FileList
            files={hookFiles}
            selected={selectedHook}
            onSelect={setSelectedHook}
            onRemove={(name) => {
              setHookFiles((prev) => prev.filter((f) => f.name !== name))
              if (selectedHook === name) setSelectedHook(null)
            }}
          />
          {hookFile && (
            <FileEditor
              file={hookFile}
              onChange={(content) => setHookFiles((prev) => prev.map((f) => (f.name === hookFile.name ? { ...f, content } : f)))}
            />
          )}
        </div>
      </SettingsSection>

      <SettingsSection title="Output styles">
        <div className="flex flex-col gap-2 py-4">
          <span className="text-xs text-muted-foreground/70">Default output style</span>
          <Select value={outputStyle ?? NO_STYLE} onValueChange={(v) => setOutputStyle(v === NO_STYLE ? null : v)}>
            <SelectTrigger className="w-64">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={NO_STYLE}>Claude default</SelectItem>
              {['Explanatory', 'Learning'].map((name) => (
                <SelectItem key={name} value={name}>
                  {name} (built-in)
                </SelectItem>
              ))}
              {styles.map((s) => (
                <SelectItem key={s.name} value={s.name}>
                  {s.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="flex flex-col gap-2 py-4">
          <div className="flex items-center justify-between gap-2">
            <span className="text-xs text-muted-foreground/70">Custom styles (copied to ~/.claude/output-styles/)</span>
            <Button size="sm" variant="outline" onClick={() => importFiles('styles')}>
              <Upload className="size-3.5" />
              Upload .md
            </Button>
          </div>
          <div className="flex gap-2">
            <Input
              className="h-8 max-w-64"
              placeholder="new style name"
              value={newStyleName}
              onChange={(e) => setNewStyleName(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && addStyle()}
            />
            <Button size="sm" variant="outline" onClick={addStyle} disabled={!newStyleName.trim()}>
              <Plus className="size-3.5" />
              New
            </Button>
          </div>
          <FileList files={styles} selected={selectedStyle} onSelect={setSelectedStyle} onRemove={removeStyle} />
          {styleFile && (
            <FileEditor
              file={styleFile}
              onChange={(content) => setStyles((prev) => prev.map((f) => (f.name === styleFile.name ? { ...f, content } : f)))}
            />
          )}
        </div>
      </SettingsSection>
    </div>
  )
}
