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
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function browseRepoPath() {
    const path = await open({ directory: true, multiple: false })
    if (typeof path === 'string') setRepoPath(path)
  }

  async function handleSave() {
    setSaving(true)
    setError(null)
    try {
      const args = { name, repoPath }
      if (initial) await invoke<Project>('update_project', { id: initial.id, ...args })
      else await invoke<Project>('create_project', args)
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
