<script setup lang="ts">
defineProps<{
  createDisabled: boolean
  refreshDisabled: boolean
}>()
const emit = defineEmits<{ create: []; refresh: [] }>()
</script>

<template>
  <div class="file-tree__toolbar">
    <span>Markdown files</span>
    <div class="file-tree__actions">
      <button
        type="button"
        aria-label="선택 위치에 새 문서"
        :disabled="createDisabled"
        @click="emit('create')"
      >
        <span aria-hidden="true">＋</span>
      </button>
      <button
        type="button"
        aria-label="파일 트리 새로고침"
        :disabled="refreshDisabled"
        @click="emit('refresh')"
      >
        <span aria-hidden="true">↻</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.file-tree__toolbar {
  display: flex;
  min-height: 2.75rem;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0 0.65rem 0 1rem;
  border-bottom: 1px solid var(--color-border);
  color: var(--color-text-muted);
  font-size: 0.68rem;
  font-weight: 800;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.file-tree__actions {
  display: flex;
  align-items: center;
}

.file-tree__toolbar button {
  min-width: 2.75rem;
  min-height: 2.75rem;
  border: 0;
  border-radius: 0.65rem;
  background: transparent;
  color: var(--color-text);
  cursor: pointer;
  font: inherit;
  font-weight: 750;
}

.file-tree__toolbar button:hover {
  background: var(--color-surface-muted);
}

.file-tree__toolbar button:disabled {
  cursor: wait;
  opacity: 0.55;
}
</style>
