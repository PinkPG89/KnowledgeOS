import DOMPurify from 'dompurify'
import MarkdownIt from 'markdown-it'

const markdown = new MarkdownIt({
  breaks: true,
  html: false,
  linkify: true,
  typographer: false,
})

const defaultLinkOpen =
  markdown.renderer.rules.link_open ??
  ((tokens, index, options, _environment, renderer) => renderer.renderToken(tokens, index, options))

markdown.renderer.rules.link_open = (tokens, index, options, environment, renderer) => {
  const token = tokens[index]
  token?.attrSet('rel', 'noopener noreferrer')
  return defaultLinkOpen(tokens, index, options, environment, renderer)
}

const defaultFence = markdown.renderer.rules.fence
const defaultCodeBlock = markdown.renderer.rules.code_block

markdown.renderer.rules.fence = (tokens, index, options, environment, renderer) => {
  const token = tokens[index]
  const renderedCode = defaultFence
    ? defaultFence(tokens, index, options, environment, renderer)
    : renderPlainCodeBlock(token?.content ?? '')
  const language = token?.info.trim().split(/\s+/u)[0] ?? ''
  return renderCodeBlockContainer(renderedCode, language)
}

markdown.renderer.rules.code_block = (tokens, index, options, environment, renderer) => {
  const token = tokens[index]
  const renderedCode = defaultCodeBlock
    ? defaultCodeBlock(tokens, index, options, environment, renderer)
    : renderPlainCodeBlock(token?.content ?? '')
  return renderCodeBlockContainer(renderedCode, '')
}

export function renderMarkdown(source: string): string {
  return DOMPurify.sanitize(markdown.render(source), {
    FORBID_ATTR: ['style'],
    FORBID_TAGS: ['style'],
    USE_PROFILES: { html: true },
  })
}

function renderPlainCodeBlock(content: string): string {
  return `<pre><code>${markdown.utils.escapeHtml(content)}</code></pre>\n`
}

function renderCodeBlockContainer(renderedCode: string, language: string): string {
  const escapedLanguage = markdown.utils.escapeHtml(language)
  const languageLabel = escapedLanguage || 'code'
  return [
    '<div class="markdown-code-block">',
    '<div class="markdown-code-block__toolbar">',
    `<span class="markdown-code-block__language">${languageLabel}</span>`,
    '<button type="button" class="markdown-code-block__copy" data-copy-code',
    ' aria-label="코드 블록 복사" aria-live="polite">복사</button>',
    '</div>',
    renderedCode,
    '</div>\n',
  ].join('')
}
