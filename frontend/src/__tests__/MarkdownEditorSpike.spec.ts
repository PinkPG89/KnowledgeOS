import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { describe, expect, it } from 'vitest'

import MarkdownEditorSpike from '@/components/editor/MarkdownEditorSpike.vue'

function editorText(wrapper: ReturnType<typeof mount>) {
  return wrapper.get('[role="textbox"]').text()
}

describe('MarkdownEditorSpike', () => {
  it('renders Korean Markdown and exposes an accessible editing surface', () => {
    const wrapper = mount(MarkdownEditorSpike, {
      props: {
        modelValue: '# 첫 메모\n한글 본문',
        ariaLabel: '첫 메모 편집기',
      },
      attachTo: document.body,
    })

    expect(wrapper.get('[role="textbox"]').attributes('aria-label')).toBe('첫 메모 편집기')
    expect(editorText(wrapper)).toContain('첫 메모')
    expect(editorText(wrapper)).toContain('한글 본문')
    expect(wrapper.get('[role="toolbar"]').findAll('button')).toHaveLength(6)
  })

  it('applies toolbar formatting and emits the complete draft', async () => {
    const wrapper = mount(MarkdownEditorSpike, {
      props: { modelValue: '본문' },
      attachTo: document.body,
    })

    await wrapper.get('button[aria-label="굵게"]').trigger('click')

    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual(['****본문'])
    expect(editorText(wrapper)).toBe('****본문')
  })

  it('defers an external replacement until Korean composition ends', async () => {
    const wrapper = mount(MarkdownEditorSpike, {
      props: { modelValue: '입력 중' },
      attachTo: document.body,
    })
    const content = wrapper.get('[role="textbox"]')

    await content.trigger('compositionstart')
    await wrapper.setProps({ modelValue: '서버에서 바뀐 값' })
    expect(editorText(wrapper)).toBe('입력 중')

    await content.trigger('compositionend')
    await nextTick()
    expect(editorText(wrapper)).toBe('서버에서 바뀐 값')
  })

  it('keeps a large document virtualized instead of rendering every line', () => {
    const lineCount = 30_000
    const content = Array.from(
      { length: lineCount },
      (_, index) => `${index} KnowledgeOS 한글`,
    ).join('\n')
    const wrapper = mount(MarkdownEditorSpike, {
      props: { modelValue: content },
      attachTo: document.body,
    })

    expect(new TextEncoder().encode(content).byteLength).toBeGreaterThan(500_000)
    expect(wrapper.findAll('.cm-line').length).toBeLessThan(lineCount)
  })
})
