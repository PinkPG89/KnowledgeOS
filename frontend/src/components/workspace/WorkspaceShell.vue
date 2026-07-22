<script setup lang="ts">
import NetworkStatus from '@/components/NetworkStatus.vue'
import MarkdownDocumentPane from '@/components/editor/MarkdownDocumentPane.vue'
import FileTreePanel from '@/components/tree/FileTreePanel.vue'
import { useResponsiveLayout } from '@/composables/useResponsiveLayout'
import { getBrowserStorage } from '@/utils/browserStorage'

const layout = useResponsiveLayout()
const emit = defineEmits<{ openFile: [path: string] }>()

function toggleNavigation() {
  layout.toggleNavigation(getBrowserStorage())
}

function toggleInspector() {
  layout.toggleInspector(getBrowserStorage())
}

function handleEscape(event: KeyboardEvent) {
  if (event.key === 'Escape') layout.closeMobilePanel()
}
</script>

<template>
  <div class="workspace-shell" :data-viewport="layout.viewportMode" @keydown="handleEscape">
    <header class="workspace-topbar">
      <div class="workspace-topbar__primary">
        <button
          class="icon-button"
          type="button"
          aria-label="파일 탐색 패널 전환"
          aria-controls="workspace-navigation"
          :aria-expanded="layout.navigationVisible"
          @click="toggleNavigation"
        >
          <span aria-hidden="true">☰</span>
        </button>
        <RouterLink class="workspace-brand" to="/" aria-label="KnowledgeOS 작업공간">
          <span class="workspace-brand__mark" aria-hidden="true">K</span>
          <span>KnowledgeOS</span>
        </RouterLink>
      </div>

      <div class="workspace-topbar__actions">
        <NetworkStatus />
        <button
          class="icon-button"
          type="button"
          aria-label="파일 정보 패널 전환"
          aria-controls="workspace-inspector"
          :aria-expanded="layout.inspectorVisible"
          @click="toggleInspector"
        >
          <span aria-hidden="true">ⓘ</span>
        </button>
      </div>
    </header>

    <div
      class="workspace-grid"
      :class="{
        'workspace-grid--without-navigation': !layout.desktopNavigationOpen,
        'workspace-grid--without-inspector': !layout.desktopInspectorOpen,
      }"
    >
      <aside
        id="workspace-navigation"
        class="workspace-panel workspace-panel--navigation"
        :class="{ 'workspace-panel--open': layout.navigationVisible }"
        :aria-hidden="!layout.navigationVisible"
        :inert="!layout.navigationVisible"
        aria-labelledby="navigation-title"
      >
        <div class="workspace-panel__header">
          <div>
            <p>Vault</p>
            <h2 id="navigation-title">파일 탐색</h2>
          </div>
          <button
            v-if="layout.viewportMode === 'mobile'"
            class="icon-button"
            type="button"
            aria-label="파일 탐색 패널 닫기"
            @click="layout.closeMobilePanel"
          >
            <span aria-hidden="true">×</span>
          </button>
        </div>
        <FileTreePanel @open-file="emit('openFile', $event)" />
      </aside>

      <main class="editor-pane" aria-labelledby="editor-title">
        <MarkdownDocumentPane />
      </main>

      <aside
        id="workspace-inspector"
        class="workspace-panel workspace-panel--inspector"
        :class="{ 'workspace-panel--open': layout.inspectorVisible }"
        :aria-hidden="!layout.inspectorVisible"
        :inert="!layout.inspectorVisible"
        aria-labelledby="inspector-title"
      >
        <div class="workspace-panel__header">
          <div>
            <p>Document</p>
            <h2 id="inspector-title">파일 정보</h2>
          </div>
          <button
            v-if="layout.viewportMode === 'mobile'"
            class="icon-button"
            type="button"
            aria-label="파일 정보 패널 닫기"
            @click="layout.closeMobilePanel"
          >
            <span aria-hidden="true">×</span>
          </button>
        </div>
        <dl class="metadata-list">
          <div>
            <dt>상태</dt>
            <dd>읽기 전용 예시</dd>
          </div>
          <div>
            <dt>형식</dt>
            <dd>Markdown</dd>
          </div>
          <div>
            <dt>저장</dt>
            <dd>Vault 원본 파일</dd>
          </div>
        </dl>
      </aside>
    </div>

    <button
      v-if="layout.hasMobileOverlay"
      class="workspace-backdrop"
      type="button"
      aria-label="열린 패널 닫기"
      @click="layout.closeMobilePanel"
    />
  </div>
</template>

<style scoped>
.workspace-shell {
  min-height: 100dvh;
  overflow: hidden;
  background: var(--color-background);
}

