import { defineStore } from 'pinia'

export const DESKTOP_LAYOUT_QUERY = '(min-width: 64rem)'
export const LAYOUT_PREFERENCE_KEY = 'knowledgeos:layout:v1'

export type ViewportMode = 'desktop' | 'mobile'
export type MobilePanel = 'navigation' | 'inspector' | null

interface DesktopLayoutPreference {
  navigationOpen: boolean
  inspectorOpen: boolean
}

interface LayoutState {
  viewportMode: ViewportMode
  desktopNavigationOpen: boolean
  desktopInspectorOpen: boolean
  mobilePanel: MobilePanel
}

function readDesktopPreference(storage: Storage | undefined): DesktopLayoutPreference | null {
  if (!storage) return null

  try {
    const value: unknown = JSON.parse(storage.getItem(LAYOUT_PREFERENCE_KEY) ?? 'null')

    if (
      typeof value === 'object' &&
      value !== null &&
      'navigationOpen' in value &&
      typeof value.navigationOpen === 'boolean' &&
      'inspectorOpen' in value &&
      typeof value.inspectorOpen === 'boolean'
    ) {
      return {
        navigationOpen: value.navigationOpen,
        inspectorOpen: value.inspectorOpen,
      }
    }
  } catch {
    // 손상된 localStorage 값은 UI 시작을 막지 않고 기본 layout으로 복구합니다.
  }

  return null
}

export const useLayoutStore = defineStore('layout', {
  state: (): LayoutState => ({
    viewportMode: 'desktop',
    desktopNavigationOpen: true,
    desktopInspectorOpen: true,
    mobilePanel: null,
  }),
  getters: {
    navigationVisible: (state) =>
      state.viewportMode === 'desktop'
        ? state.desktopNavigationOpen
        : state.mobilePanel === 'navigation',
    inspectorVisible: (state) =>
      state.viewportMode === 'desktop'
        ? state.desktopInspectorOpen
        : state.mobilePanel === 'inspector',
    hasMobileOverlay: (state) => state.viewportMode === 'mobile' && state.mobilePanel !== null,
  },
  actions: {
    restoreDesktopPreference(storage: Storage | undefined) {
      const preference = readDesktopPreference(storage)
      if (!preference) return

      this.desktopNavigationOpen = preference.navigationOpen
      this.desktopInspectorOpen = preference.inspectorOpen
    },
    setViewportMode(mode: ViewportMode) {
      if (this.viewportMode === mode) return

      this.viewportMode = mode
      // Mobile overlay 상태는 일시적인 UI 상태이므로 breakpoint를 넘을 때 폐기합니다.
      this.mobilePanel = null
    },
    toggleNavigation(storage: Storage | undefined) {
      if (this.viewportMode === 'mobile') {
        this.mobilePanel = this.mobilePanel === 'navigation' ? null : 'navigation'
        return
      }

      this.desktopNavigationOpen = !this.desktopNavigationOpen
      this.persistDesktopPreference(storage)
    },
    toggleInspector(storage: Storage | undefined) {
      if (this.viewportMode === 'mobile') {
        this.mobilePanel = this.mobilePanel === 'inspector' ? null : 'inspector'
        return
      }

      this.desktopInspectorOpen = !this.desktopInspectorOpen
      this.persistDesktopPreference(storage)
    },
    closeMobilePanel() {
      this.mobilePanel = null
    },
    persistDesktopPreference(storage: Storage | undefined) {
      if (!storage) return

      try {
        storage.setItem(
          LAYOUT_PREFERENCE_KEY,
          JSON.stringify({
            navigationOpen: this.desktopNavigationOpen,
            inspectorOpen: this.desktopInspectorOpen,
          } satisfies DesktopLayoutPreference),
        )
      } catch {
        // 저장 공간이 차단되어도 현재 session의 panel 조작은 계속 동작해야 합니다.
      }
    },
  },
})
