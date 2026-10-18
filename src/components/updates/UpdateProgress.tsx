import { useUpdater } from '@/store/useUpdater'

function formatMb(bytes: number): string {
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}

/** Download bar shown while an update is being fetched and installed. */
export function UpdateProgress() {
  const status = useUpdater((s) => s.status)
  const downloaded = useUpdater((s) => s.downloaded)
  const contentLength = useUpdater((s) => s.contentLength)

  if (status !== 'downloading' && status !== 'installing') return null

  const percent = contentLength ? Math.min(100, Math.round((downloaded / contentLength) * 100)) : 0

  return (
    <div className="flex flex-col gap-1">
      <div className="h-1.5 w-full overflow-hidden rounded-full bg-muted">
        <div
          className="h-full rounded-full bg-primary transition-[width]"
          style={{ width: status === 'installing' ? '100%' : `${percent}%` }}
        />
      </div>
      <span className="text-xs text-muted-foreground">
        {status === 'installing'
          ? 'Installing… the app will restart.'
          : `Downloading ${formatMb(downloaded)}${contentLength ? ` of ${formatMb(contentLength)}` : ''}`}
      </span>
    </div>
  )
}
