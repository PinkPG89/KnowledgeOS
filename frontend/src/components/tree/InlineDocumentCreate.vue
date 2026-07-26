<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'

import {
  markdownCreateClient as defaultMarkdownCreateClient,
  MarkdownClientError,
  type MarkdownCreateClient,
} from '@/services/markdownClient'
import { isMarkdownPath } from '@/utils/canonicalPath'

const props = defineProps<{
  client?: MarkdownCreateClient
  parentPath: string
}>()
const emit = defineEmits<{ cancel: []; created: [path: string] }>()
const fileName = ref('')
const creating = ref(false)
const errorMessage = ref('')
const inputElement = ref<HTMLInputElement | null>(null)
let activeController: AbortController | null = null

onMounted(() => inputElement.value?.focus())
onBeforeUnmount(() => activeController?.abort())

async function submitCreate() {
  if (creating.value) return

  const documentPath = normalizeDocumentPath(props.parentPath, fileName.value)
  if (!documentPath) {
    errorMessage.value = '파일명만 입력하세요. 숨김 파일과 경로 구분자는 사용할 수 없습니다.'
    return
  }

  const controller = new AbortController()
  activeController = controller
  creating.value = true
  errorMessage.value = ''

  try {
    const document = await (props.client ?? defaultMarkdownCreateClient).createFile(
      documentPath,
      '',
      controller.signal,
    )
    emit('created', document.path)
  } catch (reason) {
    if (reason instanceof MarkdownClientError && reason.code === 'request_aborted') return
    errorMessage.value =
      reason instanceof MarkdownClientError ? reason.message : '새 문서를 생성하지 못했습니다.'
  } finally {
    if (activeController === controller) activeController = null
    creating.value = false
  }
}

function normalizeDocumentPath(parentPath: string, rawFileName: string): string | null {
  const trimmed = rawFileName.trim()
  if (!trimmed || trimmed.includes('/') || trimmed.includes('\\')) return null

  const markdownFileName = trimmed.toLowerCase().endsWith('.md')
    ? `${trimmed.slice(0, -3)}.md`
    : `${trimmed}.md`
  const path = parentPath ? `${parentPath}/${markdownFileName}` : markdownFileName
  return isMarkdownPath(path) ? path : null
}
</script>

<template>
  <form class="inline-create" @submit.prevent="submitCreate">
    <label for="new-document-file-name">
      <span class="inline-create__location">{{ parentPath ? `${parentPath}/` : 'Vault /' }}</span>
      <span class="inline-create__label">새 문서 파일명</span>
    </label>
    <div class="inline-create__controls">
      <input
        id="new-document-file-name"
        ref="inputElement"
        v-model="fileName"
        type="text"
        maxlength="255"
        autocomplete="off"
        spellcheck="false"
        placeholder="새 문서"
        :disabled="creating"
        @keydown.esc.stop.prevent="emit('cancel')"
      />
      <button type="submit" :disabled="creating" aria-label="새 문서 생성">
        {{ creating ? '…' : '✓' }}
      </button>
      <button type="button" :disabled="creating" aria-label="새 문서 취소" @click="emit('cancel')">
        ×
      </button>
    </div>
    <p v-if="errorMessage" role="alert">{{ errorMessage }}</p>
  </form>
</template>

<style scoped>
.inline-create {
  display: grid;
  width: 100%;
  gap: 0.35rem;
}

.inline-create label {
  display: flex;
  min-width: 0;
  justify-content: space-between;
  gap: 0.5rem;
  color: var(--color-text-muted);
  font-size: 0.68rem;
}

.inline-create__location {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.inline-create__label {
  flex: 0 0 auto;
  font-weight: 750;
}

.inline-create__controls {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 2.75rem 2.75rem;
  gap: 0.3rem;
}

.inline-create input,
.inline-create button {
  min-height: 2.75rem;
  border: 1px solid var(--color-border);
  border-radius: 0.6rem;
  font: inherit;
}

.inline-create input {
  min-width: 0;
  padding: 0 0.65rem;
  background: var(--color-background);
  color: var(--color-text);
}

.inline-create button {
  background: var(--color-surface);
  color: var(--color-text);
  cursor: pointer;
  font-weight: 800;
}

.inline-create button[type='submit'] {
  border-color: var(--color-accent);
  background: var(--color-accent);
  color: var(--color-accent-contrast);
}

.inline-create button:disabled {
  cursor: wait;
  opacity: 0.6;
}

.inline-create p {
  margin: 0;
  color: var(--color-warning);
  font-size: 0.7rem;
  line-height: 1.4;
}
</style>
