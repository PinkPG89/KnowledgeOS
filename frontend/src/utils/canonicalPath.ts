export function isCanonicalRelativePath(path: string): boolean {
  if (!path || path.startsWith('/') || path.endsWith('/') || path.includes('\\')) return false

  return path.split('/').every((segment) => {
    if (!segment || segment === '.' || segment === '..' || segment.startsWith('.')) return false
    return !Array.from(segment).some((character) => {
      const codePoint = character.codePointAt(0)
      return codePoint !== undefined && (codePoint < 0x20 || codePoint === 0x7f)
    })
  })
}

export function isMarkdownPath(path: string): boolean {
  return isCanonicalRelativePath(path) && path.endsWith('.md')
}

export function directParentPath(path: string): string {
  const separator = path.lastIndexOf('/')
  return separator === -1 ? '' : path.slice(0, separator)
}

export function finalPathSegment(path: string): string {
  const separator = path.lastIndexOf('/')
  return separator === -1 ? path : path.slice(separator + 1)
}

export function encodeCanonicalPath(path: string): string {
  return path.split('/').map(encodeURIComponent).join('/')
}

export function isRfc3339Milliseconds(value: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/.test(value)) return false

  const timestamp = Date.parse(value)
  return !Number.isNaN(timestamp) && new Date(timestamp).toISOString() === value
}
