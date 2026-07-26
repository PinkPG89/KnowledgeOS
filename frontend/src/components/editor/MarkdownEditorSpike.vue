<script setup lang="ts">
import { markdown } from '@codemirror/lang-markdown'
import { EditorView, minimalSetup } from 'codemirror'
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

const props = withDefaults(
  defineProps<{
    modelValue: string
    ariaLabel?: string
  }>(),
  {
    ariaLabel: 'Markdown 편집기',
  },
)

const emit = defineEmits<{
  'update:modelValue': [value: string]
  'composition-change': [active: boolean]
}>()

const editorHost = ref<HTMLElement | null>(null)
let editorView: EditorView | null = null
let compositionActive = false
let pendingExternalValue: string | null = null

onMounted(() => {
  if (!editorHost.value) return

  editorView = new EditorView({
    doc: props.modelValue,
    parent: editorHost.value,
    extensions: [
      minimalSetup,
      markdown(),
      EditorView.lineWrapping,
      EditorView.contentAttributes.of({
        'aria-label': props.ariaLabel,
        autocapitalize: 'sentences',
        autocorrect: 'on',
        spellcheck: 'true',
      }),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) emit('update:modelValue', update.state.doc.toString())
      }),
      EditorView.domEventHandlers({
        compositionstart: () => {
          compositionActive = true
          emit('composition-change', true)
          return false
        },
        compositionend: () => {
          compositionActive = false
          emit('composition-change', false)
          queueMicrotask(applyPendingExternalValue)
          return false
        },
      }),
    ],
  })
})

onBeforeUnmount(() => {
  editorView?.destroy()
  editorView = null
})

watch(
  () => props.modelValue,
  (value) => {
    if (!editorView || value === editorView.state.doc.toString()) return

    if (compositionActive || editorView.composing) {
      pendingExternalValue = value
      return
    }

    replaceDocument(value)
  },
)

function replaceDocument(value: string) {
  if (!editorView) return

  pendingExternalValue = null
  editorView.dispatch({
    changes: {
      from: 0,
      to: editorView.state.doc.length,
      insert: value,
    },
  })
}

function applyPendingExternalValue() {
  if (pendingExternalValue === null) return
  replaceDocument(pendingExternalValue)
}

function wrapSelection(before: string, after = before) {
  if (!editorView) return

  const selection = editorView.state.selection.main
  const selectedText = editorView.state.sliceDoc(selection.from, selection.to)
  const insertedText = `${before}${selectedText}${after}`
  const cursorFrom = selection.from + before.length
  const cursorTo = cursorFrom + selectedText.length

  editorView.dispatch({
    changes: { from: selection.from, to: selection.to, insert: insertedText },
    selection: { anchor: cursorFrom, head: cursorTo },
  })
  editorView.focus()
}

function prefixCurrentLine(prefix: string) {
  if (!editorView) return

  const line = editorView.state.doc.lineAt(editorView.state.selection.main.head)
  editorView.dispatch({
    changes: { from: line.from, insert: prefix },
    selection: {
      anchor: editorView.state.selection.main.anchor + prefix.length,
      head: editorView.state.selection.main.head + prefix.length,
    },
  })
  editorView.focus()
}

function prefixSelectedLines(prefix: string | ((index: number) => string)) {
  if (!editorView) return

  const selection = editorView.state.selection.main
  const endPosition =
    selection.to > selection.from && editorView.state.doc.lineAt(selection.to).from === selection.to
      ? selection.to - 1
      : selection.to
  const firstLine = editorView.state.doc.lineAt(selection.from)
  const lastLine = editorView.state.doc.lineAt(endPosition)
  const source = editorView.state.sliceDoc(firstLine.from, lastLine.to)
  const lines = source.split('\n')
  const prefixes = lines.map((_, index) => (typeof prefix === 'string' ? prefix : prefix(index)))
  const insertedText = lines.map((line, index) => `${prefixes[index]}${line}`).join('\n')
  const leadingPrefixLength = prefixes[0]?.length ?? 0
  const totalPrefixLength = prefixes.reduce((total, value) => total + value.length, 0)

  editorView.dispatch({
    changes: { from: firstLine.from, to: lastLine.to, insert: insertedText },
    selection: selection.empty
      ? { anchor: selection.head + leadingPrefixLength }
      : {
          anchor: selection.from + leadingPrefixLength,
          head: selection.to + totalPrefixLength,
        },
  })
  editorView.focus()
}

function insertCodeBlock() {
  wrapSelection('```\n', '\n```')
}

function insertHorizontalRule() {
  if (!editorView) return

  const selection = editorView.state.selection.main
  const line = editorView.state.doc.lineAt(selection.head)
  const insert = line.length === 0 ? '---' : '---\n'

  editorView.dispatch({
    changes: { from: line.from, insert },
    selection: { anchor: line.from + insert.length },
  })
  editorView.focus()
}

function focusEditorAfterPointer(event: PointerEvent) {
  event.preventDefault()
  void nextTick(() => editorView?.focus())
}
</script>