.workspace-topbar {
  position: relative;
  z-index: 30;
  display: flex;
  min-height: 4rem;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.55rem max(0.75rem, env(safe-area-inset-right)) 0.55rem
    max(0.75rem, env(safe-area-inset-left));
  border-bottom: 1px solid var(--color-border);
  background: color-mix(in srgb, var(--color-surface) 94%, transparent);
  backdrop-filter: blur(1rem);
}

.workspace-topbar__primary,
.workspace-topbar__actions,
.workspace-brand {
  display: flex;
  align-items: center;
}

.workspace-topbar__primary,
.workspace-topbar__actions {
  gap: 0.55rem;
}

.workspace-brand {
  gap: 0.6rem;
  color: var(--color-text);
  font-size: 0.92rem;
  font-weight: 800;
  text-decoration: none;
}

.workspace-brand__mark {
  display: grid;
  width: 2rem;
  height: 2rem;
  place-items: center;
  border-radius: 0.6rem;
  background: var(--color-text);
  color: var(--color-background);
  font-size: 0.82rem;
}

.icon-button {
  display: grid;
  width: 2.75rem;
  height: 2.75rem;
  flex: 0 0 auto;
  place-items: center;
  border: 1px solid var(--color-border);
  border-radius: 0.8rem;
  background: var(--color-surface);
  color: var(--color-text);
  cursor: pointer;
  font: inherit;
  font-size: 1.15rem;
}

.icon-button:hover {
  border-color: var(--color-border-strong);
  background: var(--color-surface-muted);
}

.workspace-grid {
  display: grid;
  grid-template-columns: 17rem minmax(0, 1fr) 18rem;
  height: calc(100dvh - 4rem);
}

.workspace-grid--without-navigation {
  grid-template-columns: 0 minmax(0, 1fr) 18rem;
}

.workspace-grid--without-inspector {
  grid-template-columns: 17rem minmax(0, 1fr) 0;
}

.workspace-grid--without-navigation.workspace-grid--without-inspector {
  grid-template-columns: 0 minmax(0, 1fr) 0;
}

.workspace-panel,
.editor-pane {
  min-width: 0;
  min-height: 0;
}

.workspace-panel {
  overflow: hidden auto;
  background: var(--color-surface);
  visibility: visible;
}

.workspace-panel[aria-hidden='true'] {
  visibility: hidden;
}

.workspace-panel--navigation {
  border-right: 1px solid var(--color-border);
}

.workspace-panel--inspector {
  border-left: 1px solid var(--color-border);
}

.workspace-panel__header {
  display: flex;
  min-height: 5rem;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem;
  border-bottom: 1px solid var(--color-border);
}

.workspace-panel__header p,
.workspace-panel__header h2 {
  margin: 0;
}

.workspace-panel__header p {
  color: var(--color-accent);
  font-size: 0.68rem;
  font-weight: 800;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.workspace-panel__header h2 {
  margin-top: 0.25rem;
  font-size: 1rem;
}

.editor-pane {
  overflow: auto;
  background:
    linear-gradient(90deg, transparent 0, rgb(31 104 69 / 3%) 50%, transparent 100%),
    var(--color-background);
}

.metadata-list {
  margin: 0;
  padding: 0.5rem 1rem;
}

.metadata-list div {
  display: grid;
  grid-template-columns: 4rem minmax(0, 1fr);
  gap: 0.75rem;
  padding: 0.9rem 0;
  border-bottom: 1px solid var(--color-border);
  font-size: 0.8rem;
}

.metadata-list dt {
  color: var(--color-text-muted);
}

.metadata-list dd {
  margin: 0;
  overflow-wrap: anywhere;
}

.workspace-backdrop {
  display: none;
}

@media (max-width: 63.999rem) {
  .workspace-grid,
  .workspace-grid--without-navigation,
  .workspace-grid--without-inspector,
  .workspace-grid--without-navigation.workspace-grid--without-inspector {
    display: block;
    height: calc(100dvh - 4rem);
  }

  .workspace-panel {
    position: fixed;
    z-index: 50;
    top: 0;
    bottom: 0;
    width: min(86vw, 21rem);
    padding-top: env(safe-area-inset-top);
    box-shadow: var(--shadow-elevated);
    transition:
      visibility 180ms,
      transform 180ms ease;
  }

  .workspace-panel--navigation {
    left: 0;
    transform: translateX(-105%);
  }

  .workspace-panel--inspector {
    right: 0;
    transform: translateX(105%);
  }

  .workspace-panel--open {
    transform: translateX(0);
    visibility: visible;
  }

  .workspace-backdrop {
    position: fixed;
    z-index: 40;
    inset: 0;
    display: block;
    width: 100%;
    border: 0;
    background: rgb(17 24 19 / 46%);
    cursor: pointer;
  }

  .editor-pane {
    height: 100%;
  }
}

@media (prefers-reduced-motion: reduce) {
  .workspace-panel {
    transition: none;
  }
}
</style>
