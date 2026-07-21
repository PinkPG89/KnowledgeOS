import { createPinia } from 'pinia'
import { mount } from '@vue/test-utils'
import { createMemoryHistory } from 'vue-router'
import { describe, expect, it } from 'vitest'

import App from '@/App.vue'
import { createAppRouter } from '@/router'

async function mountAt(path: string) {
  const router = createAppRouter(createMemoryHistory())
  await router.push(path)
  await router.isReady()

  return mount(App, {
    global: {
      plugins: [createPinia(), router],
    },
  })
}

describe('KnowledgeOS app shell', () => {
  it('renders the workspace route', async () => {
    const wrapper = await mountAt('/')

    expect(wrapper.get('header').text()).toContain('KnowledgeOS')
    expect(wrapper.get('main').text()).toContain('Vue 3 PWA Shell')
  })

  it('renders the not-found route inside the shell', async () => {
    const wrapper = await mountAt('/missing-view')

    expect(wrapper.get('header').text()).toContain('KnowledgeOS')
    expect(wrapper.get('main').text()).toContain('요청한 화면을 찾을 수 없습니다.')
  })
})
