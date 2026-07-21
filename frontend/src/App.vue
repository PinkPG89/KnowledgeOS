<script setup lang="ts">
import PwaStatusPanel from '@/components/PwaStatusPanel.vue'
</script>

<template>
  <div class="app-shell">
    <RouterView />
    <PwaStatusPanel />
  </div>
</template>

<style scoped>
.app-shell {
  min-height: 100dvh;
}
</style>
