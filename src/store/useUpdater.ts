import { create } from 'zustand'
import { getVersion } from '@tauri-apps/api/app'
import { relaunch } from '@tauri-apps/plugin-process'
import { check, type Update } from '@tauri-apps/plugin-updater'

const RECHECK_INTERVAL_MS = 6 * 60 * 60 * 1000
const RELEASES_API = 'https://api.github.com/repos/Showrin/overnight/releases?per_page=100'

export type UpdateStatus = 'idle' | 'checking' | 'up-to-date' | 'available' | 'downloading' | 'installing' | 'error'

export interface ChangelogEntry {
  version: string
  date: string | null
  notes: string
}

interface UpdaterStore {
  currentVersion: string | null
  status: UpdateStatus
  update: Update | null
  // Every release between the installed version and the new one, newest
  // first — so someone skipping several versions sees all of them, not just
  // the latest release's notes.
  changelog: ChangelogEntry[]
  error: string | null
  lastCheckedAt: number | null
  downloaded: number
  contentLength: number | null
  toastDismissed: boolean
  changelogOpen: boolean
  checkForUpdate: (opts?: { silent?: boolean }) => Promise<void>
  installUpdate: () => Promise<void>
  dismissToast: () => void
  setChangelogOpen: (open: boolean) => void
}

function parseVersion(version: string): number[] {
  return version
    .replace(/^v/, '')
    .split(/[-+]/)[0]
    .split('.')
    .map((part) => Number(part) || 0)
}

function compareVersions(a: string, b: string): number {
  const [pa, pb] = [parseVersion(a), parseVersion(b)]
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const diff = (pa[i] ?? 0) - (pb[i] ?? 0)
    if (diff !== 0) return diff
  }
  return 0
}

// Falls back to the update's own notes (the latest release body, carried in
// latest.json) when GitHub's API is unreachable or rate limited.
async function fetchChangelog(current: string, update: Update): Promise<ChangelogEntry[]> {
  const fallback = [{ version: update.version, date: update.date ?? null, notes: update.body ?? '' }]
  try {
    const res = await fetch(RELEASES_API, { headers: { Accept: 'application/vnd.github+json' } })
    if (!res.ok) return fallback
    const releases: { tag_name: string; body: string | null; published_at: string | null; draft: boolean }[] =
      await res.json()
    const entries = releases
      .filter(
        (r) =>
          !r.draft && compareVersions(r.tag_name, current) > 0 && compareVersions(r.tag_name, update.version) <= 0
      )
      .sort((a, b) => compareVersions(b.tag_name, a.tag_name))
      .map((r) => ({ version: r.tag_name.replace(/^v/, ''), date: r.published_at, notes: r.body ?? '' }))
    return entries.length > 0 ? entries : fallback
  } catch {
    return fallback
  }
}

export const useUpdater = create<UpdaterStore>((set, get) => ({
  currentVersion: null,
  status: 'idle',
  update: null,
  changelog: [],
  error: null,
  lastCheckedAt: null,
  downloaded: 0,
  contentLength: null,
  toastDismissed: false,
  changelogOpen: false,

  // `silent` is for the background checks: a failure (offline, no release
  // published yet) leaves the status alone instead of surfacing an error.
  async checkForUpdate({ silent = false } = {}) {
    const { status } = get()
    if (status === 'checking' || status === 'downloading' || status === 'installing') return
    if (!silent) set({ status: 'checking', error: null })
    try {
      const currentVersion = get().currentVersion ?? (await getVersion())
      const update = await check()
      if (!update) {
        set({ currentVersion, status: 'up-to-date', update: null, changelog: [], lastCheckedAt: Date.now() })
        return
      }
      const isNew = get().update?.version !== update.version
      const changelog = await fetchChangelog(currentVersion, update)
      set({
        currentVersion,
        status: 'available',
        update,
        changelog,
        lastCheckedAt: Date.now(),
        toastDismissed: isNew ? false : get().toastDismissed,
      })
    } catch (e) {
      if (silent) return
      set({ status: 'error', error: String(e), lastCheckedAt: Date.now() })
    }
  },

  async installUpdate() {
    const { update } = get()
    if (!update) return
    set({ status: 'downloading', downloaded: 0, contentLength: null, error: null })
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') set({ contentLength: event.data.contentLength ?? null })
        else if (event.event === 'Progress') set((s) => ({ downloaded: s.downloaded + event.data.chunkLength }))
        else if (event.event === 'Finished') set({ status: 'installing' })
      })
      await relaunch()
    } catch (e) {
      set({ status: 'error', error: String(e) })
    }
  },

  dismissToast() {
    set({ toastDismissed: true })
  },

  setChangelogOpen(open) {
    set({ changelogOpen: open })
  },
}))

export function initUpdater() {
  getVersion().then((currentVersion) => useUpdater.setState({ currentVersion }))
  useUpdater.getState().checkForUpdate({ silent: true })
  setInterval(() => useUpdater.getState().checkForUpdate({ silent: true }), RECHECK_INTERVAL_MS)
}