<template>
  <section class="markdown-editor" aria-label="Markdown 편집 도구">
    <div class="markdown-editor__toolbar" role="toolbar" aria-label="Markdown 서식">
      <button
        type="button"
        aria-label="제목 추가"
        title="제목"
        @pointerdown="focusEditorAfterPointer"
        @click="prefixCurrentLine('# ')"
      >
        H1
      </button>
      <button
        type="button"
        aria-label="두 번째 수준 제목 추가"
        title="제목 2"
        @pointerdown="focusEditorAfterPointer"
        @click="prefixCurrentLine('## ')"
      >
        H2
      </button>
      <span class="markdown-editor__separator" aria-hidden="true" />
      <button
        type="button"
        aria-label="굵게"
        title="굵게"
        @pointerdown="focusEditorAfterPointer"
        @click="wrapSelection('**')"
      >
        <strong>B</strong>
      </button>
      <button
        type="button"
        aria-label="기울임"
        title="기울임"
        @pointerdown="focusEditorAfterPointer"
        @click="wrapSelection('*')"
      >
        <em>I</em>
      </button>
      <button
        type="button"
        aria-label="취소선"
        title="취소선"
        @pointerdown="focusEditorAfterPointer"
        @click="wrapSelection('~~')"
      >
        <s>S</s>
      </button>
      <button
        type="button"
        aria-label="인라인 코드"
        title="인라인 코드"
        @pointerdown="focusEditorAfterPointer"
        @click="wrapSelection('`')"
      >
        <code>&lt;/&gt;</code>
      </button>
      <span class="markdown-editor__separator" aria-hidden="true" />
      <button
        type="button"
        aria-label="목록 추가"
        title="목록"
        @pointerdown="focusEditorAfterPointer"
        @click="prefixSelectedLines('- ')"
      >
        •
      </button>
      <button
        type="button"
        aria-label="번호 목록 추가"
        title="번호 목록"
        @pointerdown="focusEditorAfterPointer"
        @click="prefixSelectedLines((index) => `${index + 1}. `)"
      >
        1.
      </button>
      <button
        type="button"
        aria-label="할 일 추가"
        title="할 일"
        @pointerdown="focusEditorAfterPointer"
        @click="prefixSelectedLines('- [ ] ')"
      >
        ☑
      </button>
      <button
        type="button"
        aria-label="인용문 추가"
        title="인용문"
        @pointerdown="focusEditorAfterPointer"
        @click="prefixSelectedLines('> ')"
      >
        ❯
      </button>
      <span class="markdown-editor__separator" aria-hidden="true" />
      <button
        type="button"
        aria-label="링크 추가"
        title="링크"
        @pointerdown="focusEditorAfterPointer"
        @click="wrapSelection('[', ']()')"
      >
        ↗
      </button>
      <button
        type="button"
        aria-label="코드 블록 추가"
        title="코드 블록"
        @pointerdown="focusEditorAfterPointer"
        @click="insertCodeBlock"
      >
        { }
      </button>
      <button
        type="button"
        aria-label="구분선 추가"
        title="구분선"
        @pointerdown="focusEditorAfterPointer"
        @click="insertHorizontalRule"
      >
        ―
      </button>
    </div>
    <div ref="editorHost" class="markdown-editor__surface" />
  </section>
</template>

<style scoped>
.markdown-editor {
  overflow: hidden;
  border: 1px solid var(--color-border);
  border-radius: 0.9rem;
  background: var(--color-surface);
}

.markdown-editor__toolbar {
  position: sticky;
  z-index: 1;
  top: 2.35rem;
  display: flex;
  gap: 0.25rem;
  overflow-x: auto;
  padding: 0.45rem;
  border-bottom: 1px solid var(--color-border);
  background: color-mix(in srgb, var(--color-surface) 94%, transparent);
  scrollbar-width: thin;
  backdrop-filter: blur(0.75rem);
}

.markdown-editor__toolbar button {
  flex: 0 0 2.75rem;
  min-width: 2.75rem;
  min-height: 2.75rem;
  padding: 0;
  border: 1px solid transparent;
  border-radius: 0.65rem;
  background: transparent;
  color: var(--color-text);
  cursor: pointer;
  font: inherit;
  font-weight: 700;
}

.markdown-editor__toolbar button:hover {
  border-color: var(--color-border);
  background: var(--color-surface-muted);
}

.markdown-editor__toolbar button:focus-visible {
  outline: 3px solid color-mix(in srgb, var(--color-accent) 40%, transparent);
  outline-offset: -3px;
}

.markdown-editor__toolbar code {
  font: inherit;
  font-size: 0.75rem;
  font-weight: 800;
}

.markdown-editor__separator {
  flex: 0 0 1px;
  align-self: stretch;
  margin: 0.35rem 0.15rem;
  background: var(--color-border);
}

.markdown-editor__surface :deep(.cm-editor) {
  min-height: min(60dvh, 44rem);
  background: transparent;
  color: var(--color-text);
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 0.95rem;
  line-height: 1.7;
}

.markdown-editor__surface :deep(.cm-scroller) {
  overflow: auto;
  font-family: inherit;
}

.markdown-editor__surface :deep(.cm-content) {
  min-height: min(60dvh, 44rem);
  padding: 1.25rem 0.75rem 5rem;
  caret-color: var(--color-accent);
}

.markdown-editor__surface :deep(.cm-gutters) {
  border-right-color: var(--color-border);
  background: var(--color-surface-muted);
  color: var(--color-text-muted);
}

.markdown-editor__surface :deep(.cm-activeLine),
.markdown-editor__surface :deep(.cm-activeLineGutter) {
  background: color-mix(in srgb, var(--color-accent) 7%, transparent);
}

.markdown-editor__surface :deep(.cm-focused) {
  outline: 3px solid color-mix(in srgb, var(--color-accent) 30%, transparent);
  outline-offset: -3px;
}

@media (max-width: 40rem) {
  .markdown-editor {
    border-right: 0;
    border-left: 0;
    border-radius: 0;
  }

  .markdown-editor__toolbar {
    top: 2.35rem;
  }

  .markdown-editor__surface :deep(.cm-content) {
    padding-right: 0.6rem;
    padding-left: 0.6rem;
  }
}
</style>
