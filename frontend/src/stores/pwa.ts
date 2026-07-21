import { ref } from 'vue'
import { defineStore } from 'pinia'

export const usePwaStore = defineStore('pwa', () => {
  const offlineReady = ref(false)
  const updateAvailable = ref(false)
  const registrationFailed = ref(false)

  function markOfflineReady() {
    offlineReady.value = true
  }

  function markUpdateAvailable() {
    updateAvailable.value = true
  }

  function markRegistrationFailed() {
    registrationFailed.value = true
  }

  function dismissOfflineReady() {
    offlineReady.value = false
  }

  return {
    offlineReady,
    updateAvailable,
    registrationFailed,
    markOfflineReady,
    markUpdateAvailable,
    markRegistrationFailed,
    dismissOfflineReady,
  }
})
