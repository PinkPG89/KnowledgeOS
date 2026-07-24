export interface MarkdownDocument {
  path: string
  content: string
  hash: string
  size: number
  modifiedAt: string
}

export type DocumentLoadStatus = 'idle' | 'loading' | 'loaded' | 'error'

export interface DocumentLoadError {
  code: string
  message: string
  retryable: boolean
}

export type DocumentSaveStatus = 'clean' | 'dirty' | 'saving' | 'conflict' | 'error'

export interface DocumentSaveError {
  code: string
  message: string
  retryable: boolean
  currentHash: string | null
}

export type DraftRecoveryStatus = 'none' | 'available' | 'conflict'

export type DraftBackupStatus = 'idle' | 'pending' | 'saved' | 'error'

export interface BrowserDraft {
  path: string
  baseHash: string
  content: string
  updatedAt: string
}
