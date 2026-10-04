import { useEffect, useRef, useState } from 'react'

export type SaveState = 'idle' | 'saving' | 'saved' | 'error'

// `value` must keep identity until it changes.
export function useAutosave<T>(value: T, save: (value: T) => Promise<void>, enabled = true, delay = 600) {
  const [state, setState] = useState<SaveState>('idle')
  const [error, setError] = useState<string | null>(null)
  const saveRef = useRef(save)
  const pending = useRef<T | null>(null)
  const first = useRef(true)

  useEffect(() => {
    saveRef.current = save
  }, [save])

  useEffect(() => {
    if (!enabled) pending.current = null
  }, [enabled])

  useEffect(() => {
    if (first.current) {
      first.current = false
      return
    }
    if (!enabled) return
    pending.current = value
    const timer = setTimeout(async () => {
      pending.current = null
      setState('saving')
      try {
        await saveRef.current(value)
        setState('saved')
        setError(null)
      } catch (e) {
        setState('error')
        setError(String(e))
      }
    }, delay)
    return () => clearTimeout(timer)
  }, [value, delay, enabled])

  useEffect(
    () => () => {
      if (pending.current !== null) saveRef.current(pending.current).catch(() => {})
    },
    []
  )

  return { state, error }
}
