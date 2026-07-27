<script setup lang="ts">
import { computed, watch } from 'vue'
import { onBeforeRouteLeave, onBeforeRouteUpdate, useRoute } from 'vue-router'

import WorkspaceShell from '@/components/workspace/WorkspaceShell.vue'
import { useWorkspaceDocumentNavigation } from '@/composables/useWorkspaceDocumentNavigation'
import { useDocumentStore } from '@/stores/document'
import { useTreeStore } from '@/stores/tree'

const route = useRoute()
const documentState = useDocumentStore()
const tree = useTreeStore()
const { openCreatedDocument, openDocument } = useWorkspaceDocumentNavigation()

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
</script>

<template>
  <WorkspaceShell @document-created="openCreatedDocument" @open-file="openDocument" />
</template>
