<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'

import {
  markdownCreateClient as defaultMarkdownCreateClient,
  MarkdownClientError,
  type MarkdownCreateClient,
} from '@/services/markdownClient'
import { isMarkdownPath } from '@/utils/canonicalPath'

type CreateStatus = 'idle' | 'creating' | 'error'

const props = defineProps<{ client?: MarkdownCreateClient }>()
const emit = defineEmits<{ close: []; created: [path: string] }>()
const path = ref('')
const title = ref('')
const status = ref<CreateStatus>('idle')
const errorMessage = ref('')
const pathInput = ref<HTMLInputElement | null>(null)
let activeController: AbortController | null = null
let pendingCreate: Promise<void> | null = null
let activeCreateToken: symbol | null = null
let lastRequest: { path: string; content: string } | null = null

onMounted(() => pathInput.value?.focus())
onBeforeUnmount(() => activeController?.abort())

function submitCreate() {
  if (pendingCreate) return pendingCreate

  const normalizedPath = path.value.trim()
  if (!normalizedPath) {
    lastRequest = null
    status.value = 'error'
    errorMessage.value = '새 문서의 경로를 입력하세요.'
    return Promise.resolve()
  }
  if (!isMarkdownPath(normalizedPath)) {
    lastRequest = null
    status.value = 'error'
    errorMessage.value = '올바른 Markdown 경로를 입력하세요. 경로는 lowercase .md로 끝나야 합니다.'
    return Promise.resolve()
  }

  const normalizedTitle = title.value.trim()
  const content = normalizedTitle ? `# ${normalizedTitle}\n` : ''
  lastRequest = { path: normalizedPath, content }
  return runCreate(normalizedPath, content)
}

function retry() {
  if (!lastRequest || pendingCreate) return
  void runCreate(lastRequest.path, lastRequest.content)
}

function runCreate(documentPath: string, content: string): Promise<void> {
  activeController?.abort()
  const controller = new AbortController()
  activeController = controller
  const createToken = Symbol('document-create')
  activeCreateToken = createToken
  status.value = 'creating'
  errorMessage.value = ''

  const operation = (async () => {
    try {
      const document = await (props.client ?? defaultMarkdownCreateClient).createFile(
        documentPath,
        content,
        controller.signal,
      )
      emit('created', document.path)
    } catch (reason) {
      if (reason instanceof MarkdownClientError && reason.code === 'request_aborted') return

      errorMessage.value =
        reason instanceof MarkdownClientError ? reason.message : '새 문서를 생성하지 못했습니다.'
      status.value = 'error'
    } finally {
      if (activeController === controller) activeController = null
      if (activeCreateToken === createToken) {
        activeCreateToken = null
        pendingCreate = null
      }
    }
  })()

  pendingCreate = operation
  return operation
}
</script>

<template>
  <section class="create-panel" aria-labelledby="create-panel-title">
    <header class="create-panel__header">
      <div>
        <p>Vault</p>
        <h2 id="create-panel-title">새 문서</h2>
      </div>
      <button
        class="create-panel__close"
        type="button"
        aria-label="새 문서 패널 닫기"
        @click="emit('close')"
      >
        <span aria-hidden="true">×</span>
      </button>
    </header>

    <form class="create-form" @submit.prevent="submitCreate">
      <div class="create-field">
        <label for="new-document-path">Markdown 경로</label>
        <input
          id="new-document-path"
          ref="pathInput"
          v-model="path"
          name="path"
          type="text"
          maxlength="1024"
          autocomplete="off"
          spellcheck="false"
          placeholder="notes/new-document.md"
          :disabled="status === 'creating'"
        />
        <p>Vault 기준 상대 경로를 입력하세요. 부모 폴더는 미리 존재해야 합니다.</p>
      </div>

      <div class="create-field">
        <label for="new-document-title">문서 제목 <span>(선택)</span></label>
        <input
          id="new-document-title"
          v-model="title"
          name="title"
          type="text"
          maxlength="512"
          autocomplete="off"
          placeholder="새 문서"
          :disabled="status === 'creating'"
        />
        <p>입력하면 첫 줄을 H1 제목으로 생성합니다.</p>
      </div>

      <div v-if="status === 'creating'" class="create-state" role="status">
        <span class="create-spinner" aria-hidden="true" />
        <strong>문서를 생성 중입니다.</strong>
      </div>

      <div v-else-if="status === 'error'" class="create-state create-state--error" role="alert">
        <strong>문서를 생성하지 못했습니다.</strong>
        <p>{{ errorMessage }}</p>
        <button v-if="lastRequest" type="button" @click="retry">다시 시도</button>
      </div>

      <footer class="create-form__actions">
        <button type="button" class="create-button create-button--secondary" @click="emit('close')">
          취소
        </button>
        <button
          type="submit"
          class="create-button create-button--primary"
          :disabled="status === 'creating'"
        >
          문서 생성
        </button>
      </footer>
    </form>
  </section>
