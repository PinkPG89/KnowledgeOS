import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

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
})
