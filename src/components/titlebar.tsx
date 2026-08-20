import { getCurrentWindow } from '@tauri-apps/api/window'
import { Minus, Square, X } from 'lucide-react'
import logoIcon from '@/assets/logo-icon.svg'
import { LogoWordmark } from '@/components/logo-wordmark'

const appWindow = getCurrentWindow()

export function Titlebar() {
  return (
    <header
      data-tauri-drag-region
      className="flex h-9 shrink-0 items-center justify-between border-b bg-background pl-3"
    >
      <div data-tauri-drag-region className="flex items-center gap-2">
        <img src={logoIcon} alt="" className="h-5 w-auto rounded-[5px]" />
        <LogoWordmark className="h-3 w-auto text-foreground" />
      </div>
      <div className="flex h-full items-stretch">
        <button
          type="button"
          aria-label="Minimize"
          onClick={() => appWindow.minimize()}
          className="flex w-11 items-center justify-center text-muted-foreground hover:bg-accent hover:text-accent-foreground"
        >
          <Minus className="size-3.5" />
        </button>
        <button
          type="button"
          aria-label="Maximize"
          onClick={() => appWindow.toggleMaximize()}
          className="flex w-11 items-center justify-center text-muted-foreground hover:bg-accent hover:text-accent-foreground"
        >
          <Square className="size-3" />
        </button>
        <button
          type="button"
          aria-label="Close"
          onClick={() => appWindow.close()}
          className="flex w-11 items-center justify-center text-muted-foreground hover:bg-destructive hover:text-white"
        >
          <X className="size-3.5" />
        </button>
      </div>
    </header>
  )
}
