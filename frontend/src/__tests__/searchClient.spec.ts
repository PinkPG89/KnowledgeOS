import { describe, expect, it, vi } from 'vitest'

import { HttpSearchClient } from '@/services/searchClient'

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  })
}

describe('HttpSearchClient', () => {
  it('encodes a trimmed literal query and validates results', async () => {
    const fetcher = vi.fn().mockResolvedValue(
      jsonResponse({
        query: '설계 문서',
        limit: 20,
        results: [
          {
            path: '프로젝트/설계.md',
            title: '설계',
            snippet: '핵심 설계 문서',
            score: 1.25,
          },
        ],
      }),
    )
    const client = new HttpSearchClient(fetcher)

    await expect(client.search('  설계 문서  ')).resolves.toEqual({
      query: '설계 문서',
      limit: 20,
      results: [
        {
          path: '프로젝트/설계.md',
          title: '설계',
          snippet: '핵심 설계 문서',
          score: 1.25,
        },
      ],
    })
    expect(fetcher).toHaveBeenCalledWith(
      '/api/search?q=%EC%84%A4%EA%B3%84+%EB%AC%B8%EC%84%9C',
      expect.objectContaining({ headers: { Accept: 'application/json' } }),
    )
  })

  it('preserves a structured API error', async () => {
    const client = new HttpSearchClient(
      vi
        .fn()
        .mockResolvedValue(
          jsonResponse(
            { error: { code: 'search_unavailable', message: '검색 색인을 사용할 수 없습니다.' } },
            503,
          ),
        ),
    )

    await expect(client.search('query')).rejects.toMatchObject({
      code: 'search_unavailable',
      message: '검색 색인을 사용할 수 없습니다.',
      status: 503,
    })
  })

  it.each([
    { query: 'other', limit: 20, results: [] },
    { query: 'query', limit: 0, results: [] },
    {
      query: 'query',
      limit: 20,
      results: [{ path: '../escape.md', title: 'Bad', snippet: '', score: 1 }],
    },
    {
      query: 'query',
      limit: 20,
      results: [{ path: 'note.md', title: 'Bad', snippet: '', score: Number.NaN }],
    },
  ])('rejects an invalid response %#', async (body) => {
    const client = new HttpSearchClient(vi.fn().mockResolvedValue(jsonResponse(body)))

    await expect(client.search('query')).rejects.toEqual(
      expect.objectContaining({ code: 'invalid_response' }),
    )
  })

  it('normalizes network and abort failures', async () => {
    const networkClient = new HttpSearchClient(vi.fn().mockRejectedValue(new TypeError('failed')))
    await expect(networkClient.search('query')).rejects.toMatchObject({ code: 'network_error' })

    const abortedClient = new HttpSearchClient(
      vi.fn().mockRejectedValue(new DOMException('aborted', 'AbortError')),
    )
    await expect(abortedClient.search('query')).rejects.toMatchObject({
      code: 'request_aborted',
    })
  })
})
