import { useMemo, useRef, useState } from 'react'
import CodeMirror, { type ReactCodeMirrorRef } from '@uiw/react-codemirror'
import { yaml } from '@codemirror/lang-yaml'
import { redo, redoDepth, undo, undoDepth } from '@codemirror/commands'
import { Redo2, Undo2 } from 'lucide-react'
import { parse } from 'yaml'
import { Button } from '@/components/ui/button'

const extensions = [yaml()]

export function KitEditor({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  const ref = useRef<ReactCodeMirrorRef>(null)
  const [depth, setDepth] = useState({ undo: 0, redo: 0 })
  const isDark = document.documentElement.classList.contains('dark')

  const yamlError = useMemo(() => {
    try {
      parse(value)
      return null
    } catch (e) {
      return (e as Error).message
    }
  }, [value])

  function run(command: typeof undo) {
    const view = ref.current?.view
    if (view) command(view)
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      <div className="flex items-center gap-1">
        <Button variant="ghost" size="icon-sm" aria-label="Undo" disabled={depth.undo === 0} onClick={() => run(undo)}>
          <Undo2 className="size-4" strokeWidth={1.5} />
        </Button>
        <Button variant="ghost" size="icon-sm" aria-label="Redo" disabled={depth.redo === 0} onClick={() => run(redo)}>
          <Redo2 className="size-4" strokeWidth={1.5} />
        </Button>
        <span className="ml-1 text-xs text-muted-foreground">spec.yaml · Ctrl+Z / Ctrl+Shift+Z</span>
      </div>
      <CodeMirror
        ref={ref}
        value={value}
        height="100%"
        theme={isDark ? 'dark' : 'light'}
        extensions={extensions}
        basicSetup={{ lineNumbers: true, foldGutter: true, highlightActiveLine: true, tabSize: 2 }}
        onChange={onChange}
        onUpdate={(update) => {
          const next = { undo: undoDepth(update.state), redo: redoDepth(update.state) }
          setDepth((prev) => (prev.undo === next.undo && prev.redo === next.redo ? prev : next))
        }}
        className="min-h-64 flex-1 overflow-hidden rounded-lg border border-border font-mono text-sm"
      />
      {yamlError && <p className="font-mono text-xs text-destructive">{yamlError}</p>}
    </div>
  )
}
