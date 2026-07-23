import { beforeEach, vi } from 'vitest'

import { createMatchMediaController } from './matchMedia'

Range.prototype.getClientRects = vi.fn(() => ({
  length: 0,
  item: () => null,
  [Symbol.iterator]: function* () {},
})) as typeof Range.prototype.getClientRects

Range.prototype.getBoundingClientRect = vi.fn(
  () => new DOMRect(),
) as typeof Range.prototype.getBoundingClientRect

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
