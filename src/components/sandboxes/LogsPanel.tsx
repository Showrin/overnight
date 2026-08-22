import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

interface SandboxLogLine {
  sandbox_id: string
  line: string
}

export function LogsPanel({ sandboxId, onClose }: { sandboxId: string; onClose: () => void }) {
  const [lines, setLines] = useState<string[]>([])
  const [error, setError] = useState<string | null>(null)
  const bottomRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    let unlisten: (() => void) | undefined
    ;(async () => {
      try {
        unlisten = await listen<SandboxLogLine>('sandbox-log', (event) => {
          if (event.payload.sandbox_id !== sandboxId) return
          setLines((prev) => [...prev, event.payload.line])
        })
        await invoke('stream_sandbox_logs', { id: sandboxId })
      } catch (e) {
        setError(String(e))
      }
    })()
    return () => unlisten?.()
  }, [sandboxId])

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ block: 'end' })
  }, [lines])

  return (
    <Card className="w-full max-w-2xl">
      <CardHeader className="flex flex-row items-center justify-between">
        <CardTitle>Sandbox logs</CardTitle>
        <Button size="sm" variant="outline" onClick={onClose}>
          Close
        </Button>
      </CardHeader>
      <CardContent>
        {error && <p className="text-sm text-destructive">{error}</p>}
        <div className="h-80 overflow-auto rounded-lg bg-muted p-3 font-mono text-xs">
          {lines.length === 0 && !error && <p className="text-muted-foreground">Waiting for log output…</p>}
          {lines.map((line, i) => (
            <div key={i} className="whitespace-pre-wrap">
              {line}
            </div>
          ))}
          <div ref={bottomRef} />
        </div>
      </CardContent>
    </Card>
  )
}
