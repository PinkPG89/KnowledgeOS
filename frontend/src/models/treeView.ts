import type { TreeDirectoryState, TreeNode } from '@/models/tree'
import { directParentPath } from '@/utils/canonicalPath'

export interface VisibleTreeItem {
  node: TreeNode
  depth: number
  parentPath: string
  position: number
  setSize: number
}

type TreeNodes = Readonly<Record<string, TreeNode | undefined>>
type TreeDirectories = Readonly<Record<string, TreeDirectoryState | undefined>>

export function buildVisibleTreeItems(
  rootPath: string,
  nodesByPath: TreeNodes,
  directoriesByPath: TreeDirectories,
): VisibleTreeItem[] {
  const items: VisibleTreeItem[] = []
  appendVisibleChildren(rootPath, 1, items, nodesByPath, directoriesByPath, new Set())
  return items
}

export function resolveDocumentCreateParent(
  focusedPath: string | null,
  selectedPath: string | null,
  nodesByPath: TreeNodes,
  rootPath: string,
): string {
  const contextualPath = focusedPath ?? selectedPath
  const node = contextualPath ? nodesByPath[contextualPath] : undefined

  if (node?.kind === 'directory') return node.path
  if (node?.kind === 'file') return directParentPath(node.path)
  return rootPath
}

function appendVisibleChildren(
  parentPath: string,
  depth: number,
  items: VisibleTreeItem[],
  nodesByPath: TreeNodes,
  directoriesByPath: TreeDirectories,
  visitedDirectories: Set<string>,
) {
  if (visitedDirectories.has(parentPath)) return
  visitedDirectories.add(parentPath)

  const directory = directoriesByPath[parentPath]
  if (!directory) return

  const children = directory.childPaths
    .map((path) => nodesByPath[path])
    .filter((node): node is TreeNode => node !== undefined)

  children.forEach((node, index) => {
    items.push({
      node,
      depth,
      parentPath,
      position: index + 1,
      setSize: children.length,
    })

    if (node.kind === 'directory' && directoriesByPath[node.path]?.expanded) {
      appendVisibleChildren(
        node.path,
        depth + 1,
        items,
        nodesByPath,
        directoriesByPath,
        visitedDirectories,
      )
    }
  })
}
