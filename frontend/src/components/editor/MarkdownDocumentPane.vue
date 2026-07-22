<script setup lang="ts">
import { useDocumentStore } from '@/stores/document'

const documentState = useDocumentStore()
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
            <p>Read-only Markdown</p>
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
        <pre>{{ documentState.document.content }}</pre>
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

.document-content pre {
  margin: 2rem 0;
  overflow-wrap: anywhere;
  color: var(--color-text);
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 0.9rem;
  line-height: 1.75;
  white-space: pre-wrap;
}

.document-content footer {
  overflow: hidden;
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
  .document-content__header {
    display: grid;
    align-items: start;
  }

  .document-content__header dl {
    width: 100%;
  }
}

@media (prefers-reduced-motion: reduce) {
  .document-spinner {
    animation: none;
  }
}
</style>
