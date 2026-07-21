import { access, readFile } from 'node:fs/promises'

const requiredFiles = [
  'dist/index.html',
  'dist/manifest.webmanifest',
  'dist/sw.js',
  'dist/icons/pwa-192x192.png',
  'dist/icons/pwa-512x512.png',
  'dist/icons/maskable-icon-512x512.png',
]

await Promise.all(requiredFiles.map((path) => access(path)))

const manifest = JSON.parse(await readFile('dist/manifest.webmanifest', 'utf8'))
const iconSizes = new Set(manifest.icons.map((icon) => icon.sizes))

if (
  manifest.name !== 'KnowledgeOS' ||
  manifest.display !== 'standalone' ||
  manifest.lang !== 'ko'
) {
  throw new Error('PWA manifest identity or display mode is invalid')
}

if (!iconSizes.has('192x192') || !iconSizes.has('512x512')) {
  throw new Error('PWA manifest must include 192x192 and 512x512 icons')
}

console.log('KnowledgeOS PWA artifacts verified')
