<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, type ComponentPublicInstance } from 'vue'

import type { SearchResult } from '@/models/search'
import {
  searchClient as defaultSearchClient,
  SearchClientError,
  type SearchClient,
} from '@/services/searchClient'

type SearchStatus = 'idle' | 'loading' | 'loaded' | 'error'

const props = defineProps<{ client?: SearchClient }>()
const emit = defineEmits<{ close: []; openFile: [path: string] }>()
const query = ref('')
const submittedQuery = ref('')
const status = ref<SearchStatus>('idle')
const results = ref<SearchResult[]>([])
const errorMessage = ref('')
const inputElement = ref<HTMLInputElement | null>(null)
const resultElements = new Map<number, HTMLElement>()
const focusedIndex = ref(-1)
let activeController: AbortController | null = null
let requestGeneration = 0

onMounted(() => inputElement.value?.focus())
onBeforeUnmount(() => activeController?.abort())

function setResultElement(index: number, value: Element | ComponentPublicInstance | null) {
  if (value instanceof HTMLElement) {
    resultElements.set(index, value)
  } else {
    resultElements.delete(index)
  }
}

async function submitSearch() {
  const normalizedQuery = query.value.trim()
  if (!normalizedQuery) {
    activeController?.abort()
    requestGeneration += 1
    submittedQuery.value = ''
    results.value = []
    errorMessage.value = ''
    focusedIndex.value = -1
    status.value = 'idle'
    return
  }

  await runSearch(normalizedQuery)
}

async function runSearch(normalizedQuery: string) {
  activeController?.abort()
  const controller = new AbortController()
  activeController = controller
  requestGeneration += 1
  const generation = requestGeneration

  submittedQuery.value = normalizedQuery
  results.value = []
  errorMessage.value = ''
  focusedIndex.value = -1
  status.value = 'loading'

  try {
    const response = await (props.client ?? defaultSearchClient).search(
      normalizedQuery,
      controller.signal,
    )
    if (generation !== requestGeneration) return

    results.value = response.results
    status.value = 'loaded'
  } catch (reason) {
    if (generation !== requestGeneration) return
    if (reason instanceof SearchClientError && reason.code === 'request_aborted') return

    errorMessage.value =
      reason instanceof SearchClientError ? reason.message : '검색 요청을 처리하지 못했습니다.'
    status.value = 'error'
  } finally {
    if (generation === requestGeneration) activeController = null
  }
}

function retry() {
  if (submittedQuery.value) void runSearch(submittedQuery.value)
}

async function focusResult(index: number) {
  if (results.value.length === 0) return
  focusedIndex.value = Math.min(Math.max(index, 0), results.value.length - 1)
  await nextTick()
  resultElements.get(focusedIndex.value)?.focus()
}

function handleInputKeydown(event: KeyboardEvent) {
  if (event.key !== 'ArrowDown' || results.value.length === 0) return
  event.preventDefault()
  void focusResult(0)
}

function handleResultKeydown(event: KeyboardEvent, index: number, path: string) {
  let nextIndex: number | null = null

  switch (event.key) {
    case 'ArrowDown':
      nextIndex = index + 1
      break
    case 'ArrowUp':
      nextIndex = index - 1
      break
    case 'Home':
      nextIndex = 0
      break
    case 'End':
      nextIndex = results.value.length - 1
      break
    case 'Enter':
      event.preventDefault()
      emit('openFile', path)
      return
    default:
      return
  }

  event.preventDefault()
  void focusResult(nextIndex)
}
</script>

