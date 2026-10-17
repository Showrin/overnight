import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { parsePorts } from '@/lib/browserTargets'
import type { Project } from './types'

export function ProjectForm({
  initial,
  onSaved,
  onCancel,
}: {
  initial: Project | null
  onSaved: () => void
  onCancel: () => void
}) {
  const [name, setName] = useState(initial?.name ?? '')
  const [repoPath, setRepoPath] = useState(initial?.repo_path ?? '')
  const [plansPath, setPlansPath] = useState(initial?.plans_path ?? '')
  const [devServerPort, setDevServerPort] = useState(
    initial?.dev_server_port != null ? String(initial.dev_server_port) : ''
  )
  const [ports, setPorts] = useState((initial?.ports ?? []).join(', '))
  const [browserUrls, setBrowserUrls] = useState((initial?.browser_urls ?? []).join('\n'))
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function browseRepoPath() {
    const path = await open({ directory: true, multiple: false })
    if (typeof path === 'string') setRepoPath(path)
  }

  async function browsePlansPath() {
    const path = await open({ directory: true, multiple: false })
    if (typeof path === 'string') setPlansPath(path)
  }

  async function handleSave() {
    const parsedPorts = parsePorts(ports)
    if (parsedPorts == null) {
      setError('Ports must be numbers between 1 and 65535, separated by commas.')
      return
    }
    setSaving(true)
    setError(null)
    try {
      const args = {
        name,
        repoPath,
        plansPath: plansPath.trim() ? plansPath : null,
        devServerPort: devServerPort.trim() ? Number(devServerPort) : null,
      }
      const saved = initial
        ? await invoke<Project>('update_project', { id: initial.id, ...args })
        : await invoke<Project>('create_project', args)
      await invoke('save_project_browser_config', {
        id: saved.id,
        ports: parsedPorts,
        browserUrls: browserUrls.split('\n').map((u) => u.trim()).filter((u) => u.length > 0),
      })
      onSaved()
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }

  return (
    <Card className="w-full">
      <CardHeader>
        <CardTitle>{initial ? 'Edit project' : 'New project'}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <div className="flex flex-col gap-1">
          <Label htmlFor="project-name">Name</Label>
          <Input id="project-name" value={name} onChange={(e) => setName(e.target.value)} />
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="project-repo-path">Local repo path</Label>
          <div className="flex gap-2">
            <Input
              id="project-repo-path"
              placeholder="/path/to/repo"
              value={repoPath}
              onChange={(e) => setRepoPath(e.target.value)}
            />
            <Button type="button" variant="outline" onClick={browseRepoPath}>
              Browse
            </Button>
          </div>
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="project-plans-path">Plans path (optional)</Label>
          <div className="flex gap-2">
            <Input
              id="project-plans-path"
              placeholder="defaults to .agent/plans under repo path"
              value={plansPath}
              onChange={(e) => setPlansPath(e.target.value)}
            />
            <Button type="button" variant="outline" onClick={browsePlansPath}>
              Browse
            </Button>
          </div>
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="project-port">Dev server port (optional)</Label>
          <Input
            id="project-port"
            type="number"
            value={devServerPort}
            onChange={(e) => setDevServerPort(e.target.value)}
          />
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="project-ports">Sandbox ports (optional)</Label>
          <Input
            id="project-ports"
            placeholder="e.g. 3000, 5173 (defaults to dev server port or 8080)"
            value={ports}
            onChange={(e) => setPorts(e.target.value)}
          />
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="project-browser-urls">Browser URLs (optional, one per line)</Label>
          <textarea
            id="project-browser-urls"
            rows={3}
            placeholder="http://app.localhost:{port}/admin"
            value={browserUrls}
            onChange={(e) => setBrowserUrls(e.target.value)}
            className="rounded-md border border-input bg-transparent px-3 py-2 font-mono text-xs outline-none focus-visible:border-ring"
          />
          <p className="text-xs text-muted-foreground">{'{port}'} is replaced with the first published host port.</p>
        </div>
        {error && <p className="text-sm text-destructive">{error}</p>}
        <div className="flex gap-2">
          <Button className="flex-1" onClick={handleSave} disabled={saving || !name || !repoPath}>
            {saving ? 'Saving…' : 'Save'}
          </Button>
          <Button variant="outline" onClick={onCancel} disabled={saving}>
            Cancel
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
