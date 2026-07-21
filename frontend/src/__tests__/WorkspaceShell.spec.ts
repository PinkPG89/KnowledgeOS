import { createPinia } from 'pinia'
import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

import WorkspaceShell from '@/components/workspace/WorkspaceShell.vue'

import { createMatchMediaController } from './matchMedia'

function mountShell(desktop: boolean) {
  const controller = createMatchMediaController(desktop)
  vi.mocked(matchMedia).mockReturnValue(controller.mediaQuery)

  const wrapper = mount(WorkspaceShell, {
    global: {
      plugins: [createPinia()],
      stubs: { RouterLink: { template: '<a><slot /></a>' } },
    },
  })

  return { controller, wrapper }
}

describe('WorkspaceShell', () => {
  it('renders all three workspace regions on desktop', () => {
    const { wrapper } = mountShell(true)

    expect(wrapper.attributes('data-viewport')).toBe('desktop')
    expect(wrapper.get('#workspace-navigation').attributes('aria-hidden')).toBe('false')
    expect(wrapper.get('.editor-pane').text()).toContain('Markdown 작업공간')
    expect(wrapper.get('#workspace-inspector').attributes('aria-hidden')).toBe('false')
  })

  it('opens one mobile drawer and closes it with the backdrop', async () => {
    const { wrapper } = mountShell(false)

    await wrapper.get('[aria-label="파일 탐색 패널 전환"]').trigger('click')
    expect(wrapper.get('#workspace-navigation').attributes('aria-hidden')).toBe('false')
    expect(wrapper.get('#workspace-inspector').attributes('aria-hidden')).toBe('true')
    expect(wrapper.find('.workspace-backdrop').exists()).toBe(true)

    await wrapper.get('[aria-label="파일 정보 패널 전환"]').trigger('click')
    expect(wrapper.get('#workspace-navigation').attributes('aria-hidden')).toBe('true')
    expect(wrapper.get('#workspace-inspector').attributes('aria-hidden')).toBe('false')

    await wrapper.get('.workspace-backdrop').trigger('click')
    expect(wrapper.find('.workspace-backdrop').exists()).toBe(false)
  })

  it('closes a mobile drawer when the viewport becomes desktop', async () => {
    const { controller, wrapper } = mountShell(false)
    await wrapper.get('[aria-label="파일 탐색 패널 전환"]').trigger('click')

    controller.setMatches(true)
    await wrapper.vm.$nextTick()

    expect(wrapper.attributes('data-viewport')).toBe('desktop')
    expect(wrapper.find('.workspace-backdrop').exists()).toBe(false)
  })
})
