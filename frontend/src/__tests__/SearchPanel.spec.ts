import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

import SearchPanel from '@/components/search/SearchPanel.vue'
import { SearchClientError, type SearchClient } from '@/services/searchClient'

function mountPanel(search: SearchClient['search']) {
  const client: SearchClient = { search: vi.fn(search) }
  const wrapper = mount(SearchPanel, {
    attachTo: document.body,
    props: { client },
  })
  return { client, wrapper }
}

describe('SearchPanel', () => {
  it('keeps an empty initial state and does not submit whitespace', async () => {
    const { client, wrapper } = mountPanel(vi.fn())

    expect(wrapper.get('[role="status"]').text()).toContain('검색어를 입력')
    await wrapper.get('input[type="search"]').setValue('   ')
    await wrapper.get('form').trigger('submit')

    expect(client.search).not.toHaveBeenCalled()
    expect(wrapper.get('[role="status"]').text()).toContain('검색어를 입력')
  })

  it('renders loading and no-result states', async () => {
    let resolveSearch: ((value: Awaited<ReturnType<SearchClient['search']>>) => void) | undefined
    const { wrapper } = mountPanel(
      () =>
        new Promise((resolve) => {
          resolveSearch = resolve
        }),
    )

    await wrapper.get('input[type="search"]').setValue('missing')
    await wrapper.get('form').trigger('submit')
    expect(wrapper.get('[role="status"]').text()).toContain('검색 중')

    resolveSearch?.({ query: 'missing', limit: 20, results: [] })
    await flushPromises()
    expect(wrapper.get('[role="status"]').text()).toContain('검색 결과가 없습니다')
    expect(wrapper.get('[role="status"]').text()).toContain('missing')
  })

  it('renders an error and retries the submitted query', async () => {
    let attempts = 0
    const { client, wrapper } = mountPanel(async (query) => {
      attempts += 1
      if (attempts === 1) {
        throw new SearchClientError('search_unavailable', '색인을 사용할 수 없습니다.', 503)
      }
      return { query, limit: 20, results: [] }
    })

    await wrapper.get('input[type="search"]').setValue('retry')
    await wrapper.get('form').trigger('submit')
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain('색인을 사용할 수 없습니다.')

    await wrapper.get('[role="alert"] button').trigger('click')
    await flushPromises()
    expect(client.search).toHaveBeenCalledTimes(2)
    expect(wrapper.get('[role="status"]').text()).toContain('검색 결과가 없습니다')
  })

  it('aborts an earlier request and ignores its stale result', async () => {
    const pending = new Map<string, (value: Awaited<ReturnType<SearchClient['search']>>) => void>()
    const signals: AbortSignal[] = []
    const { wrapper } = mountPanel(
      (query, signal) =>
        new Promise((resolve) => {
          pending.set(query, resolve)
          if (signal) signals.push(signal)
        }),
    )
    const input = wrapper.get<HTMLInputElement>('input[type="search"]')

    await input.setValue('first')
    await wrapper.get('form').trigger('submit')
    await input.setValue('second')
    await wrapper.get('form').trigger('submit')

    expect(signals[0]?.aborted).toBe(true)
    pending.get('second')?.({
      query: 'second',
      limit: 20,
      results: [{ path: 'second.md', title: 'Second', snippet: '', score: 1 }],
    })
    await flushPromises()
    expect(wrapper.get('[role="option"]').text()).toContain('Second')

    pending.get('first')?.({
      query: 'first',
      limit: 20,
      results: [{ path: 'first.md', title: 'First', snippet: '', score: 2 }],
    })
    await flushPromises()
    expect(wrapper.get('[role="option"]').text()).toContain('Second')
  })

  it('moves result focus and opens a result with Enter or click', async () => {
    const { wrapper } = mountPanel(async (query) => ({
      query,
      limit: 20,
      results: [
        { path: 'alpha.md', title: 'Alpha', snippet: 'First result', score: 2 },
        { path: 'beta.md', title: 'Beta', snippet: 'Second result', score: 1 },
        { path: 'gamma.md', title: 'Gamma', snippet: '', score: 0.5 },
      ],
    }))
    const input = wrapper.get<HTMLInputElement>('input[type="search"]')
    await input.setValue('result')
    await wrapper.get('form').trigger('submit')
    await flushPromises()

    const options = wrapper.findAll<HTMLElement>('[role="option"]')
    expect(options).toHaveLength(3)
    expect(options[0]?.text()).toContain('First result')
    expect(options[2]?.text()).not.toContain('undefined')

    input.element.focus()
    await input.trigger('keydown', { key: 'ArrowDown' })
    expect(document.activeElement).toBe(options[0]?.element)

    await options[0]?.trigger('keydown', { key: 'End' })
    expect(document.activeElement).toBe(options[2]?.element)
    await options[2]?.trigger('keydown', { key: 'ArrowUp' })
    expect(document.activeElement).toBe(options[1]?.element)
    await options[1]?.trigger('keydown', { key: 'Home' })
    expect(document.activeElement).toBe(options[0]?.element)

    await options[0]?.trigger('keydown', { key: 'Enter' })
    expect(wrapper.emitted('openFile')).toEqual([['alpha.md']])

    await options[1]?.trigger('click')
    expect(wrapper.emitted('openFile')).toEqual([['alpha.md'], ['beta.md']])
  })
})
