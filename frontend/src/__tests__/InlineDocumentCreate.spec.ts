import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

import InlineDocumentCreate from '@/components/tree/InlineDocumentCreate.vue'
import { MarkdownClientError, type MarkdownCreateClient } from '@/services/markdownClient'

const hash = `sha256:${'a'.repeat(64)}`
const modifiedAt = '2026-07-26T01:02:03.004Z'

function mountCreate(parentPath: string, createFile: MarkdownCreateClient['createFile']) {
  const client: MarkdownCreateClient = { createFile: vi.fn(createFile) }
  const wrapper = mount(InlineDocumentCreate, {
    attachTo: document.body,
    props: { client, parentPath },
  })
  return { client, wrapper }
}

function emptyDocument(path: string) {
  return { path, content: '', hash, size: 0, modifiedAt }
}

describe('InlineDocumentCreate', () => {
  it('creates in the supplied parent and appends the Markdown extension', async () => {
    const { client, wrapper } = mountCreate('projects', async (path) => emptyDocument(path))

    await wrapper.get('input').setValue('새 문서')
    await wrapper.get('form').trigger('submit')
    await flushPromises()

    expect(client.createFile).toHaveBeenCalledWith(
      'projects/새 문서.md',
      '',
      expect.any(AbortSignal),
    )
    expect(wrapper.emitted('created')).toEqual([['projects/새 문서.md']])
  })

  it('normalizes an uppercase Markdown extension', async () => {
    const { client, wrapper } = mountCreate('', async (path) => emptyDocument(path))

    await wrapper.get('input').setValue('Note.MD')
    await wrapper.get('form').trigger('submit')
    await flushPromises()

    expect(client.createFile).toHaveBeenCalledWith('Note.md', '', expect.any(AbortSignal))
  })

  it.each(['', '../escape', 'nested/note', '.hidden'])(
    'rejects an invalid file name: %s',
    async (fileName) => {
      const { client, wrapper } = mountCreate('', vi.fn())

      await wrapper.get('input').setValue(fileName)
      await wrapper.get('form').trigger('submit')

      expect(wrapper.get('[role="alert"]').text()).toContain('파일명만 입력')
      expect(client.createFile).not.toHaveBeenCalled()
    },
  )

  it('keeps the inline form open after a duplicate conflict', async () => {
    const { wrapper } = mountCreate('', async () => {
      throw new MarkdownClientError(
        'file_already_exists',
        '같은 경로의 문서가 이미 존재합니다.',
        409,
      )
    })

    await wrapper.get('input').setValue('existing')
    await wrapper.get('form').trigger('submit')
    await flushPromises()

    expect(wrapper.get('[role="alert"]').text()).toContain('이미 존재')
    expect(wrapper.find('input').exists()).toBe(true)
  })

  it('cancels inline creation with Escape', async () => {
    const { wrapper } = mountCreate('', vi.fn())

    await wrapper.get('input').trigger('keydown', { key: 'Escape' })

    expect(wrapper.emitted('cancel')).toHaveLength(1)
  })
})
