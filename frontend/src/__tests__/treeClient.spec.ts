import { describe, expect, it, vi } from 'vitest'

import { HttpTreeClient, TreeClientError } from '@/services/treeClient'

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  })
}

describe('HttpTreeClient', () => {
  it('requests the root listing without an empty query parameter', async () => {
    const fetcher = vi.fn(async () => jsonResponse({ path: '', entries: [] }))
    const client = new HttpTreeClient(fetcher)

    await client.listDirectory('')

    expect(fetcher).toHaveBeenCalledWith('/api/tree', {
      headers: { Accept: 'application/json' },
    })
  })

  it('percent-encodes a nested path and parses the tree response', async () => {
    const fetcher = vi.fn(async () =>
      jsonResponse({
        path: '한글 폴더',
        entries: [
          {
            type: 'file',
            name: '메모.md',
            path: '한글 폴더/메모.md',
            size: 12,
            modified_at: '2026-07-22T01:02:03.004Z',
          },
        ],
      }),
    )
    const client = new HttpTreeClient(fetcher)

    const listing = await client.listDirectory('한글 폴더')

    expect(fetcher).toHaveBeenCalledWith('/api/tree?path=%ED%95%9C%EA%B8%80+%ED%8F%B4%EB%8D%94', {
      headers: { Accept: 'application/json' },
    })
    expect(listing.entries[0]).toEqual({
      kind: 'file',
      name: '메모.md',
      path: '한글 폴더/메모.md',
      size: 12,
      modifiedAt: '2026-07-22T01:02:03.004Z',
    })
  })

  it('rejects entries that are not direct children of the listing path', async () => {
    const client = new HttpTreeClient(async () =>
      jsonResponse({
        path: 'projects',
        entries: [
          {
            type: 'directory',
            name: 'nested',
            path: 'projects/agent/nested',
            modified_at: '2026-07-22T01:02:03.004Z',
          },
        ],
      }),
    )

    await expect(client.listDirectory('projects')).rejects.toMatchObject({
      code: 'invalid_response',
    })
  })

  it('maps a backend error envelope without exposing response internals', async () => {
    const client = new HttpTreeClient(async () =>
      jsonResponse(
        {
          error: {
            code: 'directory_not_found',
            message: 'Directory was not found',
            details: { path: 'missing' },
          },
        },
        404,
      ),
    )

    const error = await client.listDirectory('missing').catch((reason: unknown) => reason)

    expect(error).toBeInstanceOf(TreeClientError)
    expect(error).toMatchObject({
      code: 'directory_not_found',
      message: 'Directory was not found',
      status: 404,
    })
    expect(error).not.toHaveProperty('details')
  })
})
