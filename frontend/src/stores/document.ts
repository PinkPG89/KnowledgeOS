import { defineStore } from 'pinia'
import { computed, ref, shallowRef } from 'vue'

import type {
  DocumentLoadError,
  DocumentLoadStatus,
  DocumentSaveError,
  DocumentSaveStatus,
  DraftBackupStatus,
  DraftRecoveryStatus,
  BrowserDraft,
  MarkdownDocument,
} from '@/models/markdown'
import { getBrowserDraftRepository, type DraftRepository } from '@/services/draftRepository'
import { MarkdownClientError, markdownClient, type MarkdownClient } from '@/services/markdownClient'

const DRAFT_PERSIST_DELAY_MS = 300

interface PendingDraftPersistence {
  repository: DraftRepository
  path: string
  draft: BrowserDraft | null
}

export const useDocumentStore = defineStore('document', () => {
  const status = ref<DocumentLoadStatus>('idle')
  const activePath = ref<string | null>(null)
  const document = shallowRef<MarkdownDocument | null>(null)
  const error = ref<DocumentLoadError | null>(null)
  const draft = ref('')
  const saveStatus = ref<DocumentSaveStatus>('clean')
  const saveError = ref<DocumentSaveError | null>(null)
  const recoveryStatus = ref<DraftRecoveryStatus>('none')
  const recoveredDraft = shallowRef<BrowserDraft | null>(null)
  const draftBackupStatus = ref<DraftBackupStatus>('idle')
  const draftBackupError = ref<string | null>(null)
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
  let activeDraftRepository: DraftRepository | undefined
  let draftBaseHash: string | null = null
  let pendingDraftPersistence: PendingDraftPersistence | null = null
  let draftPersistenceTimer: ReturnType<typeof setTimeout> | null = null

  async function openFile(
    path: string,
    client: MarkdownClient = markdownClient,
    repository: DraftRepository | undefined = getBrowserDraftRepository(),
  ): Promise<void> {
    requestGeneration += 1
    const generation = requestGeneration

    activeController?.abort()
    if (pendingDraftPersistence) {
      await flushDraftPersistence()
      if (generation !== requestGeneration) return
    }

    const controller = new AbortController()
    activeController = controller
    activeSave = null
    activeSaveToken = null
    activeDraftRepository = repository

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
      draftBaseHash = loadedDocument.hash
      await restoreBrowserDraft(loadedDocument, repository)
      if (generation !== requestGeneration) return
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
    return openFile(activePath.value, client, activeDraftRepository)
  }

  function setDraft(content: string) {
    if (!document.value) return

    draft.value = content
    if (content === document.value.content) {
      draftBaseHash = document.value.hash
      saveError.value = null
      saveStatus.value = 'clean'
      scheduleDraftRemoval(document.value.path)
      return
    }

    scheduleDraftBackup()
    if (saveStatus.value === 'saving' || saveStatus.value === 'conflict') return

    saveError.value = null
    saveStatus.value = 'dirty'
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
        draftBaseHash = savedDocument.hash
        if (draft.value === content) {
          draft.value = savedDocument.content
          saveStatus.value = 'clean'
          scheduleDraftRemoval(path)
        } else {
          saveStatus.value = 'dirty'
          scheduleDraftBackup()
        }
      } catch (reason) {
        if (generation !== requestGeneration || activePath.value !== path) return

        const nextError = toDocumentSaveError(reason)
        saveError.value = nextError
        saveStatus.value = nextError.code === 'write_conflict' ? 'conflict' : 'error'
        scheduleDraftBackup()
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

  async function resumeRecoveredDraft(): Promise<void> {
    if (!document.value || !recoveredDraft.value || recoveryStatus.value === 'none') return

    const recovery = recoveryStatus.value
    const browserDraft = recoveredDraft.value
    draft.value = browserDraft.content
    draftBaseHash = browserDraft.baseHash
    recoveredDraft.value = null
    recoveryStatus.value = 'none'

    if (recovery === 'conflict') {
      saveStatus.value = 'conflict'
      saveError.value = {
        code: 'draft_conflict',
        message: '브라우저 초안의 기준 hash와 서버 문서가 다릅니다.',
        retryable: false,
        currentHash: document.value.hash,
      }
      return
    }

    saveError.value = null
    saveStatus.value = browserDraft.content === document.value.content ? 'clean' : 'dirty'
  }

  async function discardRecoveredDraft(): Promise<void> {
    const path = recoveredDraft.value?.path ?? activePath.value
    if (!path) return

    if (!(await removeBrowserDraft(path))) return
    recoveredDraft.value = null
    recoveryStatus.value = 'none'
    if (document.value?.path === path) {
      draft.value = document.value.content
      draftBaseHash = document.value.hash
      saveStatus.value = 'clean'
      saveError.value = null
    }
  }

  async function discardAndReload(client: MarkdownClient = markdownClient): Promise<void> {
    if (!activePath.value) return Promise.resolve()
    const path = activePath.value
    if (!(await removeBrowserDraft(path))) return
    return openFile(path, client, activeDraftRepository)
  }

  function clearFile() {
    requestGeneration += 1
    activeController?.abort()
    activeController = null
    activeSave = null
    activeSaveToken = null
    activeDraftRepository = undefined
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
    recoveryStatus.value = 'none'
    recoveredDraft.value = null
    draftBackupStatus.value = 'idle'
    draftBackupError.value = null
    draftBaseHash = null
  }

  async function restoreBrowserDraft(
    loadedDocument: MarkdownDocument,
    repository: DraftRepository | undefined,
  ) {
    if (!repository) return

    try {
      const browserDraft = await repository.get(loadedDocument.path)
      if (!browserDraft) return

      if (browserDraft.content === loadedDocument.content) {
        await repository.remove(loadedDocument.path)
        return
      }

      recoveredDraft.value = browserDraft
      recoveryStatus.value =
        browserDraft.baseHash === loadedDocument.hash ? 'available' : 'conflict'
      draftBackupStatus.value = 'saved'
    } catch {
      draftBackupStatus.value = 'error'
      draftBackupError.value = '브라우저 초안을 불러오지 못했습니다.'
    }
  }

  function scheduleDraftBackup() {
    if (!document.value || !activeDraftRepository || !draftBaseHash) return

    scheduleDraftPersistence({
      repository: activeDraftRepository,
      path: document.value.path,
      draft: {
        path: document.value.path,
        baseHash: draftBaseHash,
        content: draft.value,
        updatedAt: new Date().toISOString(),
      },
    })
  }

  function scheduleDraftRemoval(path: string) {
    if (!activeDraftRepository) return
    scheduleDraftPersistence({
      repository: activeDraftRepository,
      path,
      draft: null,
    })
  }

  function scheduleDraftPersistence(persistence: PendingDraftPersistence) {
    if (draftPersistenceTimer) clearTimeout(draftPersistenceTimer)
    pendingDraftPersistence = persistence
    draftBackupStatus.value = persistence.draft ? 'pending' : 'idle'
    draftBackupError.value = null
    draftPersistenceTimer = setTimeout(() => {
      draftPersistenceTimer = null
      void flushDraftPersistence()
    }, DRAFT_PERSIST_DELAY_MS)
  }

  async function flushDraftPersistence(): Promise<void> {
    if (draftPersistenceTimer) {
      clearTimeout(draftPersistenceTimer)
      draftPersistenceTimer = null
    }

    const persistence = pendingDraftPersistence
    pendingDraftPersistence = null
    if (!persistence) return

    try {
      if (persistence.draft) {
        await persistence.repository.put(persistence.draft)
      } else {
        await persistence.repository.remove(persistence.path)
      }
      if (!pendingDraftPersistence && activePath.value === persistence.path) {
        draftBackupStatus.value = persistence.draft ? 'saved' : 'idle'
      }
    } catch {
      if (activePath.value === persistence.path) {
        draftBackupStatus.value = 'error'
        draftBackupError.value = '브라우저 초안을 저장하지 못했습니다.'
      }
    }
  }

  async function removeBrowserDraft(path: string): Promise<boolean> {
    if (draftPersistenceTimer) {
      clearTimeout(draftPersistenceTimer)
      draftPersistenceTimer = null
    }
    if (pendingDraftPersistence?.path === path) pendingDraftPersistence = null
    if (!activeDraftRepository) return true

    try {
      await activeDraftRepository.remove(path)
      draftBackupStatus.value = 'idle'
      draftBackupError.value = null
      return true
    } catch {
      draftBackupStatus.value = 'error'
      draftBackupError.value = '브라우저 초안을 삭제하지 못했습니다.'
      return false
    }
  }

  return {
    status,
    activePath,
    document,
    error,
    draft,
    saveStatus,
    saveError,
    recoveryStatus,
    recoveredDraft,
    draftBackupStatus,
    draftBackupError,
    hasUnsavedChanges,
    canSave,
    openFile,
    retry,
    setDraft,
    save,
    retrySave,
    resumeRecoveredDraft,
    discardRecoveredDraft,
    discardAndReload,
    flushDraftPersistence,
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
