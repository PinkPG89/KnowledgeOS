import { defineStore } from 'pinia'
import { ref, shallowRef } from 'vue'

import type { DocumentLoadError, DocumentLoadStatus, MarkdownDocument } from '@/models/markdown'
import { MarkdownClientError, markdownClient, type MarkdownClient } from '@/services/markdownClient'

export const useDocumentStore = defineStore('document', () => {
  const status = ref<DocumentLoadStatus>('idle')
  const activePath = ref<string | null>(null)
  const document = shallowRef<MarkdownDocument | null>(null)
  const error = ref<DocumentLoadError | null>(null)

  let requestGeneration = 0
  let activeController: AbortController | null = null

  async function openFile(path: string, client: MarkdownClient = markdownClient): Promise<void> {
    requestGeneration += 1
    const generation = requestGeneration

    activeController?.abort()
    const controller = new AbortController()
    activeController = controller

    activePath.value = path
    document.value = null
    error.value = null
    status.value = 'loading'

    try {
      const loadedDocument = await client.readFile(path, controller.signal)
      if (generation !== requestGeneration) return

      document.value = loadedDocument
      status.value = 'loaded'
    } catch (reason) {
      if (generation !== requestGeneration) return

      const loadError = toDocumentLoadError(reason)
      if (loadError.code === 'request_aborted') return

      error.value = loadError
      status.value = 'error'
    } finally {
      if (generation === requestGeneration) activeController = null
    }
  }

  function retry(client: MarkdownClient = markdownClient): Promise<void> {
    if (!activePath.value) return Promise.resolve()
    return openFile(activePath.value, client)
  }

  function clearFile() {
    requestGeneration += 1
    activeController?.abort()
    activeController = null
    status.value = 'idle'
    activePath.value = null
    document.value = null
    error.value = null
  }

  return { status, activePath, document, error, openFile, retry, clearFile }
})

function toDocumentLoadError(reason: unknown): DocumentLoadError {
  if (reason instanceof MarkdownClientError) {
    return {
      code: reason.code,
      message: reason.message,
      retryable: reason.code !== 'invalid_path' && (reason.status === null || reason.status >= 500),
    }
  }

  return {
    code: 'document_load_failed',
    message: 'Markdown 문서를 불러오지 못했습니다.',
    retryable: true,
  }
}
