<script setup lang="ts">
import { computed } from 'vue'

import { renderMarkdown } from '@/services/markdownRenderer'

const props = defineProps<{
  source: string
}>()

const renderedHtml = computed(() => renderMarkdown(props.source))
</script>

<template>
  <article class="markdown-preview" aria-label="Markdown 미리보기" v-html="renderedHtml" />
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

.markdown-preview :deep(pre) {
  overflow-x: auto;
  padding: 1rem;
  border-radius: 0.65rem;
  background: var(--color-text);
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
