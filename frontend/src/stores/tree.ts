import { defineStore } from 'pinia'
import { reactive, ref } from 'vue'

import type { TreeDirectoryState, TreeLoadError, TreeLoadResult, TreeNode } from '@/models/tree'
import { TreeClientError, treeClient, type TreeClient } from '@/services/treeClient'

export const ROOT_DIRECTORY_PATH = ''

export const useTreeStore = defineStore('tree', () => {
  const nodesByPath = reactive<Record<string, TreeNode>>({})
  const directoriesByPath = reactive<Record<string, TreeDirectoryState>>({
    [ROOT_DIRECTORY_PATH]: createDirectoryState(true),
  })
  const selectedPath = ref<string | null>(null)

  // Promise는 UI 상태가 아니므로 Vue reactive graph 밖에서 관리합니다.
  // 같은 path를 동시에 load하면 모든 caller가 하나의 작업 완료를 기다립니다.
  const pendingLoads = new Map<string, Promise<TreeLoadResult>>()

  function loadDirectory(
    path: string = ROOT_DIRECTORY_PATH,
    client: TreeClient = treeClient,
    force = false,
  ): Promise<TreeLoadResult> {
    const existingLoad = pendingLoads.get(path)
    if (existingLoad) return existingLoad

    const directory = ensureDirectoryState(path)
    if (directory.loadStatus === 'loaded' && !force) return Promise.resolve({ ok: true })

    directory.loadStatus = 'loading'
    directory.error = null

    const operation = executeLoad(path, directory, client)
    pendingLoads.set(path, operation)
    return operation
  }

  async function executeLoad(
    path: string,
    directory: TreeDirectoryState,
    client: TreeClient,
  ): Promise<TreeLoadResult> {
    try {
      // Promise microtask에서 client를 호출해야 pendingLoads 등록이 먼저 끝납니다.
      // 이 순서는 test double이나 adapter가 동기적으로 throw해도 stale pending entry를 남기지 않습니다.
      const listing = await Promise.resolve().then(() => client.listDirectory(path))
      const entries = [...listing.entries].sort(compareTreeNodes)

      for (const entry of entries) {
        nodesByPath[entry.path] = entry
        if (entry.kind === 'directory') ensureDirectoryState(entry.path)
      }

      directory.childPaths = entries.map((entry) => entry.path)
      directory.loadStatus = 'loaded'
      directory.error = null
      return { ok: true }
    } catch (error) {
      const treeError = toTreeLoadError(error)
      directory.loadStatus = 'error'
      directory.error = treeError
      return { ok: false, error: treeError }
    } finally {
      pendingLoads.delete(path)
    }
  }

  function refreshDirectory(
    path: string = ROOT_DIRECTORY_PATH,
    client: TreeClient = treeClient,
  ): Promise<TreeLoadResult> {
    return loadDirectory(path, client, true)
  }

  async function toggleDirectory(
    path: string,
    client: TreeClient = treeClient,
  ): Promise<TreeLoadResult> {
    if (path !== ROOT_DIRECTORY_PATH && nodesByPath[path]?.kind !== 'directory') {
      return {
        ok: false,
        error: {
          code: 'not_a_tree_directory',
          message: '선택한 Tree node는 directory가 아닙니다.',
          retryable: false,
        },
      }
    }

    const directory = ensureDirectoryState(path)
    if (directory.expanded) {
      directory.expanded = false
      return { ok: true }
    }

    directory.expanded = true
    return loadDirectory(path, client)
  }

  function selectNode(path: string | null): boolean {
    if (path !== null && !nodesByPath[path]) return false
    selectedPath.value = path
    return true
  }

  function ensureDirectoryState(path: string): TreeDirectoryState {
    const current = directoriesByPath[path]
    if (current) return current

    const state = createDirectoryState(false)
    directoriesByPath[path] = state
    return state
  }

  return {
    nodesByPath,
    directoriesByPath,
    selectedPath,
    loadDirectory,
    refreshDirectory,
    toggleDirectory,
    selectNode,
  }
})

function createDirectoryState(expanded: boolean): TreeDirectoryState {
  return {
    childPaths: [],
    loadStatus: 'idle',
    expanded,
    error: null,
  }
}

function compareTreeNodes(left: TreeNode, right: TreeNode): number {
  if (left.kind !== right.kind) return left.kind === 'directory' ? -1 : 1

  const nameOrder = compareUnicodeCodePoints(left.name, right.name)
  return nameOrder === 0 ? compareUnicodeCodePoints(left.path, right.path) : nameOrder
}

function compareUnicodeCodePoints(left: string, right: string): number {
  const leftPoints = Array.from(left, (character) => character.codePointAt(0) ?? 0)
  const rightPoints = Array.from(right, (character) => character.codePointAt(0) ?? 0)
  const sharedLength = Math.min(leftPoints.length, rightPoints.length)

  for (let index = 0; index < sharedLength; index += 1) {
    const difference = leftPoints[index]! - rightPoints[index]!
    if (difference !== 0) return difference
  }

  return leftPoints.length - rightPoints.length
}

function toTreeLoadError(error: unknown): TreeLoadError {
  if (error instanceof TreeClientError) {
    return {
      code: error.code,
      message: error.message,
      retryable: error.status === null || error.status >= 500,
    }
  }

  return {
    code: 'tree_load_failed',
    message: '파일 트리를 불러오지 못했습니다.',
    retryable: true,
  }
}
