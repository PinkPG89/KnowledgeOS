<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import MarkdownPreview from '@/components/editor/MarkdownPreview.vue'
import { useDocumentStore } from '@/stores/document'

const MarkdownEditorSpike = defineAsyncComponent(
  () => import('@/components/editor/MarkdownEditorSpike.vue'),
)
const documentState = useDocumentStore()
const compositionActive = ref(false)
const documentMode = ref<'edit' | 'preview'>('preview')

watch(
  () => documentState.activePath,
  () => {
    compositionActive.value = false
  },
)

const saveStatusLabel = computed(() => {
  switch (documentState.saveStatus) {
    case 'dirty':
      return '저장되지 않은 변경'
    case 'saving':
      return '저장 중'
    case 'conflict':
      return '외부 변경 충돌'
    case 'error':
      return '저장 실패'
    default:
      return '저장됨'
  }
})

function handleKeyboardSave(event: KeyboardEvent) {
  if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== 's') return
  event.preventDefault()
  if (compositionActive.value) return
  void documentState.save()
}

function handleBeforeUnload(event: BeforeUnloadEvent) {
  if (!documentState.hasUnsavedChanges) return
  event.preventDefault()
  event.returnValue = ''
}

function discardConflictDraft() {
  if (!window.confirm('현재 초안을 버리고 서버 버전을 다시 불러오시겠습니까?')) return
  void documentState.discardAndReload()
}

onMounted(() => {
  window.addEventListener('keydown', handleKeyboardSave)
  window.addEventListener('beforeunload', handleBeforeUnload)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeyboardSave)
  window.removeEventListener('beforeunload', handleBeforeUnload)
})
</script>

<template>
  <section class="document-pane" aria-live="polite">
    <template v-if="documentState.status === 'idle'">
      <div class="document-state">
        <p class="document-state__eyebrow">KnowledgeOS</p>
        <h1 id="editor-title">집중할 수 있는 Markdown 작업공간</h1>
        <p>파일 트리에서 Markdown 문서를 선택하면 원문을 안전하게 불러옵니다.</p>
      </div>
    </template>

    <template v-else-if="documentState.status === 'loading'">
      <div class="document-path" aria-label="불러오는 문서 경로">
        {{ documentState.activePath }}
      </div>
      <div class="document-state" role="status">
        <span class="document-spinner" aria-hidden="true" />
        <h1 id="editor-title">문서를 불러오는 중입니다.</h1>
      </div>
    </template>

    <template v-else-if="documentState.status === 'error'">
      <div class="document-path" aria-label="오류가 발생한 문서 경로">
        {{ documentState.activePath }}
      </div>
      <div class="document-state document-state--error" role="alert">
        <p class="document-state__eyebrow">{{ documentState.error?.code }}</p>
        <h1 id="editor-title">문서를 열지 못했습니다.</h1>
        <p>{{ documentState.error?.message }}</p>
        <button v-if="documentState.error?.retryable" type="button" @click="documentState.retry()">
          다시 시도
        </button>
      </div>
    </template>

    <template v-else-if="documentState.document">
      <div class="document-path" aria-label="현재 문서 경로">{{ documentState.document.path }}</div>
      <article class="document-content" aria-labelledby="editor-title">
        <header class="document-content__header">
          <div>
            <p>Markdown Document</p>
            <h1 id="editor-title">
              {{ documentState.document.path.split('/').slice(-1)[0] }}
            </h1>
          </div>
          <dl>
            <div>
              <dt>크기</dt>
              <dd>{{ documentState.document.size }} bytes</dd>
            </div>
            <div>
              <dt>수정</dt>
              <dd>{{ documentState.document.modifiedAt }}</dd>
            </div>
          </dl>
        </header>
        <div class="document-content__controls">
          <p
            class="document-content__notice"
            :data-save-status="documentState.saveStatus"
            role="status"
          >
            <span>{{ saveStatusLabel }}</span>
            <span v-if="compositionActive">한글 입력 조합 중</span>
            <span v-else-if="documentMode === 'preview'">Markdown 미리보기</span>
          </p>
          <div class="document-content__actions">
            <button
              class="save-button"
              type="button"
              :disabled="!documentState.canSave || compositionActive"
              @click="documentState.save()"
            >
              {{ documentState.saveStatus === 'saving' ? '저장 중…' : '저장' }}
            </button>
            <div class="document-mode" role="group" aria-label="문서 표시 방식">
              <button
                type="button"
                :aria-pressed="documentMode === 'preview'"
                @click="documentMode = 'preview'"
              >
                미리보기
              </button>
              <button
                type="button"
                :aria-pressed="documentMode === 'edit'"
                @click="documentMode = 'edit'"
              >
                편집
              </button>
            </div>
          </div>
        </div>
        <div
          v-if="documentState.saveStatus === 'conflict'"
          class="save-feedback save-feedback--conflict"
          role="alert"
        >
          <div>
            <strong>서버의 문서가 먼저 변경되었습니다.</strong>
            <p>현재 초안은 화면에 보존됩니다. 자동으로 덮어쓰지 않습니다.</p>
          </div>
          <button type="button" @click="discardConflictDraft">초안 버리고 다시 불러오기</button>
        </div>
        <div
          v-else-if="documentState.saveStatus === 'error'"
          class="save-feedback save-feedback--error"
          role="alert"
        >
          <div>
            <strong>{{ documentState.saveError?.code }}</strong>
            <p>{{ documentState.saveError?.message }}</p>
          </div>
          <button
            v-if="documentState.saveError?.retryable"
            type="button"
            @click="documentState.retrySave()"
          >
            저장 다시 시도
          </button>
        </div>
        <MarkdownEditorSpike
          v-if="documentMode === 'edit'"
          :model-value="documentState.draft"
          :aria-label="`${documentState.document.path} Markdown 편집기`"
          @update:model-value="documentState.setDraft"
          @composition-change="compositionActive = $event"
        />
        <MarkdownPreview v-else :source="documentState.draft" />
        <footer>Hash · {{ documentState.document.hash }}</footer>
      </article>
    </template>
  </section>
