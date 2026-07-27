<script setup lang="ts">
import { computed, type CSSProperties } from 'vue'

import type { TreeDirectoryState } from '@/models/tree'

const props = defineProps<{
  depth: number
  path: string
  state?: TreeDirectoryState
}>()
const emit = defineEmits<{ retry: [] }>()
const rowStyle = computed(() => ({ '--tree-depth': props.depth }) as CSSProperties)
const visible = computed(
  () =>
    props.state?.expanded &&
    (props.state.loadStatus === 'loading' ||
      props.state.loadStatus === 'error' ||
      (props.state.loadStatus === 'loaded' && props.state.childPaths.length === 0)),
)
</script>

<template>
  <li
    v-if="visible"
    class="tree-inline-state"
    :class="{ 'tree-inline-state--error': state?.loadStatus === 'error' }"
    role="none"
    :data-directory-state="path"
    :style="rowStyle"
  >
    <span v-if="state?.loadStatus === 'loading'" role="status">
      하위 항목을 불러오는 중입니다.
    </span>
    <template v-else-if="state?.loadStatus === 'error'">
      <span role="alert">{{ state.error?.message }}</span>
      <button type="button" @click.stop="emit('retry')">다시 시도</button>
    </template>
    <span v-else>빈 폴더</span>
  </li>
</template>

<style scoped>
.tree-inline-state {
  display: flex;
  min-height: 2.75rem;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding-left: calc(2.85rem + (var(--tree-depth) - 1) * 1rem);
  color: var(--color-text-muted);
  font-size: 0.75rem;
}

.tree-inline-state--error {
  color: var(--color-warning);
}

.tree-inline-state button {
  min-width: 2.75rem;
  min-height: 2.75rem;
  padding: 0 0.6rem;
  border: 0;
  border-radius: 0.65rem;
  background: transparent;
  color: inherit;
  cursor: pointer;
  font: inherit;
  font-weight: 750;
}

.tree-inline-state button:hover {
  background: var(--color-surface-muted);
}
</style>
