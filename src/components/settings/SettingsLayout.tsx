import type { ReactNode } from 'react'
import { TriangleAlert } from 'lucide-react'
import { Button } from '@/components/ui/button'

/**
 * Label + description on the left, controls on the right, divided by a
 * hairline. Stacks into one column below the `@2xl` container width.
 */
export function SettingsRow({
  label,
  description,
  children,
}: {
  label: string
  description?: ReactNode
  children: ReactNode
}) {
  return (
    <div className="flex flex-col gap-3 border-b border-border py-5 @2xl:flex-row @2xl:gap-8">
      <div className="flex flex-col gap-1 @2xl:w-64 @2xl:shrink-0">
        <span className="text-sm font-medium text-foreground">{label}</span>
        {description && <span className="text-xs text-muted-foreground">{description}</span>}
      </div>
      <div className="flex min-w-0 flex-1 flex-col gap-2">{children}</div>
    </div>
  )
}

export interface TabActions {
  dirty: boolean
  canSave: boolean
  saving: boolean
  error: string | null
  onSave: () => void
  onCancel: () => void
}

export function SettingsActions({ dirty, canSave, saving, onSave, onCancel }: TabActions) {
  return (
    <div className="flex shrink-0 items-center gap-2">
      <Button variant="outline" size="sm" onClick={onCancel} disabled={!dirty || saving}>
        Cancel
      </Button>
      <Button size="sm" onClick={onSave} disabled={!canSave || saving}>
        {saving ? 'Saving…' : 'Save changes'}
      </Button>
    </div>
  )
}

export function UnsavedChangesNotice() {
  return (
    <div className="flex items-center gap-2 rounded-lg border border-warning/40 bg-warning/10 px-3 py-2 text-xs text-foreground">
      <TriangleAlert className="size-3.5 shrink-0 text-warning" strokeWidth={1.5} />
      You have unsaved changes.
    </div>
  )
}
