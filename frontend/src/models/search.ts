export interface SearchResult {
  path: string
  title: string
  snippet: string
  score: number
}

export interface SearchResponse {
  query: string
  pathPrefix?: string
  limit: number
  results: SearchResult[]
}