</template>

<style scoped>
.create-panel {
  display: flex;
  min-height: 0;
  flex-direction: column;
  background: var(--color-surface);
}

.create-panel__header {
  display: flex;
  min-height: 5rem;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem;
  border-bottom: 1px solid var(--color-border);
}

.create-panel__header p,
.create-panel__header h2 {
  margin: 0;
}

.create-panel__header p {
  color: var(--color-accent);
  font-size: 0.68rem;
  font-weight: 800;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.create-panel__header h2 {
  margin-top: 0.25rem;
  font-size: 1rem;
}

.create-panel__close {
  display: grid;
  width: 2.75rem;
  height: 2.75rem;
  place-items: center;
  border: 1px solid var(--color-border);
  border-radius: 0.8rem;
  background: var(--color-surface);
  color: var(--color-text);
  cursor: pointer;
  font: inherit;
  font-size: 1.15rem;
}

.create-form {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  gap: 1.1rem;
  padding: 1rem;
  overflow-y: auto;
}

.create-field label {
  display: block;
  margin-bottom: 0.45rem;
  font-size: 0.78rem;
  font-weight: 800;
}

.create-field label span,
.create-field p {
  color: var(--color-text-muted);
  font-weight: 500;
}

.create-field input {
  width: 100%;
  min-height: 2.75rem;
  padding: 0 0.8rem;
  border: 1px solid var(--color-border);
  border-radius: 0.7rem;
  background: var(--color-background);
  color: var(--color-text);
  font: inherit;
}

.create-field p {
  margin: 0.4rem 0 0;
  font-size: 0.72rem;
  line-height: 1.45;
}

.create-state {
  display: grid;
  min-height: 8rem;
  place-items: center;
  align-content: center;
  gap: 0.55rem;
  padding: 1rem;
  border: 1px solid var(--color-border);
  border-radius: 0.8rem;
  color: var(--color-text-muted);
  text-align: center;
}

.create-state p {
  margin: 0;
  font-size: 0.8rem;
}

.create-state--error strong {
  color: var(--color-warning);
}

.create-state button,
.create-button {
  min-height: 2.75rem;
  padding: 0 1rem;
  border: 1px solid var(--color-border);
  border-radius: 0.7rem;
  cursor: pointer;
  font: inherit;
  font-weight: 750;
}

.create-state button,
.create-button--primary {
  border-color: var(--color-accent);
  background: var(--color-accent);
  color: var(--color-accent-contrast);
}

.create-button--secondary {
  background: var(--color-surface);
  color: var(--color-text);
}

.create-button:disabled {
  cursor: wait;
  opacity: 0.6;
}

.create-form__actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.6rem;
  margin-top: auto;
  padding-top: 1rem;
  border-top: 1px solid var(--color-border);
}

.create-spinner {
  width: 1.2rem;
  height: 1.2rem;
  border: 2px solid var(--color-border);
  border-top-color: var(--color-accent);
  border-radius: 50%;
  animation: create-spin 700ms linear infinite;
}

@keyframes create-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .create-spinner {
    animation: none;
  }
}
</style>
