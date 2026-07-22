import type { MarkdownDocument } from '@/models/markdown'
import { encodeCanonicalPath, isMarkdownPath, isRfc3339Milliseconds } from '@/utils/canonicalPath'

type Fetcher = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>

export interface MarkdownClient {
  readFile(path: string, signal?: AbortSignal): Promise<MarkdownDocument>
}

export class MarkdownClientError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly status: number | null,
  ) {
    super(message)
    this.name = 'MarkdownClientError'
  }
}

export class HttpMarkdownClient implements MarkdownClient {
  constructor(
    private readonly fetcher: Fetcher = (...arguments_) => globalThis.fetch(...arguments_),
  ) {}

  async readFile(path: string, signal?: AbortSignal): Promise<MarkdownDocument> {
    if (!isMarkdownPath(path)) {
      throw new MarkdownClientError('invalid_path', '올바른 Markdown 경로가 아닙니다.', 400)
    }

    let response: Response
    try {
      response = await this.fetcher(`/api/files/${encodeCanonicalPath(path)}`, {
        headers: { Accept: 'application/json' },
        signal,
      })
    } catch (error) {
      if (signal?.aborted || (error instanceof DOMException && error.name === 'AbortError')) {
        throw new MarkdownClientError('request_aborted', 'Markdown 요청이 취소되었습니다.', null)
      }
      throw new MarkdownClientError('network_error', 'Markdown API에 연결할 수 없습니다.', null)
    }

    const body = await readJson(response)
    if (!response.ok) throw parseApiError(response.status, body)
    return parseMarkdownDocument(body, path)
  }
}

export const markdownClient: MarkdownClient = new HttpMarkdownClient()

async function readJson(response: Response): Promise<unknown> {
  try {
    return await response.json()
  } catch {
    throw new MarkdownClientError(
      'invalid_response',
      'Markdown API가 올바른 JSON을 반환하지 않았습니다.',
      response.status,
    )
  }
}

function parseApiError(status: number, body: unknown): MarkdownClientError {
  if (!isRecord(body) || !isRecord(body.error)) {
    return new MarkdownClientError('http_error', 'Markdown API 요청이 실패했습니다.', status)
  }

  const code = typeof body.error.code === 'string' ? body.error.code : 'http_error'
  const message =
    typeof body.error.message === 'string'
      ? body.error.message
      : 'Markdown API 요청이 실패했습니다.'
  return new MarkdownClientError(code, message, status)
}

function parseMarkdownDocument(body: unknown, requestedPath: string): MarkdownDocument {
  if (
    !isRecord(body) ||
    body.path !== requestedPath ||
    typeof body.content !== 'string' ||
    typeof body.hash !== 'string' ||
    typeof body.size !== 'number' ||
    typeof body.modified_at !== 'string'
  ) {
    throw invalidResponse()
  }

  if (
    !/^sha256:[0-9a-f]{64}$/.test(body.hash) ||
    !Number.isSafeInteger(body.size) ||
    body.size < 0 ||
    new TextEncoder().encode(body.content).byteLength !== body.size ||
    !isRfc3339Milliseconds(body.modified_at)
  ) {
    throw invalidResponse()
  }

  return {
    path: requestedPath,
    content: body.content,
    hash: body.hash,
    size: body.size,
    modifiedAt: body.modified_at,
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function invalidResponse(): MarkdownClientError {
  return new MarkdownClientError(
    'invalid_response',
    'Markdown API 응답이 예상한 schema와 일치하지 않습니다.',
    null,
  )
}
