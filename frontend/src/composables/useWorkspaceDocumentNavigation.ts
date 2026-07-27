import { useRouter } from 'vue-router'

import { useLayoutStore } from '@/stores/layout'
import { useTreeStore } from '@/stores/tree'
import { directParentPath } from '@/utils/canonicalPath'

export function useWorkspaceDocumentNavigation() {
  const router = useRouter()
  const layout = useLayoutStore()
  const tree = useTreeStore()

  async function openDocument(path: string) {
    await router.push({ name: 'file', params: { path } })
    if (layout.viewportMode === 'mobile') layout.closeMobilePanel()
  }

  async function openCreatedDocument(path: string) {
    await tree.refreshDirectory(directParentPath(path))
    await openDocument(path)
  }

  return { openCreatedDocument, openDocument }
}
