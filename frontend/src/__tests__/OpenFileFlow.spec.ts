import { createPinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { createMemoryHistory } from 'vue-router'
import { describe, expect, it, vi } from 'vitest'

import App from '@/App.vue'
import { createAppRouter } from '@/router'

import { createMatchMediaController } from './matchMedia'

const hash = `sha256:${'a'.repeat(64)}`
const modifiedAt = '2026-07-22T01:02:03.004Z'

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  })
}

function treeEntry(type: 'directory' | 'file', name: string, path: string, size?: number) {
  return {
    type,
    name,
    path,
    modified_at: modifiedAt,
    ...(size === undefined ? {} : { size }),
  }
}

async function mountAppAt(path: string, desktop = true) {
  const controller = createMatchMediaController(desktop)
  vi.mocked(matchMedia).mockReturnValue(controller.mediaQuery)
  const router = createAppRouter(createMemoryHistory())
  await router.push(path)
  await router.isReady()

  const wrapper = mount(App, {
    attachTo: document.body,
    global: { plugins: [createPinia(), router] },
  })

  return { router, wrapper }
}

describe('open file flow', () => {
  it('loads a Unicode deep link and reveals its tree ancestor', async () => {
    const content = '# 첫 메모\n'
    let resolveDocument: ((response: Response) => void) | undefined
    vi.stubGlobal(
      'fetch',
      vi.fn((input: RequestInfo | URL) => {
        const url = String(input)
        if (url === '/api/tree') {
          return Promise.resolve(
            jsonResponse({ path: '', entries: [treeEntry('directory', '프로젝트', '프로젝트')] }),
          )
        }
        if (url.startsWith('/api/tree?')) {
          return Promise.resolve(
            jsonResponse({
              path: '프로젝트',
              entries: [treeEntry('file', '첫 메모.md', '프로젝트/첫 메모.md', 13)],
            }),
          )
        }
        return new Promise<Response>((resolve) => {
          resolveDocument = resolve
        })
      }),
    )

    const { router, wrapper } = await mountAppAt(
      '/files/%ED%94%84%EB%A1%9C%EC%A0%9D%ED%8A%B8/%EC%B2%AB%20%EB%A9%94%EB%AA%A8.md',
    )
    await flushPromises()

    expect(wrapper.get('.document-state[role="status"]').text()).toContain('불러오는 중')
    resolveDocument?.(
      jsonResponse({
        path: '프로젝트/첫 메모.md',
        content,
        hash,
        size: new TextEncoder().encode(content).byteLength,
        modified_at: modifiedAt,
      }),
    )
    await flushPromises()

    expect(router.currentRoute.value.params.path).toBe('프로젝트/첫 메모.md')
    expect(wrapper.get('.document-content pre').text()).toBe(content.trim())
    expect(wrapper.get('[aria-selected="true"]').text()).toContain('첫 메모.md')
  })

  it('navigates from a tree file and closes only the mobile drawer', async () => {
    const content = '# Note\n'
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: RequestInfo | URL) => {
        const url = String(input)
        if (url === '/api/tree') {
          return jsonResponse({
            path: '',
            entries: [treeEntry('file', 'note.md', 'note.md', content.length)],
          })
        }
        return jsonResponse({
          path: 'note.md',
          content,
          hash,
          size: content.length,
          modified_at: modifiedAt,
        })
      }),
    )
    const { router, wrapper } = await mountAppAt('/', false)
    await flushPromises()

    await wrapper.get('[aria-label="파일 탐색 패널 전환"]').trigger('click')
    await wrapper.get('[role="treeitem"]').trigger('click')
    await flushPromises()

    expect(router.currentRoute.value).toMatchObject({ name: 'file' })
    expect(router.currentRoute.value.params.path).toBe('note.md')
    expect(wrapper.get('#workspace-navigation').attributes('aria-hidden')).toBe('true')
    expect(wrapper.get('.document-content pre').text()).toBe(content.trim())
  })

  it('renders a retryable read error and clears the document on the root route', async () => {
    const content = 'Recovered'
    let documentAttempts = 0
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: RequestInfo | URL) => {
        const url = String(input)
        if (url === '/api/tree') {
          return jsonResponse({
            path: '',
            entries: [treeEntry('file', 'note.md', 'note.md', content.length)],
          })
        }

        documentAttempts += 1
        if (documentAttempts === 1) {
          return jsonResponse(
            { error: { code: 'internal_error', message: 'Temporary failure' } },
            500,
          )
        }
        return jsonResponse({
          path: 'note.md',
          content,
          hash,
          size: content.length,
          modified_at: modifiedAt,
        })
      }),
    )
    const { router, wrapper } = await mountAppAt('/files/note.md')
    await flushPromises()

    expect(wrapper.get('.document-state[role="alert"]').text()).toContain('Temporary failure')
    await wrapper.get('.document-state[role="alert"] button').trigger('click')
    await flushPromises()
    expect(wrapper.get('.document-content pre').text()).toBe(content)

    await router.push('/')
    await flushPromises()
    expect(wrapper.get('.document-state').text()).toContain('파일 트리에서 Markdown 문서를 선택')
    expect(wrapper.find('[aria-selected="true"]').exists()).toBe(false)
  })
})
