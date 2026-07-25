<script setup lang="ts">
import { computed, onBeforeUnmount } from 'vue'

import { renderMarkdown } from '@/services/markdownRenderer'

const props = defineProps<{
  source: string
}>()

const renderedHtml = computed(() => renderMarkdown(props.source))
const resetTimers = new Map<HTMLButtonElement, number>()

async function handlePreviewClick(event: MouseEvent) {
  const target = event.target
  if (!(target instanceof Element)) return

  const button = target.closest<HTMLButtonElement>('[data-copy-code]')
  const preview = event.currentTarget
  if (!button || !(preview instanceof HTMLElement) || !preview.contains(button)) return

  const code = button.closest('.markdown-code-block')?.querySelector('pre code')
  if (!code) return

  try {
    await writeClipboard(code.textContent ?? '')
    setCopyStatus(button, '복사됨', 'success')
  } catch {
    setCopyStatus(button, '복사 실패', 'error')
  }
}

async function writeClipboard(content: string) {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(content)
      return
    }
  } catch {
    writeClipboardFallback(content)
    return
  }

  writeClipboardFallback(content)
}

function writeClipboardFallback(content: string) {
  const textarea = document.createElement('textarea')
  textarea.value = content
  textarea.setAttribute('readonly', '')
  textarea.style.position = 'fixed'
  textarea.style.opacity = '0'
  document.body.append(textarea)
  textarea.select()
  textarea.setSelectionRange(0, textarea.value.length)

  try {
    if (!document.execCommand?.('copy')) throw new Error('clipboard copy failed')
  } finally {
    textarea.remove()
  }
}

function setCopyStatus(
  button: HTMLButtonElement,
  label: '복사됨' | '복사 실패',
  status: 'success' | 'error',
) {
  const previousTimer = resetTimers.get(button)
  if (previousTimer) window.clearTimeout(previousTimer)

  button.textContent = label
  button.dataset.copyState = status
  const timer = window.setTimeout(() => {
    if (button.isConnected) {
      button.textContent = '복사'
      delete button.dataset.copyState
    }
    resetTimers.delete(button)
  }, 2000)
  resetTimers.set(button, timer)
}

onBeforeUnmount(() => {
  for (const timer of resetTimers.values()) window.clearTimeout(timer)
  resetTimers.clear()
})
</script>

<template>
  <article
    class="markdown-preview"
    aria-label="Markdown 미리보기"
    v-html="renderedHtml"
    @click="handlePreviewClick"
  />
</template>

<style scoped>
.markdown-preview {
  min-height: min(60dvh, 44rem);
  padding: clamp(1.25rem, 4vw, 2.5rem);
  border: 1px solid var(--color-border);
  border-radius: 0.9rem;
  background: var(--color-surface);
  color: var(--color-text);
  font-size: 0.98rem;
  line-height: 1.75;
  overflow-wrap: anywhere;
}

.markdown-preview :deep(> :first-child) {
  margin-top: 0;
}

.markdown-preview :deep(> :last-child) {
  margin-bottom: 0;
}

.markdown-preview :deep(h1),
.markdown-preview :deep(h2),
.markdown-preview :deep(h3),
.markdown-preview :deep(h4) {
  margin: 1.8em 0 0.65em;
  letter-spacing: -0.025em;
  line-height: 1.3;
}

.markdown-preview :deep(h1) {
  padding-bottom: 0.4em;
  border-bottom: 1px solid var(--color-border);
  font-size: clamp(1.8rem, 5vw, 2.6rem);
}

.markdown-preview :deep(h2) {
  padding-bottom: 0.3em;
  border-bottom: 1px solid var(--color-border);
  font-size: clamp(1.4rem, 4vw, 2rem);
}

.markdown-preview :deep(h3) {
  font-size: 1.25rem;
}

.markdown-preview :deep(p),
.markdown-preview :deep(ul),
.markdown-preview :deep(ol),
.markdown-preview :deep(blockquote),
.markdown-preview :deep(pre),
.markdown-preview :deep(table) {
  margin: 0 0 1rem;
}

.markdown-preview :deep(ul),
.markdown-preview :deep(ol) {
  padding-left: 1.6rem;
}

.markdown-preview :deep(li + li) {
  margin-top: 0.3rem;
}

.markdown-preview :deep(a) {
  color: var(--color-accent);
  text-decoration-thickness: 0.08em;
  text-underline-offset: 0.18em;
}

.markdown-preview :deep(blockquote) {
  padding: 0.2rem 1rem;
  border-left: 0.25rem solid var(--color-accent);
  color: var(--color-text-muted);
}

.markdown-preview :deep(code) {
  padding: 0.12em 0.35em;
  border-radius: 0.3rem;
  background: var(--color-surface-muted);
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 0.9em;
}

.markdown-preview :deep(.markdown-code-block) {
  overflow: hidden;
  margin: 0 0 1rem;
  border: 1px solid color-mix(in srgb, var(--color-text) 75%, var(--color-border));
  border-radius: 0.65rem;
  background: var(--color-text);
}

.markdown-preview :deep(.markdown-code-block__toolbar) {
  display: flex;
  min-height: 2.75rem;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.35rem 0.45rem 0.35rem 0.85rem;
  border-bottom: 1px solid color-mix(in srgb, var(--color-background) 22%, transparent);
  color: color-mix(in srgb, var(--color-background) 72%, transparent);
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 0.72rem;
}

.markdown-preview :deep(.markdown-code-block__copy) {
  min-width: 3.8rem;
  min-height: 2rem;
  padding: 0 0.7rem;
  border: 1px solid color-mix(in srgb, var(--color-background) 28%, transparent);
  border-radius: 0.45rem;
  background: color-mix(in srgb, var(--color-background) 10%, transparent);
  color: var(--color-background);
  cursor: pointer;
  font: inherit;
  font-weight: 800;
}

.markdown-preview :deep(.markdown-code-block__copy:hover) {
  background: color-mix(in srgb, var(--color-background) 18%, transparent);
}

.markdown-preview :deep(.markdown-code-block__copy[data-copy-state='success']) {
  border-color: var(--color-success);
  color: color-mix(in srgb, var(--color-success) 45%, white);
}

.markdown-preview :deep(.markdown-code-block__copy[data-copy-state='error']) {
  border-color: var(--color-warning);
  color: color-mix(in srgb, var(--color-warning) 45%, white);
}

.markdown-preview :deep(.markdown-code-block pre) {
  overflow-x: auto;
  margin: 0;
  padding: 1rem;
  border-radius: 0;
  background: transparent;
  color: var(--color-background);
}

.markdown-preview :deep(pre code) {
  padding: 0;
  background: transparent;
  color: inherit;
}

.markdown-preview :deep(table) {
  display: block;
  width: 100%;
  overflow-x: auto;
  border-spacing: 0;
  border-collapse: collapse;
}

.markdown-preview :deep(th),
.markdown-preview :deep(td) {
  padding: 0.55rem 0.7rem;
  border: 1px solid var(--color-border);
  text-align: left;
}

.markdown-preview :deep(th) {
  background: var(--color-surface-muted);
}

.markdown-preview :deep(hr) {
  margin: 2rem 0;
  border: 0;
  border-top: 1px solid var(--color-border);
}

.markdown-preview :deep(img) {
  max-width: 100%;
  height: auto;
  border-radius: 0.5rem;
}

@media (max-width: 40rem) {
  .markdown-preview {
    padding: 1rem;
    border-right: 0;
    border-left: 0;
    border-radius: 0;
  }
}
</style>
