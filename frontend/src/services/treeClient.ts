import type { TreeListing, TreeNode, TreeNodeKind } from '@/models/tree'
import {
  directParentPath,
  finalPathSegment,
  isCanonicalRelativePath,
  isRfc3339Milliseconds,
} from '@/utils/canonicalPath'

type Fetcher = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>

export interface TreeClient {
  listDirectory(path: string): Promise<TreeListing>
}

export class TreeClientError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly status: number | null,
  ) {
    super(message)
    this.name = 'TreeClientError'
  }
}

export class HttpTreeClient implements TreeClient {
  constructor(
    private readonly fetcher: Fetcher = (...arguments_) => globalThis.fetch(...arguments_),
  ) {}

  async listDirectory(path: string): Promise<TreeListing> {
    const url = new URL('/api/tree', globalThis.location.origin)
    if (path) url.searchParams.set('path', path)

    let response: Response
    try {
      response = await this.fetcher(`${url.pathname}${url.search}`, {
        headers: { Accept: 'application/json' },
      })
    } catch {
      throw new TreeClientError('network_error', 'Tree API에 연결할 수 없습니다.', null)
    }

    const body = await readJson(response)
    if (!response.ok) throw parseApiError(response.status, body)

    return parseTreeListing(body, path)
  }
}

export const treeClient: TreeClient = new HttpTreeClient()

async function readJson(response: Response): Promise<unknown> {
  try {
    return await response.json()
  } catch {
    throw new TreeClientError(
      'invalid_response',
      'Tree API가 올바른 JSON을 반환하지 않았습니다.',
      response.status,
    )
  }
}

function parseApiError(status: number, body: unknown): TreeClientError {
  if (!isRecord(body) || !isRecord(body.error)) {
    return new TreeClientError('http_error', 'Tree API 요청이 실패했습니다.', status)
  }

  const code = typeof body.error.code === 'string' ? body.error.code : 'http_error'
  const message =
    typeof body.error.message === 'string' ? body.error.message : 'Tree API 요청이 실패했습니다.'
  return new TreeClientError(code, message, status)
}

function parseTreeListing(body: unknown, requestedPath: string): TreeListing {
  if (!isRecord(body) || typeof body.path !== 'string' || !Array.isArray(body.entries)) {
    throw invalidResponse()
  }

  const listingPath = body.path
  if (listingPath !== requestedPath || !isDirectoryPath(listingPath)) throw invalidResponse()

  return {
    path: listingPath,
    entries: body.entries.map((entry) => parseTreeNode(entry, listingPath)),
  }
}

function parseTreeNode(value: unknown, parentPath: string): TreeNode {
  if (
    !isRecord(value) ||
    !isTreeNodeKind(value.type) ||
    typeof value.name !== 'string' ||
    typeof value.path !== 'string' ||
    typeof value.modified_at !== 'string'
  ) {
    throw invalidResponse()
  }

  if (
    !isCanonicalRelativePath(value.path) ||
    directParentPath(value.path) !== parentPath ||
    finalPathSegment(value.path) !== value.name ||
    !isRfc3339Milliseconds(value.modified_at)
  ) {
    throw invalidResponse()
  }

  if (value.type === 'directory') {
    if (value.size !== undefined) throw invalidResponse()
    return toTreeNode(value.type, value.name, value.path, value.modified_at)
  }

  if (
    !value.path.endsWith('.md') ||
    typeof value.size !== 'number' ||
    !Number.isSafeInteger(value.size) ||
    value.size < 0
  ) {
    throw invalidResponse()
  }

  return toTreeNode(value.type, value.name, value.path, value.modified_at, value.size)
}

function toTreeNode(
  kind: TreeNodeKind,
  name: string,
  path: string,
  modifiedAt: string,
  size?: number,
): TreeNode {
  return { kind, name, path, modifiedAt, ...(size === undefined ? {} : { size }) }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function isTreeNodeKind(value: unknown): value is TreeNodeKind {
  return value === 'directory' || value === 'file'
}

function isDirectoryPath(path: string): boolean {
  return path === '' || isCanonicalRelativePath(path)
}

function invalidResponse(): TreeClientError {
  return new TreeClientError(
    'invalid_response',
    'Tree API 응답이 예상한 schema와 일치하지 않습니다.',
    null,
  )
}
