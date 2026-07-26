import type { SearchResponse, SearchResult } from '@/models/search'
import { isCanonicalRelativePath, isMarkdownPath } from '@/utils/canonicalPath'

type Fetcher = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>

export interface SearchClient {
  search(query: string, signal?: AbortSignal): Promise<SearchResponse>
}

export class SearchClientError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly status: number | null,
  ) {
    super(message)
    this.name = 'SearchClientError'
  }
}

export class HttpSearchClient implements SearchClient {
  constructor(
    private readonly fetcher: Fetcher = (...arguments_) => globalThis.fetch(...arguments_),
  ) {}

  async search(query: string, signal?: AbortSignal): Promise<SearchResponse> {
    const normalizedQuery = query.trim()
    const parameters = new URLSearchParams({ q: normalizedQuery })

    let response: Response
    try {
      response = await this.fetcher(`/api/search?${parameters.toString()}`, {
        headers: { Accept: 'application/json' },
        signal,
      })
    } catch (error) {
      if (signal?.aborted || (error instanceof DOMException && error.name === 'AbortError')) {
        throw new SearchClientError('request_aborted', '검색 요청이 취소되었습니다.', null)
      }
      throw new SearchClientError('network_error', 'Search API에 연결할 수 없습니다.', null)
    }

    const body = await readJson(response)
    if (!response.ok) throw parseApiError(response.status, body)
    return parseSearchResponse(body, normalizedQuery)
  }
}

export const searchClient: SearchClient = new HttpSearchClient()

async function readJson(response: Response): Promise<unknown> {
  try {
    return await response.json()
  } catch {
    throw new SearchClientError(
      'invalid_response',
      'Search API가 올바른 JSON을 반환하지 않았습니다.',
      response.status,
    )
  }
}

function parseApiError(status: number, body: unknown): SearchClientError {
  if (!isRecord(body) || !isRecord(body.error)) {
    return new SearchClientError('http_error', '검색 요청이 실패했습니다.', status)
  }

  const code = typeof body.error.code === 'string' ? body.error.code : 'http_error'
  const message =
    typeof body.error.message === 'string' ? body.error.message : '검색 요청이 실패했습니다.'
  return new SearchClientError(code, message, status)
}

function parseSearchResponse(body: unknown, requestedQuery: string): SearchResponse {
  if (
    !isRecord(body) ||
    body.query !== requestedQuery ||
    !Number.isSafeInteger(body.limit) ||
    (body.limit as number) < 1 ||
    (body.limit as number) > 100 ||
    !Array.isArray(body.results)
  ) {
    throw invalidResponse()
  }

  if (
    body.path_prefix !== undefined &&
    (typeof body.path_prefix !== 'string' || !isCanonicalRelativePath(body.path_prefix))
  ) {
    throw invalidResponse()
  }

  return {
    query: requestedQuery,
    ...(typeof body.path_prefix === 'string' ? { pathPrefix: body.path_prefix } : {}),
    limit: body.limit as number,
    results: body.results.map(parseSearchResult),
  }
}

function parseSearchResult(value: unknown): SearchResult {
  if (
    !isRecord(value) ||
    typeof value.path !== 'string' ||
    !isMarkdownPath(value.path) ||
    typeof value.title !== 'string' ||
    typeof value.snippet !== 'string' ||
    typeof value.score !== 'number' ||
    !Number.isFinite(value.score)
  ) {
    throw invalidResponse()
  }

  return {
    path: value.path,
    title: value.title,
    snippet: value.snippet,
    score: value.score,
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function invalidResponse(): SearchClientError {
  return new SearchClientError(
    'invalid_response',
    'Search API 응답이 예상한 schema와 일치하지 않습니다.',
    null,
  )
}
