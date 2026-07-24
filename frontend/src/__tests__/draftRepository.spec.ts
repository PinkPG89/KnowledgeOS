import { describe, expect, it } from 'vitest'

import { getBrowserDraftRepository } from '@/services/draftRepository'

describe('browser draft repository', () => {
  it('falls back safely when IndexedDB access is blocked', () => {
    const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'indexedDB')
    Object.defineProperty(globalThis, 'indexedDB', {
      configurable: true,
      get: () => {
        throw new DOMException('Blocked by privacy mode', 'SecurityError')
      },
    })

    try {
      expect(getBrowserDraftRepository()).toBeUndefined()
    } finally {
      if (descriptor) {
        Object.defineProperty(globalThis, 'indexedDB', descriptor)
      } else {
        Reflect.deleteProperty(globalThis, 'indexedDB')
      }
    }
  })
})
