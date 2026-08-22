import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
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
  const [extraClonePaths, setExtraClonePaths] = useState<string[]>(initial?.extra_clone_paths ?? [])
  const [newPattern, setNewPattern] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  function addPattern() {
    const pattern = newPattern.trim()
    if (!pattern) return
    setExtraClonePaths((paths) => [...paths, pattern])
    setNewPattern('')
  }

  function removePattern(index: number) {
    setExtraClonePaths((paths) => paths.filter((_, i) => i !== index))
  }

  async function browseRepoPath() {
    const path = await open({ directory: true, multiple: false })
    if (typeof path === 'string') setRepoPath(path)
  }

  async function browsePlansPath() {
    const path = await open({ directory: true, multiple: false })
    if (typeof path === 'string') setPlansPath(path)
  }

  async function browseClonePath(kind: 'file' | 'folder') {
    const selection = await open({ directory: kind === 'folder', multiple: true })
    if (!selection) return
    const paths = Array.isArray(selection) ? selection : [selection]
    if (paths.length === 0) return
    setExtraClonePaths((existing) => [...existing, ...paths])
  }

  async function handleSave() {
    setSaving(true)
    setError(null)
    try {
      const args = {
        name,
        repoPath,
        plansPath: plansPath.trim() ? plansPath : null,
        devServerPort: devServerPort.trim() ? Number(devServerPort) : null,
        extraClonePaths,
      }
      if (initial) {
        await invoke('update_project', { id: initial.id, ...args })
      } else {
        await invoke('create_project', args)
      }
      onSaved()
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }

  return (
    <Card className="w-96">
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
          <Label>Extra files/folders to clone (optional)</Label>
          <div className="flex gap-2">
            <Input
              placeholder="type a glob pattern, or browse below"
              value={newPattern}
              onChange={(e) => setNewPattern(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault()
                  addPattern()
                }
              }}
            />
            <Button type="button" variant="outline" onClick={addPattern}>
              Add
            </Button>
          </div>
          <div className="flex gap-2">
            <Button type="button" variant="outline" size="sm" onClick={() => browseClonePath('file')}>
              Browse file(s)
            </Button>
            <Button type="button" variant="outline" size="sm" onClick={() => browseClonePath('folder')}>
              Browse folder
            </Button>
          </div>
          {extraClonePaths.length > 0 && (
            <ul className="flex flex-col gap-1">
              {extraClonePaths.map((pattern, index) => (
                <li
                  key={`${pattern}-${index}`}
                  className="flex items-center justify-between gap-2 rounded-lg border border-border px-2.5 py-1 text-sm"
                >
                  <span className="truncate">{pattern}</span>
                  <button
                    type="button"
                    onClick={() => removePattern(index)}
                    className="text-muted-foreground hover:text-destructive"
                  >
                    Remove
                  </button>
                </li>
              ))}
            </ul>
          )}
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
