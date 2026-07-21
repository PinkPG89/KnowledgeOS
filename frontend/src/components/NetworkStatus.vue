<script setup lang="ts">
import { storeToRefs } from 'pinia'

import { useNetworkStore } from '@/stores/network'

const network = useNetworkStore()
const { isOnline, label } = storeToRefs(network)
</script>

<template>
  <div
    class="network-status"
    :class="{ 'network-status--offline': !isOnline }"
    role="status"
    aria-live="polite"
  >
    <span class="network-status__dot" aria-hidden="true"></span>
    <span>{{ label }}</span>
    <span class="network-status__detail">
      {{ isOnline ? '서버 연결 사용 가능' : 'App shell만 사용 가능' }}
    </span>
  </div>
</template>

<style scoped>
.network-status {
  display: inline-flex;
  min-height: 2rem;
  align-items: center;
  gap: 0.45rem;
  color: var(--color-text-muted);
  font-size: 0.8rem;
  font-weight: 650;
}

.network-status__dot {
  width: 0.55rem;
  height: 0.55rem;
  border-radius: 999px;
  background: var(--color-success);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-success) 18%, transparent);
}

.network-status--offline .network-status__dot {
  background: var(--color-warning);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-warning) 20%, transparent);
}

.network-status__detail {
  font-weight: 450;
}

@media (max-width: 40rem) {
  .network-status__detail {
    display: none;
  }
}
</style>