</template>

<style scoped>
.document-pane {
  min-height: 100%;
}

.document-path {
  position: sticky;
  z-index: 2;
  top: 0;
  overflow: hidden;
  padding: 0.7rem 1rem;
  border-bottom: 1px solid var(--color-border);
  background: color-mix(in srgb, var(--color-background) 92%, transparent);
  color: var(--color-text-muted);
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 0.75rem;
  text-overflow: ellipsis;
  white-space: nowrap;
  backdrop-filter: blur(0.75rem);
}

.document-state {
  display: grid;
  width: min(48rem, 100%);
  min-height: calc(100dvh - 4rem);
  align-content: center;
  justify-items: start;
  margin: 0 auto;
  padding: clamp(2.5rem, 7vw, 6rem) clamp(1rem, 5vw, 4rem);
}

.document-path + .document-state {
  min-height: calc(100dvh - 6.4rem);
}

.document-state__eyebrow,
.document-content__header p {
  margin: 0 0 0.8rem;
  color: var(--color-accent);
  font-size: 0.72rem;
  font-weight: 800;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.document-state h1 {
  max-width: 38rem;
  margin: 0;
  font-size: clamp(2rem, 5vw, 4.5rem);
  letter-spacing: -0.055em;
  line-height: 1.02;
}

.document-state > p:not(.document-state__eyebrow) {
  max-width: 38rem;
  margin: 1.25rem 0 0;
  color: var(--color-text-muted);
  line-height: 1.75;
}

.document-state button {
  min-height: 2.75rem;
  margin-top: 1.5rem;
  padding: 0 1rem;
  border: 0;
  border-radius: 0.75rem;
  background: var(--color-accent);
  color: var(--color-accent-contrast);
  cursor: pointer;
  font: inherit;
  font-weight: 800;
}

.document-spinner {
  width: 1.75rem;
  height: 1.75rem;
  margin-bottom: 1.5rem;
  border: 3px solid var(--color-border);
  border-top-color: var(--color-accent);
  border-radius: 50%;
  animation: spin 700ms linear infinite;
}

.document-content {
  width: min(58rem, 100%);
  margin: 0 auto;
  padding: clamp(1.5rem, 5vw, 4rem) clamp(1rem, 5vw, 4rem) 5rem;
}

.document-content__header {
  display: flex;
  align-items: end;
  justify-content: space-between;
  gap: 2rem;
  padding-bottom: 1.5rem;
  border-bottom: 1px solid var(--color-border);
}

.document-content__header h1 {
  margin: 0;
  overflow-wrap: anywhere;
  font-size: clamp(1.6rem, 4vw, 3rem);
  letter-spacing: -0.04em;
}

.document-content__header dl {
  display: grid;
  flex: 0 0 auto;
  gap: 0.35rem;
  margin: 0;
  color: var(--color-text-muted);
  font-size: 0.72rem;
}

.document-content__header dl div {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
}

.document-content__header dd {
  margin: 0;
  color: var(--color-text);
}

.document-content__controls {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  margin: 1rem 0;
}

.document-content__notice {
  display: flex;
  flex: 1 1 auto;
  justify-content: space-between;
  gap: 1rem;
  margin: 0;
  padding: 0.7rem 0.85rem;
  border: 1px solid color-mix(in srgb, var(--color-warning) 35%, var(--color-border));
  border-radius: 0.75rem;
  background: color-mix(in srgb, var(--color-warning) 8%, var(--color-surface));
  color: var(--color-text-muted);
  font-size: 0.78rem;
  line-height: 1.5;
}

.document-content__notice > span:first-child {
  color: var(--color-text);
  font-weight: 800;
}

.document-content__notice[data-save-status='dirty'],
.document-content__notice[data-save-status='saving'] {
  border-color: color-mix(in srgb, var(--color-warning) 45%, var(--color-border));
}

.document-content__notice[data-save-status='conflict'],
.document-content__notice[data-save-status='error'] {
  border-color: color-mix(in srgb, var(--color-danger) 45%, var(--color-border));
  background: color-mix(in srgb, var(--color-danger) 8%, var(--color-surface));
}

.document-content__notice span:last-child:not(:first-child) {
  flex: 0 0 auto;
  color: var(--color-warning);
  font-weight: 800;
}

.document-content__actions {
  display: flex;
  flex: 0 0 auto;
  gap: 0.5rem;
}

.save-button,
.save-feedback button {
  min-height: 2.8rem;
  padding: 0 1rem;
  border: 0;
  border-radius: 0.75rem;
  background: var(--color-accent);
  color: var(--color-accent-contrast);
  cursor: pointer;
  font: inherit;
  font-size: 0.78rem;
  font-weight: 800;
}

.save-button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.save-feedback {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  margin: 0 0 1rem;
  padding: 1rem;
  border: 1px solid var(--color-border);
  border-radius: 0.85rem;
  background: var(--color-surface);
}

.save-feedback--conflict,
.save-feedback--error {
  border-color: color-mix(in srgb, var(--color-danger) 45%, var(--color-border));
}

.save-feedback strong {
  color: var(--color-danger);
}

.save-feedback p {
  margin: 0.35rem 0 0;
  color: var(--color-text-muted);
  font-size: 0.8rem;
  line-height: 1.5;
}

.document-mode {
  display: flex;
  flex: 0 0 auto;
  padding: 0.2rem;
  border: 1px solid var(--color-border);
  border-radius: 0.75rem;
  background: var(--color-surface-muted);
}

.document-mode button {
  min-height: 2.4rem;
  padding: 0 0.8rem;
  border: 0;
  border-radius: 0.55rem;
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  font: inherit;
  font-size: 0.78rem;
  font-weight: 800;
}

.document-mode button[aria-pressed='true'] {
  background: var(--color-surface);
  color: var(--color-accent);
  box-shadow: 0 1px 3px rgb(23 33 26 / 12%);
}

.document-content footer {
  overflow: hidden;
  margin-top: 1rem;
  padding-top: 1rem;
  border-top: 1px solid var(--color-border);
  color: var(--color-text-muted);
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 0.68rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 40rem) {
  .document-content {
    padding-right: 0;
    padding-left: 0;
  }

  .document-content__header {
    display: grid;
    align-items: start;
    padding-right: 1rem;
    padding-left: 1rem;
  }

  .document-content__header dl {
    width: 100%;
  }

  .document-content__controls {
    display: grid;
    margin-right: 1rem;
    margin-left: 1rem;
  }

  .document-content__actions,
  .document-mode {
    width: 100%;
  }

  .save-button {
    flex: 0 0 auto;
  }

  .document-mode button {
    flex: 1 1 50%;
    min-height: 2.75rem;
  }

  .save-feedback {
    display: grid;
    margin-right: 1rem;
    margin-left: 1rem;
  }

  .save-feedback button {
    width: 100%;
  }

  .document-content footer {
    margin-right: 1rem;
    margin-left: 1rem;
  }
}

@media (prefers-reduced-motion: reduce) {
  .document-spinner {
    animation: none;
  }
}
</style>
