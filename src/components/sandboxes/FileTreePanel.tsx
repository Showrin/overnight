import { useMemo, useState } from 'react'
import { Accordion as AccordionPrimitive } from 'radix-ui'
import { ChevronRight, File, Folder, PanelLeftClose, PanelLeftOpen } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { collectDirPaths, type FileTreeNode } from '@/lib/buildFileTree'

function TreeNode({
  node,
  depth,
  selectedFile,
  onSelectFile,
  defaultOpenDirs,
}: {
  node: FileTreeNode
  depth: number
  selectedFile: string | null
  onSelectFile: (path: string) => void
  defaultOpenDirs: string[]
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
        <AccordionPrimitive.Root type="multiple" defaultValue={defaultOpenDirs}>
          {node.children.map((child) => (
            <TreeNode
              key={child.path}
              node={child}
              depth={depth + 1}
              selectedFile={selectedFile}
              onSelectFile={onSelectFile}
              defaultOpenDirs={defaultOpenDirs}
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
  const defaultOpenDirs = useMemo(() => collectDirPaths(tree), [tree])

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
    <div className="flex w-64 shrink-0 flex-col gap-1 border-r border-border pr-2">
      <div className="flex items-center justify-between px-1">
        <span className="text-xs text-muted-foreground/70">Files</span>
        <Button size="sm" variant="ghost" onClick={() => setCollapsed(true)} title="Hide file tree">
          <PanelLeftClose className="size-3.5" />
        </Button>
      </div>
      <AccordionPrimitive.Root type="multiple" defaultValue={defaultOpenDirs} className="flex flex-col gap-0.5 overflow-auto">
        {tree.map((node) => (
          <TreeNode
            key={node.path}
            node={node}
            depth={0}
            selectedFile={selectedFile}
            onSelectFile={onSelectFile}
            defaultOpenDirs={defaultOpenDirs}
          />
        ))}
      </AccordionPrimitive.Root>
    </div>
  )
}
