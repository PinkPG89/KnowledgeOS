export type TreeNodeKind = 'directory' | 'file'

export interface TreeNode {
  kind: TreeNodeKind
  name: string
  path: string
  size?: number
  modifiedAt: string
}

export interface TreeListing {
  path: string
  entries: TreeNode[]
}

export type DirectoryLoadStatus = 'idle' | 'loading' | 'loaded' | 'error'

export interface TreeLoadError {
  code: string
  message: string
  retryable: boolean
}

export interface TreeDirectoryState {
  childPaths: string[]
  loadStatus: DirectoryLoadStatus
  expanded: boolean
  error: TreeLoadError | null
}

export type TreeLoadResult = { ok: true } | { ok: false; error: TreeLoadError }
