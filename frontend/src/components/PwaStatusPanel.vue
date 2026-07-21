<script setup lang="ts">
import { storeToRefs } from 'pinia'

import { updatePwa } from '@/pwa'
import { usePwaStore } from '@/stores/pwa'

const pwa = usePwaStore()
const { offlineReady, updateAvailable, registrationFailed } = storeToRefs(pwa)
</script>

<template>
  <aside
    v-if="offlineReady || updateAvailable || registrationFailed"
    class="pwa-status"
    aria-live="polite"
    aria-label="애플리케이션 상태"
  >
    <template v-if="updateAvailable">
      <div>
        <strong>새 버전을 사용할 수 있습니다.</strong>
        <p>작성 중인 내용이 없는지 확인한 후 업데이트하세요.</p>
      </div>
      <button type="button" @click="updatePwa">업데이트</button>
    </template>

    <template v-else-if="registrationFailed">
      <div>
        <strong>오프라인 준비에 실패했습니다.</strong>
        <p>온라인 기능은 계속 사용할 수 있습니다.</p>
      </div>
    </template>

    <template v-else>
      <div>
        <strong>오프라인 App shell 준비 완료</strong>
        <p>네트워크가 끊겨도 기본 화면을 다시 열 수 있습니다.</p>
      </div>
      <button type="button" class="pwa-status__secondary" @click="pwa.dismissOfflineReady">
        확인
      </button>
    </template>
  </aside>
</template>

<style scoped>
.pwa-status {
  position: fixed;
  z-index: 20;
  right: max(1rem, env(safe-area-inset-right));
  bottom: max(1rem, env(safe-area-inset-bottom));
  display: flex;
  width: min(30rem, calc(100vw - 2rem));
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem;
  border: 1px solid var(--color-border-strong);
  border-radius: 1rem;
  background: var(--color-surface);
  box-shadow: var(--shadow-elevated);
}

.pwa-status strong,
.pwa-status p {
  display: block;
}

.pwa-status p {
  margin: 0.25rem 0 0;
  color: var(--color-text-muted);
  font-size: 0.82rem;
}

.pwa-status button {
  min-width: 5.5rem;
  min-height: 2.75rem;
  border: 0;
  border-radius: 0.75rem;
  background: var(--color-accent);
  color: var(--color-accent-contrast);
  cursor: pointer;
  font: inherit;
  font-weight: 700;
}

.pwa-status__secondary {
  background: var(--color-surface-muted) !important;
  color: var(--color-text) !important;
}
</style>
