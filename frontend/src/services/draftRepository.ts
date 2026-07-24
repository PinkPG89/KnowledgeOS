import type { BrowserDraft } from '@/models/markdown'
import { isMarkdownPath, isRfc3339Milliseconds } from '@/utils/canonicalPath'

const DATABASE_NAME = 'knowledgeos'
const DATABASE_VERSION = 1
const DRAFT_STORE_NAME = 'drafts'
const DRAFT_SCHEMA_VERSION = 1

interface StoredBrowserDraft extends BrowserDraft {
  version: number
}

export interface DraftRepository {
  get(path: string): Promise<BrowserDraft | null>
  put(draft: BrowserDraft): Promise<void>
  remove(path: string): Promise<void>
}

export class IndexedDbDraftRepository implements DraftRepository {
  private databasePromise: Promise<IDBDatabase> | null = null

  constructor(private readonly factory: IDBFactory) {}

  async get(path: string): Promise<BrowserDraft | null> {
    const storedDraft = await this.execute<unknown>('readonly', (store) => store.get(path))
    return parseStoredDraft(storedDraft, path)
  }

  async put(draft: BrowserDraft): Promise<void> {
    const storedDraft: StoredBrowserDraft = {
      version: DRAFT_SCHEMA_VERSION,
      ...draft,
    }
    await this.execute('readwrite', (store) => store.put(storedDraft))
  }

  async remove(path: string): Promise<void> {
    await this.execute('readwrite', (store) => store.delete(path))
  }

  private openDatabase(): Promise<IDBDatabase> {
    if (this.databasePromise) return this.databasePromise

    this.databasePromise = new Promise((resolve, reject) => {
      const request = this.factory.open(DATABASE_NAME, DATABASE_VERSION)

      request.onupgradeneeded = () => {
        const database = request.result
        if (!database.objectStoreNames.contains(DRAFT_STORE_NAME)) {
          database.createObjectStore(DRAFT_STORE_NAME, { keyPath: 'path' })
        }
      }
      request.onsuccess = () => {
        const database = request.result
        database.onversionchange = () => {
          database.close()
          this.databasePromise = null
        }
        resolve(database)
      }
      request.onerror = () => {
        this.databasePromise = null
        reject(request.error ?? new Error('IndexedDB open failed'))
      }
      request.onblocked = () => {
        this.databasePromise = null
        reject(new Error('IndexedDB upgrade was blocked'))
      }
    })

    return this.databasePromise
  }

  private async execute<Result>(
    mode: IDBTransactionMode,
    operation: (store: IDBObjectStore) => IDBRequest<Result>,
  ): Promise<Result> {
    const database = await this.openDatabase()

    return new Promise((resolve, reject) => {
      const transaction = database.transaction(DRAFT_STORE_NAME, mode)
      const request = operation(transaction.objectStore(DRAFT_STORE_NAME))
      let result: Result

      request.onsuccess = () => {
        result = request.result
      }
      transaction.oncomplete = () => resolve(result)
      transaction.onerror = () =>
        reject(transaction.error ?? request.error ?? new Error('IndexedDB transaction failed'))
      transaction.onabort = () =>
        reject(transaction.error ?? request.error ?? new Error('IndexedDB transaction aborted'))
    })
  }
}

let browserDraftRepository: DraftRepository | undefined

export function getBrowserDraftRepository(): DraftRepository | undefined {
  let factory: IDBFactory | undefined
  try {
    factory = globalThis.indexedDB
  } catch {
    return undefined
  }

  if (!factory) return undefined
  browserDraftRepository ??= new IndexedDbDraftRepository(factory)
  return browserDraftRepository
}

function parseStoredDraft(value: unknown, expectedPath: string): BrowserDraft | null {
  if (
    !isRecord(value) ||
    value.version !== DRAFT_SCHEMA_VERSION ||
    value.path !== expectedPath ||
    !isMarkdownPath(value.path) ||
    !isSha256Hash(value.baseHash) ||
    typeof value.content !== 'string' ||
    typeof value.updatedAt !== 'string' ||
    !isRfc3339Milliseconds(value.updatedAt)
  ) {
    return null
  }

  return {
    path: value.path,
    baseHash: value.baseHash,
    content: value.content,
    updatedAt: value.updatedAt,
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function isSha256Hash(value: unknown): value is string {
  return typeof value === 'string' && /^sha256:[0-9a-f]{64}$/.test(value)
}
