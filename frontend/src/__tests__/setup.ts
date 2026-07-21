import { beforeEach, vi } from 'vitest'

import { createMatchMediaController } from './matchMedia'

beforeEach(() => {
  localStorage.clear()
  const controller = createMatchMediaController(true)
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => controller.mediaQuery),
  )
})
