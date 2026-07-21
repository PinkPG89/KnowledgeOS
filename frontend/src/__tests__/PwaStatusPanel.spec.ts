import { createPinia, setActivePinia } from 'pinia'
import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it } from 'vitest'

import PwaStatusPanel from '@/components/PwaStatusPanel.vue'
import { usePwaStore } from '@/stores/pwa'

describe('PwaStatusPanel', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('shows and dismisses the offline-ready state', async () => {
    const store = usePwaStore()
    store.markOfflineReady()
    const wrapper = mount(PwaStatusPanel)

    expect(wrapper.text()).toContain('오프라인 App shell 준비 완료')
    await wrapper.get('button').trigger('click')
    expect(wrapper.find('aside').exists()).toBe(false)
  })

  it('requires an explicit action when an update is available', () => {
    const store = usePwaStore()
    store.markUpdateAvailable()
    const wrapper = mount(PwaStatusPanel)

    expect(wrapper.text()).toContain('새 버전을 사용할 수 있습니다.')
    expect(wrapper.get('button').text()).toBe('업데이트')
  })
})
