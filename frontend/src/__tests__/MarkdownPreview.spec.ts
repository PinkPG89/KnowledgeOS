import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

import MarkdownPreview from '@/components/editor/MarkdownPreview.vue'

describe('MarkdownPreview', () => {
  it('renders headings, emphasis, lists, tables and code as HTML', () => {
    const wrapper = mount(MarkdownPreview, {
      props: {
        source: [
          '# 운영 가이드',
          '',
          '**중요한 설정**',
          '',
          '- 첫 번째',
          '- 두 번째',
          '',
          '| 항목 | 상태 |',
          '| --- | --- |',
          '| API | 정상 |',
          '',
          '`cargo test`',
        ].join('\n'),
      },
    })

    expect(wrapper.get('h1').text()).toBe('운영 가이드')
    expect(wrapper.get('strong').text()).toBe('중요한 설정')
    expect(wrapper.findAll('li')).toHaveLength(2)
    expect(wrapper.get('table').text()).toContain('API')
    expect(wrapper.get('code').text()).toBe('cargo test')
  })

  it('reactively renders the latest draft', async () => {
    const wrapper = mount(MarkdownPreview, {
      props: { source: '# 처음' },
    })

    await wrapper.setProps({ source: '## 변경됨' })

    expect(wrapper.find('h1').exists()).toBe(false)
    expect(wrapper.get('h2').text()).toBe('변경됨')
  })

  it('blocks raw HTML and unsafe link protocols', () => {
    const wrapper = mount(MarkdownPreview, {
      props: {
        source: [
          '<script>alert("xss")</script>',
          '<img src="x" onerror="alert(1)">',
          '[위험 링크](javascript:alert(1))',
          '[안전 링크](https://example.com)',
        ].join('\n\n'),
      },
    })

    expect(wrapper.find('script').exists()).toBe(false)
    expect(wrapper.find('img').exists()).toBe(false)
    expect(wrapper.find('a[href^="javascript:"]').exists()).toBe(false)
    expect(wrapper.get('a[href="https://example.com"]').attributes('rel')).toBe(
      'noopener noreferrer',
    )
  })

  it('renders copy controls for fenced and indented code blocks', () => {
    const wrapper = mount(MarkdownPreview, {
      props: {
        source: ['```bash', 'echo "안녕"', '```', '', '    cargo test'].join('\n'),
      },
    })

    const blocks = wrapper.findAll('.markdown-code-block')
    expect(blocks).toHaveLength(2)
    expect(blocks[0]?.get('.markdown-code-block__language').text()).toBe('bash')
    expect(blocks[1]?.get('.markdown-code-block__language').text()).toBe('code')
    expect(wrapper.findAll('[data-copy-code]')).toHaveLength(2)
  })

  it('copies only the selected code block and announces success', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    const restoreClipboard = replaceClipboard({ writeText })
    const wrapper = mount(MarkdownPreview, {
      props: {
        source: ['```text', 'first', '```', '', '```text', '두 번째', '```'].join('\n'),
      },
    })

    try {
      const buttons = wrapper.findAll<HTMLButtonElement>('[data-copy-code]')
      await buttons[1]?.trigger('click')
      await flushPromises()

      expect(writeText).toHaveBeenCalledExactlyOnceWith('두 번째\n')
      expect(buttons[1]?.text()).toBe('복사됨')
      expect(buttons[1]?.attributes('data-copy-state')).toBe('success')
      expect(buttons[0]?.text()).toBe('복사')
    } finally {
      wrapper.unmount()
      restoreClipboard()
    }
  })

  it('shows a failure state when clipboard and fallback copy both fail', async () => {
    const restoreClipboard = replaceClipboard({
      writeText: vi.fn().mockRejectedValue(new DOMException('Denied', 'NotAllowedError')),
    })
    const originalExecCommand = document.execCommand
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: vi.fn(() => false),
    })
    const wrapper = mount(MarkdownPreview, {
      props: { source: ['```', 'cannot copy', '```'].join('\n') },
    })

    try {
      const button = wrapper.get<HTMLButtonElement>('[data-copy-code]')
      await button.trigger('click')
      await flushPromises()

      expect(button.text()).toBe('복사 실패')
      expect(button.attributes('data-copy-state')).toBe('error')
    } finally {
      wrapper.unmount()
      restoreClipboard()
      Object.defineProperty(document, 'execCommand', {
        configurable: true,
        value: originalExecCommand,
      })
    }
  })
})

function replaceClipboard(clipboard: Pick<Clipboard, 'writeText'>) {
  const descriptor = Object.getOwnPropertyDescriptor(navigator, 'clipboard')
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: clipboard,
  })

  return () => {
    if (descriptor) {
      Object.defineProperty(navigator, 'clipboard', descriptor)
    } else {
      Reflect.deleteProperty(navigator, 'clipboard')
    }
  }
}
