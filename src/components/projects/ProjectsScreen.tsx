import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { ProjectForm } from './ProjectForm'
import type { Project } from './types'

export function ProjectsScreen() {
  const [projects, setProjects] = useState<Project[]>([])
  const [editing, setEditing] = useState<Project | 'new' | null>(null)
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  async function loadProjects() {
    const list = await invoke<Project[]>('list_projects')
    setProjects(list)
  }

  useEffect(() => {
    loadProjects()
  }, [])

  async function handleDelete(id: string) {
    setError(null)
    try {
      await invoke('delete_project', { id })
      setConfirmingDeleteId(null)
      await loadProjects()
    } catch (e) {
      setError(String(e))
    }
  }

  if (editing) {
    return (
      <ProjectForm
        initial={editing === 'new' ? null : editing}
        onSaved={() => {
          setEditing(null)
          loadProjects()
        }}
        onCancel={() => setEditing(null)}
      />
    )
  }

  return (
    <div className="flex w-full flex-col gap-4">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-medium">Projects</h1>
        <Button size="sm" onClick={() => setEditing('new')}>
          New Project
        </Button>
      </div>
      {error && <p className="text-sm text-destructive">{error}</p>}
      {projects.length === 0 && (
        <p className="text-sm text-muted-foreground">No projects yet — click New Project to add one.</p>
      )}
      <div className="flex flex-col gap-2">
        {projects.map((project) => (
          <Card key={project.id}>
            <CardHeader className="flex flex-row items-center justify-between">
              <CardTitle>{project.name}</CardTitle>
              <div className="flex gap-2">
                <Button size="sm" variant="outline" onClick={() => setEditing(project)}>
                  Edit
                </Button>
                {confirmingDeleteId === project.id ? (
                  <Button size="sm" variant="destructive" onClick={() => handleDelete(project.id)}>
                    Confirm?
                  </Button>
                ) : (
                  <Button size="sm" variant="destructive" onClick={() => setConfirmingDeleteId(project.id)}>
                    Delete
                  </Button>
                )}
              </div>
            </CardHeader>
            <CardContent className="flex flex-col gap-1 text-sm text-muted-foreground">
              <span>{project.repo_path}</span>
              <div className="flex gap-2">
                {project.dev_server_port != null && (
                  <Badge variant="outline">port {project.dev_server_port}</Badge>
                )}
                {project.extra_clone_paths.length > 0 && (
                  <Badge variant="outline">{project.extra_clone_paths.length} clone pattern(s)</Badge>
                )}
              </div>
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  )
}
