import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { createPinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

import FileTreePanel from '@/components/tree/FileTreePanel.vue'
import type { TreeListing, TreeNode } from '@/models/tree'
import { TreeClientError, type TreeClient } from '@/services/treeClient'

const timestamp = '2026-07-22T01:02:03.004Z'

function directory(name: string, path = name): TreeNode {
  return { kind: 'directory', name, path, modifiedAt: timestamp }
}

function file(name: string, path = name): TreeNode {
  return { kind: 'file', name, path, size: 10, modifiedAt: timestamp }
}

function mountPanel(listDirectory: TreeClient['listDirectory']) {
  const client: TreeClient = { listDirectory: vi.fn(listDirectory) }
  const wrapper = mount(FileTreePanel, {
    props: { client },
    attachTo: document.body,
    global: { plugins: [createPinia()] },
  })
  return { client, wrapper }
}

describe('FileTreePanel', () => {
  it('loads the root and exposes treeitem metadata', async () => {
    const { wrapper } = mountPanel(async () => ({
      path: '',
      entries: [directory('projects'), file('README.md')],
    }))
    await flushPromises()

    const items = wrapper.findAll('[role="treeitem"]')
    expect(items).toHaveLength(2)
    expect(items[0]?.attributes()).toMatchObject({
      'aria-expanded': 'false',
      'aria-level': '1',
      'aria-posinset': '1',
      'aria-setsize': '2',
      tabindex: '0',
    })
    expect(items[1]?.attributes('aria-selected')).toBe('false')
  })

  it('shows root loading, error, retry, and empty states', async () => {
    let resolveRoot: ((listing: TreeListing) => void) | undefined
    let attempts = 0
    const { client, wrapper } = mountPanel(() => {
      attempts += 1
      if (attempts === 1) {
        return new Promise<TreeListing>((resolve) => {
          resolveRoot = resolve
        })
      }
      if (attempts === 2) {
        return Promise.reject(new TreeClientError('internal_error', 'Temporary failure', 500))
      }
      return Promise.resolve({ path: '', entries: [] })
    })

    await Promise.resolve()
    expect(wrapper.get('[role="status"]').text()).toContain('불러오는 중')
    resolveRoot?.({ path: '', entries: [] })
    await flushPromises()
    expect(wrapper.text()).toContain('Vault가 비어 있습니다.')

    await wrapper.get('[aria-label="파일 트리 새로고침"]').trigger('click')
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain('Temporary failure')

    await wrapper.get('[role="alert"] button').trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('Vault가 비어 있습니다.')
    expect(client.listDirectory).toHaveBeenCalledTimes(3)
  })

  it('loads a directory once and preserves empty children across collapse', async () => {
    const listDirectory = vi.fn(async (path: string) => {
      if (path === '') return { path, entries: [directory('projects')] }
      return { path, entries: [] }
    })
    const { wrapper } = mountPanel(listDirectory)
    await flushPromises()
    const projects = wrapper.get('[role="treeitem"]')

    await projects.trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('빈 폴더')
    expect(projects.attributes('aria-expanded')).toBe('true')

    await projects.trigger('click')
    await projects.trigger('click')
    await flushPromises()
    expect(listDirectory).toHaveBeenCalledTimes(2)
  })

  it('shows nested loading and supports inline error retry', async () => {
    let rejectChild: ((reason: TreeClientError) => void) | undefined
    let childAttempts = 0
    const { wrapper } = mountPanel((path) => {
      if (path === '') return Promise.resolve({ path, entries: [directory('projects')] })

      childAttempts += 1
      if (childAttempts === 1) {
        return new Promise<TreeListing>((_resolve, reject) => {
          rejectChild = reject
        })
      }
      return Promise.resolve({ path, entries: [] })
    })
    await flushPromises()

    await wrapper.get('[role="treeitem"]').trigger('click')
    await Promise.resolve()
    const directoryState = wrapper.get('[data-directory-state="projects"]')
    expect(directoryState.get('[role="status"]').text()).toContain('하위 항목을 불러오는 중')

    rejectChild?.(new TreeClientError('internal_error', 'Nested failure', 500))
    await flushPromises()
    expect(wrapper.get('[data-directory-state="projects"] [role="alert"]').text()).toContain(
      'Nested failure',
    )

    await wrapper.get('[data-directory-state="projects"] button').trigger('click')
    await flushPromises()
    expect(wrapper.get('[data-directory-state="projects"]').text()).toContain('빈 폴더')
    expect(childAttempts).toBe(2)
  })

  it('moves focus with Arrow, Home, and End keys', async () => {
    const { wrapper } = mountPanel(async () => ({
      path: '',
      entries: [directory('alpha'), file('middle.md'), file('zeta.md')],
    }))
    await flushPromises()
    const items = wrapper.findAll<HTMLElement>('[role="treeitem"]')

    items[0]?.element.focus()
    await items[0]?.trigger('keydown', { key: 'ArrowDown' })
    expect(document.activeElement).toBe(items[1]?.element)

    await items[1]?.trigger('keydown', { key: 'End' })
    expect(document.activeElement).toBe(items[2]?.element)

    await items[2]?.trigger('keydown', { key: 'Home' })
    expect(document.activeElement).toBe(items[0]?.element)

    await items[0]?.trigger('keydown', { key: 'ArrowUp' })
    expect(document.activeElement).toBe(items[0]?.element)
  })

  it('expands, enters a child, returns to its parent, and selects a file', async () => {
    const { wrapper } = mountPanel(async (path) => {
      if (path === '') return { path, entries: [directory('projects'), file('root.md')] }
      return { path, entries: [file('note.md', 'projects/note.md')] }
    })
    await flushPromises()
    let items = wrapper.findAll<HTMLElement>('[role="treeitem"]')

    await items[0]?.trigger('keydown', { key: 'ArrowRight' })
    await flushPromises()
    expect(items[0]?.attributes('aria-expanded')).toBe('true')

    await items[0]?.trigger('keydown', { key: 'ArrowRight' })
    items = wrapper.findAll<HTMLElement>('[role="treeitem"]')
    expect(document.activeElement).toBe(items[1]?.element)

    await items[1]?.trigger('keydown', { key: 'ArrowLeft' })
    expect(document.activeElement).toBe(items[0]?.element)

    await items[0]?.trigger('keydown', { key: 'ArrowLeft' })
    expect(items[0]?.attributes('aria-expanded')).toBe('false')

    items = wrapper.findAll<HTMLElement>('[role="treeitem"]')
    await items[1]?.trigger('keydown', { key: ' ' })
    expect(items[1]?.attributes('aria-selected')).toBe('true')
  })

  it('keeps tree rows and actions at the 44px touch target contract', () => {
    const source = readFileSync(
      resolve(process.cwd(), 'src/components/tree/FileTreePanel.vue'),
      'utf8',
    )

    expect(source).toMatch(/\.tree-item \{[\s\S]*?min-height: 2\.75rem;/)
    expect(source).toMatch(/\.file-tree__toolbar button,[\s\S]*?min-height: 2\.75rem;/)
  })
})
