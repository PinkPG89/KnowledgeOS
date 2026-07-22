<script setup lang="ts">
import { computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

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
