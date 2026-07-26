import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

import CreateDocumentPanel from '@/components/create/CreateDocumentPanel.vue'
import { MarkdownClientError, type MarkdownCreateClient } from '@/services/markdownClient'

const hash = `sha256:${'a'.repeat(64)}`
const modifiedAt = '2026-07-26T01:02:03.004Z'

function mountPanel(createFile: MarkdownCreateClient['createFile']) {
  const client: MarkdownCreateClient = { createFile: vi.fn(createFile) }
  const wrapper = mount(CreateDocumentPanel, {
    attachTo: document.body,
    props: { client },
  })
  return { client, wrapper }
}

function createdDocument(path: string, content: string) {
  return {
    path,
    content,
    hash,
    size: new TextEncoder().encode(content).byteLength,
    modifiedAt,
  }
}

describe('CreateDocumentPanel', () => {
  it('validates the Markdown path without calling the API', async () => {
    const { client, wrapper } = mountPanel(vi.fn())

    await wrapper.get('form').trigger('submit')
    expect(wrapper.get('[role="alert"]').text()).toContain('경로를 입력')

    await wrapper.get('input[name="path"]').setValue('../escape.md')
    await wrapper.get('form').trigger('submit')
    expect(wrapper.get('[role="alert"]').text()).toContain('올바른 Markdown 경로')
    expect(client.createFile).not.toHaveBeenCalled()
  })

  it('creates an empty document when the optional title is blank', async () => {
    const { client, wrapper } = mountPanel(async (path, content) => createdDocument(path, content))

    await wrapper.get('input[name="path"]').setValue('notes/empty.md')
    await wrapper.get('form').trigger('submit')
    await flushPromises()

    expect(client.createFile).toHaveBeenCalledWith('notes/empty.md', '', expect.any(AbortSignal))
    expect(wrapper.emitted('created')).toEqual([['notes/empty.md']])
  })

  it('creates title content and blocks duplicate submit while pending', async () => {
    let resolveCreate: ((document: ReturnType<typeof createdDocument>) => void) | undefined
    const { client, wrapper } = mountPanel(
      (_path, _content) =>
        new Promise((resolve) => {
          resolveCreate = resolve
        }),
    )

    await wrapper.get('input[name="path"]').setValue('notes/new.md')
    await wrapper.get('input[name="title"]').setValue('새 문서')
    await wrapper.get('form').trigger('submit')
    await wrapper.get('form').trigger('submit')

    expect(wrapper.get('[role="status"]').text()).toContain('생성 중')
    expect(client.createFile).toHaveBeenCalledTimes(1)

    resolveCreate?.(createdDocument('notes/new.md', '# 새 문서\n'))
    await flushPromises()
    expect(client.createFile).toHaveBeenCalledWith(
      'notes/new.md',
      '# 새 문서\n',
      expect.any(AbortSignal),
    )
    expect(wrapper.emitted('created')).toEqual([['notes/new.md']])
  })

  it('shows a duplicate conflict and retries without closing', async () => {
    let attempts = 0
    const { client, wrapper } = mountPanel(async (path, content) => {
      attempts += 1
      if (attempts === 1) {
        throw new MarkdownClientError(
          'file_already_exists',
          '같은 경로의 문서가 이미 존재합니다.',
          409,
        )
      }
      return createdDocument(path, content)
    })

    await wrapper.get('input[name="path"]').setValue('notes/existing.md')
    await wrapper.get('form').trigger('submit')
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain('이미 존재')

    await wrapper.get('[role="alert"] button').trigger('click')
    await flushPromises()
    expect(client.createFile).toHaveBeenCalledTimes(2)
    expect(wrapper.emitted('created')).toEqual([['notes/existing.md']])
  })
})
