import { createPinia, setActivePinia } from 'pinia'
import { afterEach, describe, expect, it } from 'vitest'

import { useNetworkStore } from '@/stores/network'

const originalOnline = Object.getOwnPropertyDescriptor(Navigator.prototype, 'onLine')

function setBrowserOnline(value: boolean) {
  Object.defineProperty(Navigator.prototype, 'onLine', {
    configurable: true,
    get: () => value,
  })
}

afterEach(() => {
  if (originalOnline) {
    Object.defineProperty(Navigator.prototype, 'onLine', originalOnline)
  }
})

describe('network store', () => {
  it('tracks browser online and offline events', () => {
    setBrowserOnline(true)
    setActivePinia(createPinia())
    const store = useNetworkStore()
    store.startListening()

    expect(store.isOnline).toBe(true)
    expect(store.label).toBe('온라인')

    setBrowserOnline(false)
    window.dispatchEvent(new Event('offline'))

    expect(store.isOnline).toBe(false)
    expect(store.label).toBe('오프라인')
    store.stopListening()
  })
})
