<script setup lang="ts">
import NetworkStatus from '@/components/NetworkStatus.vue'
import PwaStatusPanel from '@/components/PwaStatusPanel.vue'
</script>

<template>
  <div class="app-shell">
    <header class="app-header">
      <RouterLink class="app-brand" to="/" aria-label="KnowledgeOS 홈">
        <span class="app-brand__mark" aria-hidden="true">K</span>
        <span>KnowledgeOS</span>
      </RouterLink>
      <NetworkStatus />
    </header>

    <RouterView />
    <PwaStatusPanel />
  </div>
</template>

<style scoped>
.app-shell {
  min-height: 100dvh;
}

.app-header {
  position: sticky;
  z-index: 10;
  top: 0;
  display: flex;
  min-height: 4rem;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.6rem max(1rem, env(safe-area-inset-right)) 0.6rem max(1rem, env(safe-area-inset-left));
  border-bottom: 1px solid color-mix(in srgb, var(--color-border) 75%, transparent);
  background: color-mix(in srgb, var(--color-background) 88%, transparent);
  backdrop-filter: blur(1rem);
}

.app-brand {
  display: inline-flex;
  align-items: center;
  gap: 0.65rem;
  color: var(--color-text);
  font-size: 0.95rem;
  font-weight: 800;
  text-decoration: none;
}

.app-brand__mark {
  display: grid;
  width: 2.2rem;
  height: 2.2rem;
  place-items: center;
  border-radius: 0.65rem;
  background: var(--color-text);
  color: var(--color-background);
  font-size: 0.9rem;
}
</style>
