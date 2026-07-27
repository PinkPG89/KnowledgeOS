<script setup lang="ts">
import { computed, ref, type CSSProperties } from 'vue'

import type { TreeDirectoryState } from '@/models/tree'
import type { VisibleTreeItem } from '@/models/treeView'

const props = defineProps<{
  directoryState?: TreeDirectoryState
  item: VisibleTreeItem
  selected: boolean
  tabIndex: 0 | -1
}>()
const emit = defineEmits<{
  activate: []
  focus: []
  keydown: [event: KeyboardEvent]
}>()
const rowElement = ref<HTMLElement | null>(null)
const rowStyle = computed(() => ({ '--tree-depth': props.item.depth }) as CSSProperties)

defineExpose({
  focus: () => rowElement.value?.focus(),
})
</script>

<template>
  <li
    ref="rowElement"
    class="tree-item"
    :class="{ 'tree-item--selected': selected }"
    role="treeitem"
    :tabindex="tabIndex"
    :aria-level="item.depth"
    :aria-posinset="item.position"
    :aria-setsize="item.setSize"
    :aria-expanded="item.node.kind === 'directory' ? directoryState?.expanded : undefined"
    :aria-selected="item.node.kind === 'file' ? selected : undefined"
    :aria-busy="item.node.kind === 'directory' && directoryState?.loadStatus === 'loading'"
    :style="rowStyle"
    @click="emit('activate')"
    @focus="emit('focus')"
    @keydown="emit('keydown', $event)"
  >
    <span v-if="item.node.kind === 'directory'" class="tree-item__disclosure" aria-hidden="true">
      {{ directoryState?.expanded ? '▾' : '▸' }}
    </span>
    <span v-else class="tree-item__disclosure" aria-hidden="true">·</span>
    <span class="tree-item__icon" aria-hidden="true">{{
      item.node.kind === 'directory' ? '□' : '≡'
    }}</span>
    <span class="tree-item__name" :title="item.node.name">{{ item.node.name }}</span>
    <span v-if="directoryState?.loadStatus === 'loading'" class="tree-spinner" aria-hidden="true" />
  </li>
</template>

<style scoped>
.tree-item {
  display: grid;
  grid-template-columns: 1rem 1.15rem minmax(0, 1fr) auto;
  min-height: 2.75rem;
  align-items: center;
  gap: 0.35rem;
  padding: 0.25rem 0.65rem 0.25rem calc(0.35rem + (var(--tree-depth) - 1) * 1rem);
  border-radius: 0.65rem;
  color: var(--color-text);
  cursor: default;
  font-size: 0.84rem;
  user-select: none;
}

.tree-item:hover,
.tree-item:focus-visible {
  background: var(--color-surface-muted);
}

.tree-item--selected {
  background: color-mix(in srgb, var(--color-accent) 13%, var(--color-surface));
  color: var(--color-accent);
  font-weight: 720;
}

.tree-item__disclosure,
.tree-item__icon {
  display: grid;
  place-items: center;
  color: var(--color-text-muted);
}

.tree-item__name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tree-spinner {
  width: 0.85rem;
  height: 0.85rem;
  border: 2px solid color-mix(in srgb, var(--color-accent) 25%, transparent);
  border-top-color: var(--color-accent);
  border-radius: 50%;
  animation: tree-spin 800ms linear infinite;
}

@keyframes tree-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .tree-spinner {
    animation: none;
  }
}
</style>
