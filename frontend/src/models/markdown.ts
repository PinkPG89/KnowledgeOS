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
