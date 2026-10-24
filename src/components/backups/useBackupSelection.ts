import { useState } from 'react'

export function useBackupSelection() {
  const [selecting, setSelecting] = useState(false)
  const [selected, setSelected] = useState<Set<string>>(new Set())

  function start() {
    setSelecting(true)
  }

  function stop() {
    setSelecting(false)
    setSelected(new Set())
  }

  function toggle(id: string, checked: boolean) {
    setMany([id], checked)
  }

  function setMany(ids: string[], checked: boolean) {
    setSelected((prev) => {
      const next = new Set(prev)
      for (const id of ids) {
        if (checked) next.add(id)
        else next.delete(id)
      }
      return next
    })
  }

  return { selecting, selected, start, stop, toggle, setMany }
}
