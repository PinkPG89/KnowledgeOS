import { onBeforeUnmount, onMounted } from 'vue'

import { DESKTOP_LAYOUT_QUERY, useLayoutStore } from '@/stores/layout'
import { getBrowserStorage } from '@/utils/browserStorage'

export function useResponsiveLayout() {
  const layout = useLayoutStore()
  let mediaQuery: MediaQueryList | undefined

  const applyViewport = (matches: boolean) => {
    layout.setViewportMode(matches ? 'desktop' : 'mobile')
  }

  const handleChange = (event: MediaQueryListEvent) => {
    applyViewport(event.matches)
  }

  onMounted(() => {
    layout.restoreDesktopPreference(getBrowserStorage())
    mediaQuery = globalThis.matchMedia(DESKTOP_LAYOUT_QUERY)
    applyViewport(mediaQuery.matches)
    mediaQuery.addEventListener('change', handleChange)
  })

  onBeforeUnmount(() => {
    mediaQuery?.removeEventListener('change', handleChange)
  })

  return layout
}
