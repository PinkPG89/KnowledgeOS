import { beforeEach, vi } from 'vitest'

import { createMatchMediaController } from './matchMedia'

beforeEach(() => {
  localStorage.clear()
  const controller = createMatchMediaController(true)
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => controller.mediaQuery),
  )
  vi.stubGlobal(
    'fetch',
    vi.fn(
      async () =>
        new Response(JSON.stringify({ path: '', entries: [] }), {
          status: 200,
          headers: { 'Content-Type': 'application/json' },
        }),
    ),
  )
})
