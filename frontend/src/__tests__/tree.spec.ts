import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { TreeListing, TreeNode } from '@/models/tree'
import type { TreeClient } from '@/services/treeClient'
import { TreeClientError } from '@/services/treeClient'
import { useTreeStore } from '@/stores/tree'

const timestamp = '2026-07-22T01:02:03.004Z'

function directory(name: string, path = name): TreeNode {
  return { kind: 'directory', name, path, modifiedAt: timestamp }
}

function file(name: string, path = name): TreeNode {
  return { kind: 'file', name, path, size: 10, modifiedAt: timestamp }
}

function clientWith(listDirectory: TreeClient['listDirectory']): TreeClient {
  return { listDirectory }
}

describe('tree store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('upserts nodes and applies directory-first Unicode ordering', async () => {
    const store = useTreeStore()
    const client = clientWith(async () => ({
      path: '',
      entries: [file('z.md'), directory('한글'), directory('alpha'), file('a.md')],
    }))

    const result = await store.loadDirectory('', client)

    expect(result.ok).toBe(true)
    expect(store.directoriesByPath['']?.childPaths).toEqual(['alpha', '한글', 'a.md', 'z.md'])
    expect(store.nodesByPath['한글']?.kind).toBe('directory')
    expect(store.directoriesByPath['한글']?.loadStatus).toBe('idle')
  })

  it('shares one pending load and skips a request after loading', async () => {
    let resolveListing: ((listing: TreeListing) => void) | undefined
    const deferred = new Promise<TreeListing>((resolve) => {
      resolveListing = resolve
    })
    const listDirectory = vi.fn(() => deferred)
    const client = clientWith(listDirectory)
    const store = useTreeStore()

    const first = store.loadDirectory('', client)
    const second = store.loadDirectory('', client)

    await Promise.resolve()
    expect(listDirectory).toHaveBeenCalledTimes(1)
    resolveListing?.({ path: '', entries: [] })
    await Promise.all([first, second])
    await store.loadDirectory('', client)

    expect(listDirectory).toHaveBeenCalledTimes(1)
    expect(store.directoriesByPath['']?.loadStatus).toBe('loaded')
  })

  it('preserves loaded children while collapsed and expands without refetching', async () => {
    const listDirectory = vi.fn(async () => ({
      path: '',
      entries: [directory('projects')],
    }))
    const client = clientWith(listDirectory)
    const store = useTreeStore()
    await store.loadDirectory('', client)

    const childClient = clientWith(
      vi.fn(async () => ({ path: 'projects', entries: [file('note.md', 'projects/note.md')] })),
    )
    await store.toggleDirectory('projects', childClient)
    await store.toggleDirectory('projects', childClient)
    await store.toggleDirectory('projects', childClient)

    expect(childClient.listDirectory).toHaveBeenCalledTimes(1)
    expect(store.directoriesByPath.projects?.expanded).toBe(true)
    expect(store.directoriesByPath.projects?.childPaths).toEqual(['projects/note.md'])
  })

  it('records a typed failure and succeeds on retry', async () => {
    let attempts = 0
    const listDirectory = vi.fn(() => {
      attempts += 1
      if (attempts === 1) {
        throw new TreeClientError('internal_error', 'Temporary failure', 500)
      }
      return Promise.resolve({ path: '', entries: [] })
    })
    const client = clientWith(listDirectory)
    const store = useTreeStore()

    const failed = await store.loadDirectory('', client)

    expect(failed).toEqual({
      ok: false,
      error: { code: 'internal_error', message: 'Temporary failure', retryable: true },
    })
    expect(store.directoriesByPath['']?.loadStatus).toBe('error')

    const retried = await store.loadDirectory('', client)
    expect(retried.ok).toBe(true)
    expect(store.directoriesByPath['']?.loadStatus).toBe('loaded')
    expect(listDirectory).toHaveBeenCalledTimes(2)
  })

  it('does not create directory state when a file is toggled', async () => {
    const store = useTreeStore()
    await store.loadDirectory(
      '',
      clientWith(async () => ({ path: '', entries: [file('note.md')] })),
    )
    const client = clientWith(vi.fn(async () => ({ path: 'note.md', entries: [] })))

    const result = await store.toggleDirectory('note.md', client)

    expect(result).toEqual({
      ok: false,
      error: {
        code: 'not_a_tree_directory',
        message: '선택한 Tree node는 directory가 아닙니다.',
        retryable: false,
      },
    })
    expect(client.listDirectory).not.toHaveBeenCalled()
    expect(store.directoriesByPath['note.md']).toBeUndefined()
  })

  it('selects only nodes present in the projection', async () => {
    const store = useTreeStore()
    await store.loadDirectory(
      '',
      clientWith(async () => ({ path: '', entries: [file('note.md')] })),
    )

    expect(store.selectNode('missing.md')).toBe(false)
    expect(store.selectedPath).toBeNull()
    expect(store.selectNode('note.md')).toBe(true)
    expect(store.selectedPath).toBe('note.md')
    expect(store.selectNode(null)).toBe(true)
    expect(store.selectedPath).toBeNull()
  })
})
