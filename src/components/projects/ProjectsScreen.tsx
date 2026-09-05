import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { statusBadgeVariant } from '@/lib/sandboxDisplay'
import { useAppStore } from '@/store/useAppStore'
import { ProjectForm } from './ProjectForm'
import type { Project } from './types'

export function ProjectsScreen() {
  const projects = useAppStore((s) => s.projects)
  const sandboxes = useAppStore((s) => s.sandboxes)
  const loadProjects = useAppStore((s) => s.loadProjects)
  const [editing, setEditing] = useState<Project | 'new' | null>(null)
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

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

  return (
    <div className="flex w-full flex-col gap-4">
      <Dialog open={editing != null} onOpenChange={(open) => !open && setEditing(null)}>
        <DialogContent title={editing === 'new' ? 'New project' : 'Edit project'}>
          {editing && (
            <ProjectForm
              initial={editing === 'new' ? null : editing}
              onSaved={() => {
                setEditing(null)
                loadProjects()
              }}
              onCancel={() => setEditing(null)}
            />
          )}
        </DialogContent>
      </Dialog>
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
        {projects.map((project) => {
          const projectSandboxes = sandboxes.filter((sb) => sb.project_id === project.id)
          return (
            <Card key={project.id}>
              <CardHeader className="flex flex-row items-center justify-between">
                <div className="flex flex-col">
                  <CardTitle>{project.name}</CardTitle>
                  <span className="text-xs text-muted-foreground">{project.repo_path}</span>
                </div>
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
              <CardContent className="flex flex-col gap-2 text-sm text-muted-foreground">
                <div className="flex gap-2">
                  {project.dev_server_port != null && (
                    <Badge variant="outline">port {project.dev_server_port}</Badge>
                  )}
                </div>
                {projectSandboxes.length > 0 && (
                  <div className="flex flex-col gap-1 border-t border-border-subtle pt-2">
                    {projectSandboxes.map((sandbox) => (
                      <div key={sandbox.id} className="flex items-center justify-between gap-2">
                        <span className="truncate text-xs">{sandbox.name ?? sandbox.sbx_name ?? sandbox.id}</span>
                        <Badge variant={statusBadgeVariant(sandbox.status)}>{sandbox.status}</Badge>
                      </div>
                    ))}
                  </div>
                )}
              </CardContent>
            </Card>
          )
        })}
      </div>
    </div>
  )
}
