import { createApp } from 'vue'
import { createPinia } from 'pinia'

import App from './App.vue'
import router from './router'
import { registerPwa } from './pwa'
import { useNetworkStore } from './stores/network'
import './assets/main.css'

const app = createApp(App)
const pinia = createPinia()

app.use(pinia)
app.use(router)

useNetworkStore(pinia).startListening()
app.mount('#app')

void registerPwa(router, pinia)
