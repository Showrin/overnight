import { useState, type ReactNode } from 'react'
import { Check, ChevronDown, GitBranch, Loader2, Search } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { cn } from '@/lib/utils'
import type { HostBranches } from './useHostBranches'

function Option({ selected, onSelect, children }: { selected: boolean; onSelect: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent"
    >
      <Check className={cn('size-3.5 shrink-0 text-primary', !selected && 'invisible')} />
      {children}
    </button>
  )
}

/**
 * Picks which host branch one sandbox branch is diffed against. `value`
 * is the user's override (null = the sandbox's own base branch).
 */
export function BaseBranchPicker({
  hostBranches: { branches, error, reload },
  defaultBase,
  value,
  onChange,
}: {
  hostBranches: HostBranches
  defaultBase: string | null
  value: string | null
  onChange: (base: string | null) => void
}) {
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')

  const effective = value ?? defaultBase
  const q = query.trim().toLowerCase()
  const matches = (branches ?? []).filter((b) => b !== defaultBase && b.toLowerCase().includes(q))
  const showDefault = defaultBase !== null && defaultBase.toLowerCase().includes(q)

  function pick(base: string | null) {
    onChange(base === defaultBase ? null : base)
    setOpen(false)
    setQuery('')
  }

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (next) reload()
      }}
    >
      <PopoverTrigger asChild>
        <Button size="sm" variant="outline" className="max-w-56" title="Compare against">
          <GitBranch className="size-3.5" />
          <span className="truncate font-mono">{effective ?? 'Pick base branch'}</span>
          <ChevronDown className="size-3.5 text-muted-foreground" />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="flex w-72 flex-col gap-1 p-1.5">
        <div className="relative">
          <Search
            className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
            strokeWidth={1.5}
          />
          <Input
            autoFocus
            className="h-8 pl-8"
            placeholder="Search host branches"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <div className="flex max-h-64 flex-col overflow-auto">
          {showDefault && (
            <Option selected={value === null} onSelect={() => pick(null)}>
              <span className="truncate font-mono">{defaultBase}</span>
              <span className="ml-auto shrink-0 text-xs text-muted-foreground">sandbox base</span>
            </Option>
          )}
          {matches.map((branch) => (
            <Option key={branch} selected={value === branch} onSelect={() => pick(branch)}>
              <span className="truncate font-mono">{branch}</span>
            </Option>
          ))}
          {!branches && !error && (
            <span className="flex items-center gap-2 px-2 py-1.5 text-xs text-muted-foreground">
              <Loader2 className="size-3.5 animate-spin" />
              Loading branches…
            </span>
          )}
          {error && <span className="px-2 py-1.5 text-xs text-destructive">{error}</span>}
          {branches && !showDefault && matches.length === 0 && (
            <span className="px-2 py-1.5 text-xs text-muted-foreground">No branches match "{query}".</span>
          )}
        </div>
      </PopoverContent>
    </Popover>
  )
}
