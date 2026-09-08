export interface FileTreeFileNode {
  type: 'file'
  name: string
  path: string
}

export interface FileTreeDirNode {
  type: 'dir'
  name: string
  path: string
  children: FileTreeNode[]
}

export type FileTreeNode = FileTreeFileNode | FileTreeDirNode

function sortNodes(nodes: FileTreeNode[]): FileTreeNode[] {
  return nodes.sort((a, b) => {
    if (a.type !== b.type) return a.type === 'dir' ? -1 : 1
    return a.name.localeCompare(b.name)
  })
}

// Nests a flat list of changed-file paths into a directory tree — the
// left-hand accordion panel's data source. Sorts directories before files,
// alphabetically within each group, at every level.
export function buildFileTree(filePaths: string[]): FileTreeNode[] {
  const root: FileTreeDirNode = { type: 'dir', name: '', path: '', children: [] }

  for (const filePath of filePaths) {
    const parts = filePath.split('/')
    let current = root
    parts.forEach((name, i) => {
      const path = parts.slice(0, i + 1).join('/')
      if (i === parts.length - 1) {
        current.children.push({ type: 'file', name, path })
        return
      }
      let dir = current.children.find((c): c is FileTreeDirNode => c.type === 'dir' && c.name === name)
      if (!dir) {
        dir = { type: 'dir', name, path, children: [] }
        current.children.push(dir)
      }
      current = dir
    })
  }

  function sortTree(nodes: FileTreeNode[]): FileTreeNode[] {
    for (const node of nodes) {
      if (node.type === 'dir') sortTree(node.children)
    }
    return sortNodes(nodes)
  }

  return sortTree(root.children)
}
