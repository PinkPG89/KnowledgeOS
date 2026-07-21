import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

export const useNetworkStore = defineStore('network', () => {
  const isOnline = ref(typeof navigator === 'undefined' ? true : navigator.onLine)
  let listening = false

  const label = computed(() => (isOnline.value ? '온라인' : '오프라인'))

  function syncFromBrowser() {
    isOnline.value = navigator.onLine
  }

  function startListening() {
    if (typeof window === 'undefined' || listening) {
      return
    }

    listening = true
    syncFromBrowser()
    window.addEventListener('online', syncFromBrowser)
    window.addEventListener('offline', syncFromBrowser)
  }

  function stopListening() {
    if (typeof window === 'undefined' || !listening) {
      return
    }

    listening = false
    window.removeEventListener('online', syncFromBrowser)
    window.removeEventListener('offline', syncFromBrowser)
  }

  return { isOnline, label, startListening, stopListening }
})
