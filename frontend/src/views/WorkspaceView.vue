<script setup lang="ts">
import { computed, watch } from 'vue'
import { onBeforeRouteLeave, onBeforeRouteUpdate, useRoute, useRouter } from 'vue-router'

import WorkspaceShell from '@/components/workspace/WorkspaceShell.vue'
import { useDocumentStore } from '@/stores/document'
import { useLayoutStore } from '@/stores/layout'
import { useTreeStore } from '@/stores/tree'

const route = useRoute()
const router = useRouter()
const documentState = useDocumentStore()
const layout = useLayoutStore()
const tree = useTreeStore()

const routePath = computed(() => {
  const path = route.params.path
  if (typeof path === 'string' && path) return path
  if (Array.isArray(path) && path.length > 0) return path.join('/')
  return null
})

function confirmDocumentChange() {
  if (!documentState.hasUnsavedChanges) return true
  return window.confirm('저장하지 않은 변경은 브라우저 초안으로 보관됩니다. 이동하시겠습니까?')
}

onBeforeRouteUpdate((to, from) => {
  if (to.params.path === from.params.path) return true
  return confirmDocumentChange()
})

onBeforeRouteLeave(() => confirmDocumentChange())

watch(
  routePath,
  (path) => {
    if (!path) {
      documentState.clearFile()
      tree.selectNode(null)
      return
    }

    tree.selectNode(null)
    void documentState.openFile(path)
    void tree.revealPath(path)
  },
  { immediate: true },
)

async function openFile(path: string) {
  await router.push({ name: 'file', params: { path } })
  if (layout.viewportMode === 'mobile') layout.closeMobilePanel()
}
</script>

<template>
  <WorkspaceShell @open-file="openFile" />
</template>
