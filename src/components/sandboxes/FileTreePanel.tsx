import { useCallback, useMemo, useRef, useState } from 'react'
import { Accordion as AccordionPrimitive } from 'radix-ui'
import { ChevronRight, File, Folder, PanelLeftClose, PanelLeftOpen, Search } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'
import { collectDirPaths, filterFileTree, type FileTreeNode } from '@/lib/buildFileTree'

const MIN_WIDTH = 240
const MAX_WIDTH = 400

function TreeNode({
  node,
  depth,
  selectedFile,
  onSelectFile,
  defaultOpenDirs,
  query,
}: {
  node: FileTreeNode
  depth: number
  selectedFile: string | null
  onSelectFile: (path: string) => void
  defaultOpenDirs: string[]
  query: string
}) {
  const indent = { paddingLeft: `${depth * 14 + 8}px` }

  if (node.type === 'file') {
    return (
      <button
        type="button"
        onClick={() => onSelectFile(node.path)}
        style={indent}
        className={cn(
          'flex w-full items-center gap-1.5 rounded py-1 pr-2 text-left text-xs',
          node.path === selectedFile ? 'bg-accent text-accent-foreground' : 'text-muted-foreground hover:bg-accent/50'
        )}
      >
        <File className="size-3.5 shrink-0" />
        <span className="truncate">{node.name}</span>
      </button>
    )
  }

  return (
    <AccordionPrimitive.Item value={node.path}>
      <AccordionPrimitive.Header>
        <AccordionPrimitive.Trigger
          style={indent}
          className="flex w-full items-center gap-1.5 rounded py-1 pr-2 text-left text-xs text-foreground hover:bg-accent/50 [&[data-state=open]>svg:first-child]:rotate-90"
        >
          <ChevronRight className="size-3.5 shrink-0 transition-transform" />
          <Folder className="size-3.5 shrink-0" />
          <span className="truncate">{node.name}</span>
        </AccordionPrimitive.Trigger>
      </AccordionPrimitive.Header>
      <AccordionPrimitive.Content>
        <AccordionPrimitive.Root key={query} type="multiple" defaultValue={defaultOpenDirs}>
          {node.children.map((child) => (
            <TreeNode
              key={child.path}
              node={child}
              depth={depth + 1}
              selectedFile={selectedFile}
              onSelectFile={onSelectFile}
              defaultOpenDirs={defaultOpenDirs}
              query={query}
            />
          ))}
        </AccordionPrimitive.Root>
      </AccordionPrimitive.Content>
    </AccordionPrimitive.Item>
  )
}

export function FileTreePanel({
  tree,
  selectedFile,
  onSelectFile,
}: {
  tree: FileTreeNode[]
  selectedFile: string | null
  onSelectFile: (path: string) => void
}) {
  const [collapsed, setCollapsed] = useState(false)
  const [query, setQuery] = useState('')
  const [width, setWidth] = useState(256) // matches previous w-64
  const resizing = useRef(false)
  const filteredTree = useMemo(() => filterFileTree(tree, query), [tree, query])
  const defaultOpenDirs = useMemo(() => collectDirPaths(tree), [tree])
  const openDirs = useMemo(
    () => (query.trim() ? collectDirPaths(filteredTree) : defaultOpenDirs),
    [query, filteredTree, defaultOpenDirs]
  )

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault()
      resizing.current = true
      const startX = e.clientX
      const startWidth = width

      function onMouseMove(ev: MouseEvent) {
        if (!resizing.current) return
        const next = startWidth + (ev.clientX - startX)
        setWidth(Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, next)))
      }
      function onMouseUp() {
        resizing.current = false
        window.removeEventListener('mousemove', onMouseMove)
        window.removeEventListener('mouseup', onMouseUp)
      }
      window.addEventListener('mousemove', onMouseMove)
      window.addEventListener('mouseup', onMouseUp)
    },
    [width]
  )

  if (collapsed) {
    return (
      <div className="flex shrink-0 flex-col border-r border-border">
        <Button size="sm" variant="ghost" onClick={() => setCollapsed(false)} title="Show file tree">
          <PanelLeftOpen className="size-3.5" />
        </Button>
      </div>
    )
  }

  return (
    <div className="relative flex shrink-0 flex-col gap-1 border-r border-border pr-2" style={{ width }}>
      <div className="flex items-center justify-between px-1">
        <span className="text-xs text-muted-foreground/70">Files</span>
        <Button size="sm" variant="ghost" onClick={() => setCollapsed(true)} title="Hide file tree">
          <PanelLeftClose className="size-3.5" />
        </Button>
      </div>
      <div className="relative px-1">
        <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
        <Input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Filter files…"
          className="h-7 pl-7 text-xs"
        />
      </div>
      {filteredTree.length === 0 ? (
        <p className="px-2 py-2 text-xs text-muted-foreground">No files match "{query}"</p>
      ) : (
        <AccordionPrimitive.Root
          key={query}
          type="multiple"
          defaultValue={openDirs}
          className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-auto"
        >
          {filteredTree.map((node) => (
            <TreeNode
              key={node.path}
              node={node}
              depth={0}
              selectedFile={selectedFile}
              onSelectFile={onSelectFile}
              defaultOpenDirs={openDirs}
              query={query}
            />
          ))}
        </AccordionPrimitive.Root>
      )}
      <div
        onMouseDown={handleMouseDown}
        className="absolute right-0 top-0 h-full w-1 cursor-col-resize select-none hover:bg-accent active:bg-accent"
      />
    </div>
  )
}
