import { describe, expect, it, vi } from 'vitest'

import { HttpMarkdownClient, MarkdownClientError } from '@/services/markdownClient'

const hash = `sha256:${'a'.repeat(64)}`
const modifiedAt = '2026-07-22T01:02:03.004Z'

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  })
}

describe('HttpMarkdownClient', () => {
  it('encodes each path segment and validates the document snapshot', async () => {
    const content = '# 한글\n'
    const fetcher = vi.fn(async () =>
      jsonResponse({
        path: '프로젝트/첫 메모.md',
        content,
        hash,
        size: new TextEncoder().encode(content).byteLength,
        modified_at: modifiedAt,
      }),
    )
    const client = new HttpMarkdownClient(fetcher)

    const document = await client.readFile('프로젝트/첫 메모.md')

    expect(fetcher).toHaveBeenCalledWith(
      '/api/files/%ED%94%84%EB%A1%9C%EC%A0%9D%ED%8A%B8/%EC%B2%AB%20%EB%A9%94%EB%AA%A8.md',
      expect.objectContaining({ headers: { Accept: 'application/json' } }),
    )
    expect(document).toEqual({
      path: '프로젝트/첫 메모.md',
      content,
      hash,
      size: 9,
      modifiedAt,
    })
  })

  it('rejects malformed hashes and byte sizes', async () => {
    const client = new HttpMarkdownClient(async () =>
      jsonResponse({
        path: 'note.md',
        content: '한글',
        hash: 'sha256:invalid',
        size: 2,
        modified_at: modifiedAt,
      }),
    )

    await expect(client.readFile('note.md')).rejects.toMatchObject({ code: 'invalid_response' })
  })

  it('rejects an invalid path before calling fetch', async () => {
    const fetcher = vi.fn(async () => jsonResponse({}))
    const client = new HttpMarkdownClient(fetcher)

    await expect(client.readFile('../note.md')).rejects.toMatchObject({ code: 'invalid_path' })
    expect(fetcher).not.toHaveBeenCalled()
  })

  it('maps the backend error envelope', async () => {
    const client = new HttpMarkdownClient(async () =>
      jsonResponse(
        { error: { code: 'file_not_found', message: 'Markdown file was not found' } },
        404,
      ),
    )

    const error = await client.readFile('missing.md').catch((reason: unknown) => reason)

    expect(error).toBeInstanceOf(MarkdownClientError)
    expect(error).toMatchObject({
      code: 'file_not_found',
      message: 'Markdown file was not found',
      status: 404,
    })
  })

  it('updates a document with its base hash', async () => {
    const content = '# Updated\n'
    const fetcher = vi.fn(async () =>
      jsonResponse({
        path: 'notes/한글.md',
        content,
        hash: `sha256:${'b'.repeat(64)}`,
        size: content.length,
        modified_at: modifiedAt,
      }),
    )
    const client = new HttpMarkdownClient(fetcher)

    const updated = await client.updateFile('notes/한글.md', content, hash)

    expect(fetcher).toHaveBeenCalledWith(
      '/api/files/notes/%ED%95%9C%EA%B8%80.md',
      expect.objectContaining({
        method: 'PUT',
        headers: {
          Accept: 'application/json',
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ content, base_hash: hash }),
      }),
    )
    expect(updated.content).toBe(content)
  })

  it('preserves the current hash from a write conflict', async () => {
    const currentHash = `sha256:${'c'.repeat(64)}`
    const client = new HttpMarkdownClient(async () =>
      jsonResponse(
        {
          error: {
            code: 'write_conflict',
            message: 'Markdown file changed',
            details: { path: 'note.md', current_hash: currentHash },
          },
        },
        409,
      ),
    )

    const error = await client.updateFile('note.md', 'changed', hash).catch((reason) => reason)

    expect(error).toMatchObject({
      code: 'write_conflict',
      status: 409,
      currentHash,
    })
  })
})
