import type { ReactNode } from 'react'
import { TriangleAlert } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

/**
 * Label + description on the left, controls on the right, divided by a
 * hairline. Stacks into one column below the `@2xl` container width.
 * `divider` defaults to true (a tab whose rows sit directly one after
 * another, like General/Backups/Network); pass `false` inside a
 * SettingsSection, which draws its own divider between sections instead
 * of one under every row.
 */
export function SettingsRow({
  label,
  description,
  divider = true,
  className,
  children,
}: {
  label: string
  description?: ReactNode
  divider?: boolean
  className?: string
  children: ReactNode
}) {
  return (
    <div className={cn('flex flex-col gap-3 py-5 @2xl:flex-row @2xl:gap-8', divider && 'border-b border-border', className)}>
      <div className="flex flex-col gap-1 @2xl:w-64 @2xl:shrink-0">
        <span className="text-sm font-medium text-foreground">{label}</span>
        {description && <span className="text-xs text-muted-foreground">{description}</span>}
      </div>
      <div className="flex min-w-0 flex-1 flex-col gap-2">{children}</div>
    </div>
  )
}

/**
 * Groups a set of rows under a heading — used to split a tab into named
 * subsections (e.g. "Environment Variables" vs "Secrets" on the
 * Credentials tab) without giving each its own Save/Cancel bar. Pass
 * `divider` on every section but the last one, so sections are separated
 * from each other but nothing trails after the final section.
 */
export function SettingsSection({ title, divider = false, children }: { title: string; divider?: boolean; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1 pt-6 first:pt-0">
      <h2 className="text-base font-semibold text-foreground">{title}</h2>
      <div className="flex flex-col">{children}</div>
      {divider && <div className="mt-6 border-t border-border" />}
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
