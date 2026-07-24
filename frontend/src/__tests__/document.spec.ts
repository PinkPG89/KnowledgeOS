import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { MarkdownDocument } from '@/models/markdown'
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
    await store.openFile('note.md', client)
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
})