<template>
  <section class="search-panel" aria-labelledby="search-panel-title">
    <header class="search-panel__header">
      <div>
        <p>Vault</p>
        <h2 id="search-panel-title">문서 검색</h2>
      </div>
      <button
        class="search-panel__close"
        type="button"
        aria-label="검색 패널 닫기"
        @click="emit('close')"
      >
        <span aria-hidden="true">×</span>
      </button>
    </header>

    <form class="search-form" role="search" @submit.prevent="submitSearch">
      <label for="vault-search-query">검색어</label>
      <div class="search-form__controls">
        <input
          id="vault-search-query"
          ref="inputElement"
          v-model="query"
          type="search"
          maxlength="512"
          autocomplete="off"
          placeholder="제목, 태그, 본문 검색"
          @keydown="handleInputKeydown"
        />
        <button type="submit" :disabled="status === 'loading'">검색</button>
      </div>
    </form>

    <div v-if="status === 'idle'" class="search-state" role="status">
      <span aria-hidden="true">⌕</span>
      <strong>검색어를 입력하세요.</strong>
      <p>Vault의 제목, 태그와 본문을 함께 검색합니다.</p>
    </div>

    <div v-else-if="status === 'loading'" class="search-state" role="status">
      <span class="search-spinner" aria-hidden="true" />
      <strong>검색 중입니다.</strong>
    </div>

    <div v-else-if="status === 'error'" class="search-state search-state--error" role="alert">
      <strong>검색하지 못했습니다.</strong>
      <p>{{ errorMessage }}</p>
      <button type="button" @click="retry">다시 시도</button>
    </div>

    <div v-else-if="results.length === 0" class="search-state" role="status">
      <span aria-hidden="true">◇</span>
      <strong>검색 결과가 없습니다.</strong>
      <p>“{{ submittedQuery }}”에 해당하는 문서를 찾지 못했습니다.</p>
    </div>

    <ul v-else class="search-results" role="listbox" aria-label="검색 결과">
      <li v-for="(result, index) in results" :key="result.path">
        <button
          :ref="(value) => setResultElement(index, value)"
          class="search-result"
          type="button"
          role="option"
          :tabindex="index === 0 ? 0 : -1"
          :aria-selected="focusedIndex === index"
          @click="emit('openFile', result.path)"
          @focus="focusedIndex = index"
          @keydown="handleResultKeydown($event, index, result.path)"
        >
          <span class="search-result__title">{{ result.title }}</span>
          <span class="search-result__path">{{ result.path }}</span>
          <span v-if="result.snippet" class="search-result__snippet">{{ result.snippet }}</span>
        </button>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.search-panel {
  display: flex;
  min-height: 0;
  flex-direction: column;
  background: var(--color-surface);
}

.search-panel__header {
  display: flex;
  min-height: 5rem;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem;
  border-bottom: 1px solid var(--color-border);
}

.search-panel__header p,
.search-panel__header h2 {
  margin: 0;
}

.search-panel__header p {
  color: var(--color-accent);
  font-size: 0.68rem;
  font-weight: 800;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.search-panel__header h2 {
  margin-top: 0.25rem;
  font-size: 1rem;
}

.search-panel__close {
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

.search-form {
  padding: 1rem;
  border-bottom: 1px solid var(--color-border);
}

.search-form label {
  display: block;
  margin-bottom: 0.45rem;
  color: var(--color-text-muted);
  font-size: 0.75rem;
  font-weight: 700;
}

.search-form__controls {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 0.5rem;
}

.search-form input,
.search-form button,
.search-state button {
  min-height: 2.75rem;
  border: 1px solid var(--color-border);
  border-radius: 0.7rem;
  font: inherit;
}

.search-form input {
  min-width: 0;
  padding: 0 0.8rem;
  background: var(--color-background);
  color: var(--color-text);
}

.search-form button,
.search-state button {
  padding: 0 0.9rem;
  background: var(--color-accent);
  color: var(--color-accent-contrast);
  cursor: pointer;
  font-weight: 750;
}

.search-form button:disabled {
  cursor: wait;
  opacity: 0.6;
}

.search-state {
  display: grid;
  min-height: 12rem;
  place-items: center;
  align-content: center;
  gap: 0.55rem;
  padding: 1.5rem;
  color: var(--color-text-muted);
  text-align: center;
}

.search-state strong {
  color: var(--color-text);
}

.search-state p {
  margin: 0;
  font-size: 0.82rem;
}

.search-state--error strong {
  color: var(--color-warning);
}

.search-results {
  min-height: 0;
  margin: 0;
  padding: 0.5rem;
  overflow-y: auto;
  list-style: none;
}

.search-result {
  display: grid;
  width: 100%;
  min-height: 4.5rem;
  gap: 0.3rem;
  padding: 0.8rem;
  border: 1px solid transparent;
  border-radius: 0.7rem;
  background: transparent;
  color: var(--color-text);
  cursor: pointer;
  font: inherit;
  text-align: left;
}

.search-result:hover,
.search-result:focus-visible {
  border-color: var(--color-border);
  background: var(--color-surface-muted);
}

.search-result__title {
  font-size: 0.9rem;
  font-weight: 800;
}

.search-result__path,
.search-result__snippet {
  overflow: hidden;
  color: var(--color-text-muted);
  font-size: 0.75rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.search-spinner {
  width: 1.2rem;
  height: 1.2rem;
  border: 2px solid var(--color-border);
  border-top-color: var(--color-accent);
  border-radius: 50%;
  animation: search-spin 700ms linear infinite;
}

@keyframes search-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .search-spinner {
    animation: none;
  }
}
</style>
