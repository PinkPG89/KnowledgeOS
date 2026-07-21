import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it } from 'vitest'

import { LAYOUT_PREFERENCE_KEY, useLayoutStore } from '@/stores/layout'

describe('layout store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('persists only desktop panel preferences', () => {
    const store = useLayoutStore()

    store.toggleNavigation(localStorage)

    expect(store.desktopNavigationOpen).toBe(false)
    expect(JSON.parse(localStorage.getItem(LAYOUT_PREFERENCE_KEY) ?? '{}')).toEqual({
      navigationOpen: false,
      inspectorOpen: true,
    })

    store.setViewportMode('mobile')
    store.toggleInspector(localStorage)

    expect(store.mobilePanel).toBe('inspector')
    expect(JSON.parse(localStorage.getItem(LAYOUT_PREFERENCE_KEY) ?? '{}')).toEqual({
      navigationOpen: false,
      inspectorOpen: true,
    })
  })

  it('keeps mobile panels mutually exclusive and resets them at a breakpoint', () => {
    const store = useLayoutStore()
    store.setViewportMode('mobile')

    store.toggleNavigation(localStorage)
    expect(store.mobilePanel).toBe('navigation')

    store.toggleInspector(localStorage)
    expect(store.mobilePanel).toBe('inspector')

    store.setViewportMode('desktop')
    expect(store.mobilePanel).toBeNull()
    expect(store.navigationVisible).toBe(true)
    expect(store.inspectorVisible).toBe(true)
  })

  it('ignores malformed stored preferences', () => {
    localStorage.setItem(LAYOUT_PREFERENCE_KEY, '{invalid')
    const store = useLayoutStore()

    store.restoreDesktopPreference(localStorage)

    expect(store.desktopNavigationOpen).toBe(true)
    expect(store.desktopInspectorOpen).toBe(true)
  })
})
