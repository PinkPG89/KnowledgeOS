import type { Pinia } from 'pinia'
import type { Router } from 'vue-router'

import { usePwaStore } from '@/stores/pwa'

let applyUpdate: ((reloadPage?: boolean) => Promise<void>) | undefined

export async function registerPwa(router: Router, pinia: Pinia) {
  await router.isReady()

  const pwa = usePwaStore(pinia)

  try {
    const { registerSW } = await import('virtual:pwa-register')
    applyUpdate = registerSW({
      immediate: true,
      onOfflineReady: () => pwa.markOfflineReady(),
      onNeedRefresh: () => pwa.markUpdateAvailable(),
      onRegisterError: () => pwa.markRegistrationFailed(),
    })
  } catch {
    pwa.markRegistrationFailed()
  }
}

export async function updatePwa() {
  await applyUpdate?.(true)
}
