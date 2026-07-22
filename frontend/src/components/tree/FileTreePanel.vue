<script setup lang="ts">
import {
  computed,
  nextTick,
  onMounted,
  type ComponentPublicInstance,
  type CSSProperties,
} from 'vue'

import type { TreeDirectoryState, TreeNode } from '@/models/tree'
import { treeClient as defaultTreeClient, type TreeClient } from '@/services/treeClient'
import { ROOT_DIRECTORY_PATH, useTreeStore } from '@/stores/tree'

interface VisibleTreeItem {
  node: TreeNode
  depth: number
  parentPath: string
  position: number
  setSize: number
}

const props = defineProps<{ client?: TreeClient }>()
const tree = useTreeStore()
const itemElements = new Map<string, HTMLElement>()
const focusedPath = defineModel<string | null>('focusedPath', { default: null })

const rootState = computed(() => tree.directoriesByPath[ROOT_DIRECTORY_PATH]!)
const visibleItems = computed(() => {
  const items: VisibleTreeItem[] = []
  const visitedDirectories = new Set<string>()

  appendVisibleChildren(ROOT_DIRECTORY_PATH, 1, items, visitedDirectories)
  return items
})

onMounted(() => {
  void tree.loadDirectory(ROOT_DIRECTORY_PATH, props.client ?? defaultTreeClient)
})

function appendVisibleChildren(
  parentPath: string,
  depth: number,
  items: VisibleTreeItem[],
  visitedDirectories: Set<string>,
) {
  if (visitedDirectories.has(parentPath)) return
  visitedDirectories.add(parentPath)

  const directory = tree.directoriesByPath[parentPath]
  if (!directory) return

  const children = directory.childPaths
    .map((path) => tree.nodesByPath[path])
    .filter((node): node is TreeNode => node !== undefined)

  children.forEach((node, index) => {
    items.push({
      node,
      depth,
      parentPath,
      position: index + 1,
      setSize: children.length,
    })

    if (node.kind === 'directory' && tree.directoriesByPath[node.path]?.expanded) {
      appendVisibleChildren(node.path, depth + 1, items, visitedDirectories)
    }
  })
}

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

function setItemElement(path: string, value: Element | ComponentPublicInstance | null) {
  if (value instanceof HTMLElement) {
    itemElements.set(path, value)
  } else {
    itemElements.delete(path)
  }
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
}

async function retryDirectory(path: string) {
  await tree.loadDirectory(path, props.client ?? defaultTreeClient)
  await focusItem(path || visibleItems.value[0]?.node.path)
}

async function refreshRoot() {
  await tree.refreshDirectory(ROOT_DIRECTORY_PATH, props.client ?? defaultTreeClient)
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
    <div class="file-tree__toolbar">
      <span>Markdown files</span>
      <button
        type="button"
        aria-label="파일 트리 새로고침"
        :disabled="rootState.loadStatus === 'loading'"
        @click="refreshRoot"
      >
        <span aria-hidden="true">↻</span>
      </button>
    </div>

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

    <div v-else-if="visibleItems.length === 0" class="tree-state">
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
      <template v-for="item in visibleItems" :key="item.node.path">
        <li
          :ref="(value) => setItemElement(item.node.path, value)"
          class="tree-item"
          :class="{ 'tree-item--selected': tree.selectedPath === item.node.path }"
          role="treeitem"
          :tabindex="itemTabIndex(item.node.path)"
          :aria-level="item.depth"
          :aria-posinset="item.position"
          :aria-setsize="item.setSize"
          :aria-expanded="
            item.node.kind === 'directory' ? directoryState(item.node.path)?.expanded : undefined
          "
          :aria-selected="
            item.node.kind === 'file' ? tree.selectedPath === item.node.path : undefined
          "
          :aria-busy="
            item.node.kind === 'directory' &&
            directoryState(item.node.path)?.loadStatus === 'loading'
          "
          :style="rowStyle(item.depth)"
          @click="activateItem(item)"
          @focus="focusedPath = item.node.path"
          @keydown="handleKeydown($event, item)"
        >
          <span
            v-if="item.node.kind === 'directory'"
            class="tree-item__disclosure"
            aria-hidden="true"
          >
            {{ directoryState(item.node.path)?.expanded ? '▾' : '▸' }}
          </span>
          <span v-else class="tree-item__disclosure" aria-hidden="true">·</span>
          <span class="tree-item__icon" aria-hidden="true">{{
            item.node.kind === 'directory' ? '□' : '≡'
          }}</span>
          <span class="tree-item__name" :title="item.node.name">{{ item.node.name }}</span>
          <span
            v-if="directoryState(item.node.path)?.loadStatus === 'loading'"
            class="tree-spinner"
            aria-hidden="true"
          />
        </li>

        <li
          v-if="
            item.node.kind === 'directory' &&
            directoryState(item.node.path)?.expanded &&
            directoryState(item.node.path)?.loadStatus === 'loading'
          "
          class="tree-inline-state"
          role="none"
          :data-directory-state="item.node.path"
          :style="rowStyle(item.depth + 1)"
        >
          <span role="status">하위 항목을 불러오는 중입니다.</span>
        </li>

        <li
          v-else-if="
            item.node.kind === 'directory' &&
            directoryState(item.node.path)?.expanded &&
            directoryState(item.node.path)?.loadStatus === 'error'
          "
          class="tree-inline-state tree-inline-state--error"
          role="none"
          :data-directory-state="item.node.path"
          :style="rowStyle(item.depth + 1)"
        >
          <span role="alert">{{ directoryState(item.node.path)?.error?.message }}</span>
          <button type="button" @click.stop="retryDirectory(item.node.path)">다시 시도</button>
        </li>

        <li
          v-else-if="
            item.node.kind === 'directory' &&
            directoryState(item.node.path)?.expanded &&
            directoryState(item.node.path)?.loadStatus === 'loaded' &&
            directoryState(item.node.path)?.childPaths.length === 0
          "
          class="tree-inline-state"
          role="none"
          :data-directory-state="item.node.path"
          :style="rowStyle(item.depth + 1)"
        >
          빈 폴더
        </li>
      </template>
    </ul>
  </section>
</template>

<style scoped>
.file-tree {
  min-height: 0;
}

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

.file-tree__toolbar button,
.tree-state button,
.tree-inline-state button {
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

.file-tree__toolbar button:hover,
.tree-state button:hover,
.tree-inline-state button:hover {
  background: var(--color-surface-muted);
}

.file-tree__toolbar button:disabled {
  cursor: wait;
  opacity: 0.55;
}

.tree-list {
  margin: 0;
  padding: 0.45rem;
  list-style: none;
}

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

.tree-state--error strong,
.tree-inline-state--error {
  color: var(--color-warning);
}

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

.tree-inline-state button {
  padding: 0 0.6rem;
  color: inherit;
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
