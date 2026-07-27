<script setup lang="ts">
import {
  computed,
  nextTick,
  onMounted,
  ref,
  type ComponentPublicInstance,
  type CSSProperties,
} from 'vue'

import type { TreeDirectoryState } from '@/models/tree'
import {
  buildVisibleTreeItems,
  resolveDocumentCreateParent,
  type VisibleTreeItem,
} from '@/models/treeView'
import type { MarkdownCreateClient } from '@/services/markdownClient'
import { treeClient as defaultTreeClient, type TreeClient } from '@/services/treeClient'
import { ROOT_DIRECTORY_PATH, useTreeStore } from '@/stores/tree'

import InlineDocumentCreate from './InlineDocumentCreate.vue'
import FileTreeToolbar from './FileTreeToolbar.vue'
import TreeDirectoryStateRow from './TreeDirectoryStateRow.vue'
import TreeItemRow from './TreeItemRow.vue'

const props = defineProps<{ client?: TreeClient; createClient?: MarkdownCreateClient }>()
const emit = defineEmits<{ documentCreated: [path: string]; openFile: [path: string] }>()
const tree = useTreeStore()
const itemElements = new Map<string, FocusableTreeRow>()
const focusedPath = defineModel<string | null>('focusedPath', { default: null })
const createParentPath = ref<string | null>(null)

const rootState = computed(() => tree.directoriesByPath[ROOT_DIRECTORY_PATH]!)
const visibleItems = computed(() =>
  buildVisibleTreeItems(ROOT_DIRECTORY_PATH, tree.nodesByPath, tree.directoriesByPath),
)

onMounted(() => {
  void tree.loadDirectory(ROOT_DIRECTORY_PATH, props.client ?? defaultTreeClient)
})

function directoryState(path: string): TreeDirectoryState | undefined {
  return tree.directoriesByPath[path]
}

function rowStyle(depth: number): CSSProperties {
  return { '--tree-depth': depth } as CSSProperties
}

function itemTabIndex(path: string): 0 | -1 {
  const activePath = focusedPath.value ?? visibleItems.value[0]?.node.path
  return activePath === path ? 0 : -1
}

interface FocusableTreeRow {
  focus(): void
}

function setItemElement(path: string, value: Element | ComponentPublicInstance | null) {
  if (isFocusableTreeRow(value)) {
    itemElements.set(path, value)
  } else {
    itemElements.delete(path)
  }
}

function isFocusableTreeRow(value: unknown): value is FocusableTreeRow {
  return (
    typeof value === 'object' &&
    value !== null &&
    'focus' in value &&
    typeof value.focus === 'function'
  )
}

async function focusItem(path: string | undefined) {
  if (!path) return
  focusedPath.value = path
  await nextTick()
  itemElements.get(path)?.focus()
}

async function activateItem(item: VisibleTreeItem) {
  focusedPath.value = item.node.path
  if (item.node.kind === 'directory') {
    await tree.toggleDirectory(item.node.path, props.client ?? defaultTreeClient)
    return
  }

  tree.selectNode(item.node.path)
  emit('openFile', item.node.path)
}

async function retryDirectory(path: string) {
  await tree.loadDirectory(path, props.client ?? defaultTreeClient)
  await focusItem(path || visibleItems.value[0]?.node.path)
}

async function refreshRoot() {
  await tree.refreshDirectory(ROOT_DIRECTORY_PATH, props.client ?? defaultTreeClient)
}

async function beginDocumentCreate() {
  if (createParentPath.value !== null) return

  const parentPath = resolveDocumentCreateParent(
    focusedPath.value,
    tree.selectedPath,
    tree.nodesByPath,
    ROOT_DIRECTORY_PATH,
  )

  if (parentPath !== ROOT_DIRECTORY_PATH && !directoryState(parentPath)?.expanded) {
    await tree.toggleDirectory(parentPath, props.client ?? defaultTreeClient)
  }

  createParentPath.value = parentPath
}

function cancelDocumentCreate() {
  createParentPath.value = null
}

function handleDocumentCreated(path: string) {
  createParentPath.value = null
  emit('documentCreated', path)
}

async function handleKeydown(event: KeyboardEvent, item: VisibleTreeItem) {
  const itemIndex = visibleItems.value.findIndex(({ node }) => node.path === item.node.path)
  let nextPath: string | undefined

  switch (event.key) {
    case 'ArrowDown':
      nextPath =
        visibleItems.value[Math.min(itemIndex + 1, visibleItems.value.length - 1)]?.node.path
      break
    case 'ArrowUp':
      nextPath = visibleItems.value[Math.max(itemIndex - 1, 0)]?.node.path
      break
    case 'Home':
      nextPath = visibleItems.value[0]?.node.path
      break
    case 'End':
      nextPath = visibleItems.value[visibleItems.value.length - 1]?.node.path
      break
    case 'ArrowRight':
      if (item.node.kind !== 'directory') return
      if (!directoryState(item.node.path)?.expanded) {
        await tree.toggleDirectory(item.node.path, props.client ?? defaultTreeClient)
      } else {
        nextPath = directoryState(item.node.path)?.childPaths[0]
      }
      break
    case 'ArrowLeft':
      if (item.node.kind === 'directory' && directoryState(item.node.path)?.expanded) {
        await tree.toggleDirectory(item.node.path, props.client ?? defaultTreeClient)
      } else if (item.parentPath !== ROOT_DIRECTORY_PATH) {
        nextPath = item.parentPath
      }
      break
    case 'Enter':
    case ' ':
      await activateItem(item)
      break
    default:
      return
  }

  event.preventDefault()
  await focusItem(nextPath ?? item.node.path)
}
</script>

