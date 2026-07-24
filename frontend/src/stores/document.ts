import { defineStore } from 'pinia'
import { computed, ref, shallowRef } from 'vue'

import type {
  DocumentLoadError,
  DocumentLoadStatus,
  DocumentSaveError,
  DocumentSaveStatus,
  MarkdownDocument,
} from '@/models/markdown'
import { MarkdownClientError, markdownClient, type MarkdownClient } from '@/services/markdownClient'

export const useDocumentStore = defineStore('document', () => {
  const status = ref<DocumentLoadStatus>('idle')
  const activePath = ref<string | null>(null)
  const document = shallowRef<MarkdownDocument | null>(null)
  const error = ref<DocumentLoadError | null>(null)
  const draft = ref('')
  const saveStatus = ref<DocumentSaveStatus>('clean')
  const saveError = ref<DocumentSaveError | null>(null)
  const hasUnsavedChanges = computed(
    () => document.value !== null && draft.value !== document.value.content,
  )
  const canSave = computed(
    () =>
      status.value === 'loaded' &&
      hasUnsavedChanges.value &&
      saveStatus.value !== 'saving' &&
      saveStatus.value !== 'conflict',
  )

  let requestGeneration = 0
  let activeController: AbortController | null = null
  let activeSave: Promise<void> | null = null
  let activeSaveToken: symbol | null = null

  async function openFile(path: string, client: MarkdownClient = markdownClient): Promise<void> {
    requestGeneration += 1
    const generation = requestGeneration

    activeController?.abort()
    const controller = new AbortController()
    activeController = controller
    activeSave = null
    activeSaveToken = null

    activePath.value = path
    document.value = null
    error.value = null
    resetSaveState()
    status.value = 'loading'

    try {
      const loadedDocument = await client.readFile(path, controller.signal)
      if (generation !== requestGeneration) return

      document.value = loadedDocument
      draft.value = loadedDocument.content
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

  function setDraft(content: string) {
    if (!document.value) return

    draft.value = content
    if (saveStatus.value === 'saving' || saveStatus.value === 'conflict') return

    saveError.value = null
    saveStatus.value = content === document.value.content ? 'clean' : 'dirty'
  }

  function save(client: MarkdownClient = markdownClient): Promise<void> {
    if (activeSave) return activeSave
    if (!canSave.value || !document.value) return Promise.resolve()

    const generation = requestGeneration
    const path = document.value.path
    const content = draft.value
    const baseHash = document.value.hash
    const saveToken = Symbol('document-save')
    saveStatus.value = 'saving'
    saveError.value = null
    activeSaveToken = saveToken

    const saveRequest = (async () => {
      try {
        const savedDocument = await client.updateFile(path, content, baseHash)
        if (generation !== requestGeneration || activePath.value !== path) return

        document.value = savedDocument
        saveStatus.value = draft.value === content ? 'clean' : 'dirty'
      } catch (reason) {
        if (generation !== requestGeneration || activePath.value !== path) return

        const nextError = toDocumentSaveError(reason)
        saveError.value = nextError
        saveStatus.value = nextError.code === 'write_conflict' ? 'conflict' : 'error'
      } finally {
        if (activeSaveToken === saveToken) {
          activeSave = null
          activeSaveToken = null
        }
      }
    })()

    activeSave = saveRequest
    return saveRequest
  }

  function retrySave(client: MarkdownClient = markdownClient): Promise<void> {
    if (saveStatus.value !== 'error' || !saveError.value?.retryable) return Promise.resolve()
    saveStatus.value = 'dirty'
    return save(client)
  }

  function discardAndReload(client: MarkdownClient = markdownClient): Promise<void> {
    if (!activePath.value) return Promise.resolve()
    return openFile(activePath.value, client)
  }

  function clearFile() {
    requestGeneration += 1
    activeController?.abort()
    activeController = null
    activeSave = null
    activeSaveToken = null
    status.value = 'idle'
    activePath.value = null
    document.value = null
    error.value = null
    resetSaveState()
  }

  function resetSaveState() {
    draft.value = ''
    saveStatus.value = 'clean'
    saveError.value = null
  }

  return {
    status,
    activePath,
    document,
    error,
    draft,
    saveStatus,
    saveError,
    hasUnsavedChanges,
    canSave,
    openFile,
    retry,
    setDraft,
    save,
    retrySave,
    discardAndReload,
    clearFile,
  }
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

function toDocumentSaveError(reason: unknown): DocumentSaveError {
  if (reason instanceof MarkdownClientError) {
    return {
      code: reason.code,
      message: reason.message,
      retryable: reason.status === null || reason.status >= 500,
      currentHash: reason.currentHash,
    }
  }

  return {
    code: 'document_save_failed',
    message: 'Markdown 문서를 저장하지 못했습니다.',
    retryable: true,
    currentHash: null,
  }
}
