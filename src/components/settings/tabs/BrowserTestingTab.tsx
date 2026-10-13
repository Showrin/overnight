import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { ExternalLink } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'
import { SettingsRow } from '../SettingsLayout'

interface ChromeStatus {
  chrome_path: string | null
  chrome_path_override: string | null
  profile_dir: string
  profile_exists: boolean
  running: boolean
}

// Self-contained like KitsTab: no footer Save, every change applies at once.
// Polls so "Running" tracks the testing Chrome being opened or closed.
export function BrowserTestingTab() {
  const [status, setStatus] = useState<ChromeStatus | null>(null)
  const [pathDraft, setPathDraft] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  async function refresh() {
    try {
      setStatus(await invoke<ChromeStatus>('get_testing_chrome_status'))
    } catch (e) {
      setError(String(e))
    }
  }

  useEffect(() => {
    refresh()
    const timer = setInterval(refresh, 5000)
    return () => clearInterval(timer)
  }, [])

  async function open(setup: boolean) {
    setError(null)
    try {
      await invoke('open_testing_chrome', { setup })
      setTimeout(refresh, 1500)
    } catch (e) {
      setError(String(e))
    }
  }

  async function savePath() {
    if (pathDraft === null || pathDraft === (status?.chrome_path_override ?? '')) return
    setError(null)
    try {
      await invoke('save_testing_chrome_path', { path: pathDraft.trim() || null })
      setPathDraft(null)
      await refresh()
    } catch (e) {
      setError(String(e))
    }
  }

  if (!status) return null

  return (
    <>
      {error && <p className="pt-4 text-sm text-destructive">{error}</p>}

      <SettingsRow
        label="Testing browser"
        description="Browser tests run in a separate Chrome with its own profile, never your everyday one."
      >
        <div className="flex items-center gap-2 text-xs">
          <span className={cn('size-2 rounded-full', status.running ? 'bg-emerald-500' : 'bg-muted-foreground/40')} />
          <span className="text-foreground">
            {!status.profile_exists ? 'Not set up' : status.running ? 'Running' : 'Not running (starts on the next test)'}
          </span>
        </div>
        <div className="flex gap-2">
          {status.profile_exists ? (
            <>
              <Button size="sm" variant="outline" disabled={!status.chrome_path} onClick={() => open(false)}>
                <ExternalLink className="size-3.5" />
                Open
              </Button>
              <Button size="sm" variant="ghost" disabled={!status.chrome_path} onClick={() => open(true)}>
                Redo setup
              </Button>
            </>
          ) : (
            <Button size="sm" disabled={!status.chrome_path} onClick={() => open(true)}>
              <ExternalLink className="size-3.5" />
              Set up
            </Button>
          )}
        </div>
        <span className="truncate text-xs text-muted-foreground" title={status.profile_dir}>
          Profile: {status.profile_dir}
        </span>
      </SettingsRow>

      <SettingsRow label="Setup" description="One time, in the window Set up opens.">
        <ol className="flex list-decimal flex-col gap-1 pl-4 text-xs text-muted-foreground">
          <li>Install the Claude extension from the Chrome Web Store tab.</li>
          <li>Sign in to claude.ai with the same account Claude Code uses on this machine.</li>
          <li>Sign in to any test accounts your apps need. They stay in this profile only.</li>
          <li>
            Remove or turn off the Claude extension in your everyday Chrome profile, so this window is the only one Claude
            can connect to.
          </li>
        </ol>
      </SettingsRow>

      <SettingsRow
        label="Chrome path"
        description={status.chrome_path ? `Using ${status.chrome_path}` : 'Chrome wasn’t found. Enter the path to chrome.exe.'}
        divider={false}
      >
        <Input
          className="max-w-xl"
          placeholder={status.chrome_path ?? 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe'}
          value={pathDraft ?? status.chrome_path_override ?? ''}
          onChange={(e) => setPathDraft(e.target.value)}
          onBlur={savePath}
        />
      </SettingsRow>
    </>
  )
}