<template>
  <section class="file-tree" aria-label="Vault 파일 트리">
    <FileTreeToolbar
      :create-disabled="rootState.loadStatus !== 'loaded' || createParentPath !== null"
      :refresh-disabled="rootState.loadStatus === 'loading'"
      @create="beginDocumentCreate"
      @refresh="refreshRoot"
    />

    <div
      v-if="
        (rootState.loadStatus === 'idle' || rootState.loadStatus === 'loading') &&
        visibleItems.length === 0
      "
      class="tree-state"
      role="status"
    >
      <span class="tree-spinner" aria-hidden="true" />
      <strong>Vault를 불러오는 중입니다.</strong>
    </div>

    <div
      v-else-if="rootState.loadStatus === 'error'"
      class="tree-state tree-state--error"
      role="alert"
    >
      <strong>파일 트리를 불러오지 못했습니다.</strong>
      <p>{{ rootState.error?.message }}</p>
      <button type="button" @click="retryDirectory(ROOT_DIRECTORY_PATH)">다시 시도</button>
    </div>

    <div v-else-if="visibleItems.length === 0 && createParentPath === null" class="tree-state">
      <span aria-hidden="true">◇</span>
      <strong>Vault가 비어 있습니다.</strong>
      <p>Markdown 파일이나 폴더를 추가하면 이곳에 표시됩니다.</p>
    </div>

    <ul
      v-else
      class="tree-list"
      role="tree"
      aria-label="Vault contents"
      :aria-busy="rootState.loadStatus === 'loading'"
    >
      <li
        v-if="createParentPath === ROOT_DIRECTORY_PATH"
        class="tree-inline-create"
        role="none"
        :style="rowStyle(1)"
      >
        <InlineDocumentCreate
          :client="createClient"
          :parent-path="ROOT_DIRECTORY_PATH"
          @cancel="cancelDocumentCreate"
          @created="handleDocumentCreated"
        />
      </li>

      <template v-for="item in visibleItems" :key="item.node.path">
        <TreeItemRow
          :ref="(value) => setItemElement(item.node.path, value)"
          :directory-state="directoryState(item.node.path)"
          :item="item"
          :selected="tree.selectedPath === item.node.path"
          :tab-index="itemTabIndex(item.node.path)"
          @activate="activateItem(item)"
          @focus="focusedPath = item.node.path"
          @keydown="handleKeydown($event, item)"
        />

        <li
          v-if="item.node.kind === 'directory' && createParentPath === item.node.path"
          class="tree-inline-create"
          role="none"
          :style="rowStyle(item.depth + 1)"
        >
          <InlineDocumentCreate
            :client="createClient"
            :parent-path="item.node.path"
            @cancel="cancelDocumentCreate"
            @created="handleDocumentCreated"
          />
        </li>

        <TreeDirectoryStateRow
          v-if="item.node.kind === 'directory'"
          :depth="item.depth + 1"
          :path="item.node.path"
          :state="directoryState(item.node.path)"
          @retry="retryDirectory(item.node.path)"
        />
      </template>
    </ul>
  </section>
</template>

<style scoped>
.file-tree {
  min-height: 0;
}

.tree-state button {
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

.tree-state button:hover {
  background: var(--color-surface-muted);
}

.tree-list {
  margin: 0;
  padding: 0.45rem;
  list-style: none;
}

.tree-state {
  display: grid;
  min-height: 13rem;
  align-content: center;
  justify-items: center;
  padding: 1.5rem;
  color: var(--color-text-muted);
  text-align: center;
}

.tree-state strong {
  margin-top: 0.75rem;
  color: var(--color-text);
  font-size: 0.88rem;
}

.tree-state p {
  margin: 0.4rem 0 0;
  font-size: 0.78rem;
  line-height: 1.5;
}

.tree-state button {
  margin-top: 0.75rem;
  padding: 0 0.9rem;
  background: var(--color-surface-muted);
}

.tree-state--error strong {
  color: var(--color-warning);
}

.tree-inline-create {
  padding: 0.45rem 0.35rem 0.55rem calc(0.35rem + (var(--tree-depth) - 1) * 1rem);
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
