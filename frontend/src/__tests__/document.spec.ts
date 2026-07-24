import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { BrowserDraft, MarkdownDocument } from '@/models/markdown'
import type { DraftRepository } from '@/services/draftRepository'
import { MarkdownClientError, type MarkdownClient } from '@/services/markdownClient'
import { useDocumentStore } from '@/stores/document'

const hash = `sha256:${'a'.repeat(64)}`

function document(path: string, content: string): MarkdownDocument {
  return {
    path,
    content,
    hash,
    size: new TextEncoder().encode(content).byteLength,
    modifiedAt: '2026-07-22T01:02:03.004Z',
  }
}

class MemoryDraftRepository implements DraftRepository {
  readonly drafts = new Map<string, BrowserDraft>()

  async get(path: string): Promise<BrowserDraft | null> {
    return this.drafts.get(path) ?? null
  }

  async put(draft: BrowserDraft): Promise<void> {
    this.drafts.set(draft.path, structuredClone(draft))
  }

  async remove(path: string): Promise<void> {
    this.drafts.delete(path)
  }
}

describe('document store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('aborts the previous request and ignores its stale response', async () => {
    const resolvers = new Map<string, (value: MarkdownDocument) => void>()
    const signals = new Map<string, AbortSignal | undefined>()
    const client: MarkdownClient = {
      readFile: vi.fn(
        (path, signal) =>
          new Promise<MarkdownDocument>((resolve) => {
            resolvers.set(path, resolve)
            signals.set(path, signal)
          }),
      ),
      updateFile: vi.fn(),
    }
    const store = useDocumentStore()

    const first = store.openFile('first.md', client)
    const second = store.openFile('second.md', client)
    resolvers.get('second.md')?.(document('second.md', 'second'))
    await second
    resolvers.get('first.md')?.(document('first.md', 'first'))
    await first

    expect(signals.get('first.md')?.aborted).toBe(true)
    expect(store.activePath).toBe('second.md')
    expect(store.document?.content).toBe('second')
  })

  it('exposes a retryable error and succeeds on retry', async () => {
    const readFile = vi
      .fn<MarkdownClient['readFile']>()
      .mockRejectedValueOnce(new MarkdownClientError('internal_error', 'Temporary failure', 500))
      .mockResolvedValueOnce(document('note.md', 'recovered'))
    const client: MarkdownClient = { readFile, updateFile: vi.fn() }
    const store = useDocumentStore()

    await store.openFile('note.md', client)
    expect(store.status).toBe('error')
    expect(store.error).toEqual({
      code: 'internal_error',
      message: 'Temporary failure',
      retryable: true,
    })

    await store.retry(client)
    expect(store.status).toBe('loaded')
    expect(store.document?.content).toBe('recovered')
  })

  it('clears active state and aborts an in-flight request', async () => {
    let observedSignal: AbortSignal | undefined
    const client: MarkdownClient = {
      readFile: (_path, signal) => {
        observedSignal = signal
        return new Promise(() => undefined)
      },
      updateFile: vi.fn(),
    }
    const store = useDocumentStore()

    void store.openFile('note.md', client)
    store.clearFile()

    expect(observedSignal?.aborted).toBe(true)
    expect(store.status).toBe('idle')
    expect(store.activePath).toBeNull()
  })

  it('prevents duplicate saves and preserves edits made while saving', async () => {
    const repository = new MemoryDraftRepository()
    let resolveUpdate: ((value: MarkdownDocument) => void) | undefined
    const updateFile = vi.fn(
      () =>
        new Promise<MarkdownDocument>((resolve) => {
          resolveUpdate = resolve
        }),
    )
    const client: MarkdownClient = {
      readFile: vi.fn().mockResolvedValue(document('note.md', 'original')),
      updateFile,
    }
    const store = useDocumentStore()
    await store.openFile('note.md', client, repository)
    store.setDraft('first change')

    const firstSave = store.save(client)
    const duplicateSave = store.save(client)
    store.setDraft('second change')

    expect(updateFile).toHaveBeenCalledTimes(1)
    expect(updateFile).toHaveBeenCalledWith('note.md', 'first change', hash)
    expect(store.saveStatus).toBe('saving')

    resolveUpdate?.({
      ...document('note.md', 'first change'),
      hash: `sha256:${'b'.repeat(64)}`,
    })
    await Promise.all([firstSave, duplicateSave])

    expect(store.document?.content).toBe('first change')
    expect(store.draft).toBe('second change')
    expect(store.saveStatus).toBe('dirty')
    await store.flushDraftPersistence()
    expect(repository.drafts.get('note.md')).toMatchObject({
      baseHash: `sha256:${'b'.repeat(64)}`,
      content: 'second change',
    })
  })

  it('retries a retryable save error', async () => {
    const savedDocument = {
      ...document('note.md', 'updated'),
      hash: `sha256:${'b'.repeat(64)}`,
    }
    const updateFile = vi
      .fn<MarkdownClient['updateFile']>()
      .mockRejectedValueOnce(new MarkdownClientError('internal_error', 'Temporary failure', 500))
      .mockResolvedValueOnce(savedDocument)
    const client: MarkdownClient = {
      readFile: vi.fn().mockResolvedValue(document('note.md', 'original')),
      updateFile,
    }
    const store = useDocumentStore()
    await store.openFile('note.md', client)
    store.setDraft('updated')

    await store.save(client)
    expect(store.saveStatus).toBe('error')
    expect(store.saveError?.retryable).toBe(true)

    await store.retrySave(client)
    expect(updateFile).toHaveBeenCalledTimes(2)
    expect(store.saveStatus).toBe('clean')
    expect(store.document).toEqual(savedDocument)
  })

  it('keeps the local draft when the backend reports a conflict', async () => {
    const currentHash = `sha256:${'c'.repeat(64)}`
    const client: MarkdownClient = {
      readFile: vi.fn().mockResolvedValue(document('note.md', 'original')),
      updateFile: vi
        .fn()
        .mockRejectedValue(
          new MarkdownClientError('write_conflict', 'Markdown file changed', 409, currentHash),
        ),
    }
    const store = useDocumentStore()
    await store.openFile('note.md', client)
    store.setDraft('local draft')

    await store.save(client)

    expect(store.saveStatus).toBe('conflict')
    expect(store.draft).toBe('local draft')
    expect(store.document?.content).toBe('original')
    expect(store.saveError).toEqual({
      code: 'write_conflict',
      message: 'Markdown file changed',
      retryable: false,
      currentHash,
    })
  })

  it('persists and resumes a browser draft after a store reload', async () => {
    const repository = new MemoryDraftRepository()
    const client: MarkdownClient = {
      readFile: vi.fn().mockResolvedValue(document('note.md', 'server content')),
      updateFile: vi.fn(),
    }
    const firstStore = useDocumentStore()
    await firstStore.openFile('note.md', client, repository)
    firstStore.setDraft('local draft')
    await firstStore.flushDraftPersistence()

    expect(repository.drafts.get('note.md')).toMatchObject({
      path: 'note.md',
      baseHash: hash,
      content: 'local draft',
    })

    setActivePinia(createPinia())
    const reloadedStore = useDocumentStore()
    await reloadedStore.openFile('note.md', client, repository)

    expect(reloadedStore.recoveryStatus).toBe('available')
    expect(reloadedStore.draft).toBe('server content')
    await reloadedStore.resumeRecoveredDraft()
    expect(reloadedStore.draft).toBe('local draft')
    expect(reloadedStore.saveStatus).toBe('dirty')
  })

  it('isolates a recovered draft when the server hash changed', async () => {
    const repository = new MemoryDraftRepository()
    const serverHash = `sha256:${'c'.repeat(64)}`
    await repository.put({
      path: 'note.md',
      baseHash: hash,
      content: 'stale local draft',
      updatedAt: '2026-07-24T01:02:03.004Z',
    })
    const client: MarkdownClient = {
      readFile: vi.fn().mockResolvedValue({
        ...document('note.md', 'new server content'),
        hash: serverHash,
      }),
      updateFile: vi.fn(),
    }
    const store = useDocumentStore()

    await store.openFile('note.md', client, repository)

    expect(store.recoveryStatus).toBe('conflict')
    expect(store.draft).toBe('new server content')
    await store.resumeRecoveredDraft()
    expect(store.draft).toBe('stale local draft')
    expect(store.document?.content).toBe('new server content')
    expect(store.saveStatus).toBe('conflict')
    expect(store.canSave).toBe(false)
  })

  it('removes the browser draft after a successful save', async () => {
    const repository = new MemoryDraftRepository()
    const savedDocument = {
      ...document('note.md', 'saved content'),
      hash: `sha256:${'b'.repeat(64)}`,
    }
    const client: MarkdownClient = {
      readFile: vi.fn().mockResolvedValue(document('note.md', 'server content')),
      updateFile: vi.fn().mockResolvedValue(savedDocument),
    }
    const store = useDocumentStore()
    await store.openFile('note.md', client, repository)
    store.setDraft('saved content')
    await store.flushDraftPersistence()
    expect(repository.drafts.has('note.md')).toBe(true)

    await store.save(client)
    await store.flushDraftPersistence()

    expect(store.saveStatus).toBe('clean')
    expect(repository.drafts.has('note.md')).toBe(false)
  })

  it('opens the server document when browser draft storage fails', async () => {
    const repository: DraftRepository = {
      get: vi.fn().mockRejectedValue(new Error('quota unavailable')),
      put: vi.fn(),
      remove: vi.fn(),
    }
    const client: MarkdownClient = {
      readFile: vi.fn().mockResolvedValue(document('note.md', 'server content')),
      updateFile: vi.fn(),
    }
    const store = useDocumentStore()

    await store.openFile('note.md', client, repository)

    expect(store.status).toBe('loaded')
    expect(store.document?.content).toBe('server content')
    expect(store.draftBackupStatus).toBe('error')
    expect(store.draftBackupError).toBe('브라우저 초안을 불러오지 못했습니다.')
  })

  it('keeps recovery state when deleting the browser draft fails', async () => {
    const repository: DraftRepository = {
      get: vi.fn().mockResolvedValue({
        path: 'note.md',
        baseHash: hash,
        content: 'local draft',
        updatedAt: '2026-07-24T01:02:03.004Z',
      }),
      put: vi.fn(),
      remove: vi.fn().mockRejectedValue(new Error('quota unavailable')),
    }
    const client: MarkdownClient = {
      readFile: vi.fn().mockResolvedValue(document('note.md', 'server content')),
      updateFile: vi.fn(),
    }
    const store = useDocumentStore()
    await store.openFile('note.md', client, repository)

    await store.discardRecoveredDraft()

    expect(store.recoveryStatus).toBe('available')
    expect(store.recoveredDraft?.content).toBe('local draft')
    expect(store.draftBackupStatus).toBe('error')
    expect(store.draftBackupError).toBe('브라우저 초안을 삭제하지 못했습니다.')
  })
})
