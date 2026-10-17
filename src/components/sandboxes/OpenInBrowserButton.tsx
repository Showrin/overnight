import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { ChevronDown, ExternalLink } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { browserTargets } from '@/lib/browserTargets'
import { useAppStore } from '@/store/useAppStore'
import type { Sandbox } from './types'

export function OpenInBrowserButton({ sandbox, onError }: { sandbox: Sandbox; onError: (message: string) => void }) {
  const project = useAppStore((s) => s.projects.find((p) => p.id === sandbox.project_id))
  const [menuOpen, setMenuOpen] = useState(false)
  const targets = browserTargets(sandbox.ports ?? [], sandbox.host_port, project?.browser_urls ?? [])
  if (targets.length === 0) return null

  function open(url: string) {
    invoke('open_url', { url }).catch((e) => onError(String(e)))
  }

  return (
    <div className="inline-flex">
      <Button
        size="sm"
        variant="outline"
        className={targets.length > 1 ? 'rounded-r-none border-r-0' : undefined}
        onClick={() => open(targets[0].url)}
        title={targets[0].url}
      >
        <ExternalLink className="size-3.5" />
        Open in browser
      </Button>
      {targets.length > 1 && (
        <Popover open={menuOpen} onOpenChange={setMenuOpen}>
          <PopoverTrigger asChild>
            <Button size="sm" variant="outline" className="rounded-l-none px-1" aria-label="Choose URL">
              <ChevronDown className="size-3.5" />
            </Button>
          </PopoverTrigger>
          <PopoverContent align="end" className="flex flex-col gap-0.5">
            {targets.map((t) => (
              <button
                key={t.url}
                type="button"
                onClick={() => {
                  setMenuOpen(false)
                  open(t.url)
                }}
                className="w-full truncate rounded-md px-2 py-1.5 text-left text-sm outline-none hover:bg-accent hover:text-accent-foreground"
              >
                {t.label}
              </button>
            ))}
          </PopoverContent>
        </Popover>
      )}
    </div>
  )
}
