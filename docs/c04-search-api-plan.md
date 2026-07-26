# C04 Search API

- 상태: Completed
- 완료일: 2026-07-26

## Summary

SQLite FTS5 projection을 사용하는 `GET /api/search` public contract를 구현했습니다. 검색어,
canonical path prefix와 결과 제한을 검증하고 title, body와 tag match를 가중치 기반으로
정렬해 plain snippet과 relevance score를 반환합니다.

사용자 입력은 FTS5 query language로 직접 실행하지 않습니다. 공백으로 분리한 각 token을
quoted literal phrase로 변환하고 `AND`로 결합해 `OR`, quote, wildcard, parenthesis와
minus 입력이 query operator나 syntax error로 동작하지 않게 합니다.

## API Contract

```http
GET /api/search?q=architecture&path_prefix=projects&limit=20
```

- `q`: trim 이후 1–512 UTF-8 bytes인 필수 검색어
- `path_prefix`: 선택 canonical path이며 해당 path와 `/` 경계의 descendant만 허용
- `limit`: 선택 값, 기본 20, 허용 범위 1–100

```json
{
  "query": "architecture",
  "path_prefix": "projects",
  "limit": 20,
  "results": [
    {
      "path": "projects/knowledgeos/architecture.md",
      "title": "KnowledgeOS Architecture",
      "snippet": "Filesystem-first architecture …",
      "score": 0.0000021
    }
  ]
}
```

Score는 `-bm25`이며 값이 클수록 관련성이 높습니다. 절대 범위나 release 간 동일 값은
public contract가 아니며 결과 순서 비교에만 사용합니다. 동일 rank는 canonical path
오름차순으로 안정적으로 정렬합니다.

## 선택 이유

- FTS query syntax를 public API로 노출하지 않아 malformed query와 operator injection을
  차단합니다.
- Title 5, tag 2, body 1의 BM25 weight를 적용해 page open UX에서 제목 match를 우선합니다.
- Path prefix는 `LIKE` wildcard가 아니라 exact prefix와 `/` boundary를 비교합니다.
  underscore를 포함한 canonical path가 wildcard로 해석되지 않게 하기 위해서입니다.
- Snippet에 HTML highlight marker를 넣지 않습니다. C05가 Vue text binding으로 안전하게
  렌더링하고 highlight UX는 별도 구조로 확장할 수 있습니다.
- Blocking SQLite query는 Tokio blocking pool에서 실행해 async HTTP worker를 점유하지
  않습니다.

## 장점

- FTS5 operator를 알 필요 없는 예측 가능한 literal search입니다.
- Unicode61 tokenizer를 그대로 사용해 별도 search daemon 없이 한글과 영문을 검색합니다.
- Prefix와 limit이 SQL parameter로 전달되어 query와 path가 SQL에 직접 결합되지 않습니다.
- Index degraded 또는 query failure를 파일 API와 분리된 `503 search_unavailable`로
  반환합니다.

## 단점

- 공백 token은 모두 `AND`로 결합되어 긴 자연어 query의 recall이 낮을 수 있습니다.
- 한글 형태소 분석과 초성 검색은 지원하지 않습니다.
- Prefix wildcard, boolean operator, phrase query를 power-user 기능으로 제공하지 않습니다.
- Snippet match offset을 반환하지 않아 C05에서 정확한 client-side highlight를 제공하지
  않습니다.

## 대안

- FTS syntax passthrough: 표현력은 높지만 escaping과 오류 contract가 client에 노출되어
  제외했습니다.
- OR 기반 token 검색: recall은 높지만 일반 단어가 많은 문서가 결과를 지배할 수 있어 MVP는
  AND를 선택했습니다.
- 정규화된 0–1 score: UI에는 단순하지만 corpus 변화에 따라 의미가 불안정하므로 raw
  relative BM25 score를 사용합니다.
- Meilisearch 또는 Typesense: typo tolerance는 좋지만 별도 service와 index lifecycle
  운영 비용이 C04 범위를 초과합니다.

## 자동화 검증

- Title, body와 frontmatter tag 검색
- Title weight 우선순위와 positive relevance score
- Plain snippet contract
- Canonical nested path prefix와 exact subtree boundary
- Limit 적용과 동일 score stable path ordering
- `OR`, quote, wildcard, minus와 parenthesis literal escaping
- Punctuation-only query의 syntax safety
- Missing/blank/oversized query와 invalid limit JSON error
- Invalid canonical prefix 거부
- Degraded index의 `503 search_unavailable`

## 운영 시 고려사항

- C05는 score 숫자를 사용자에게 직접 표시하지 않고 정렬에만 의존해야 합니다.
- Query latency와 result count를 구조화된 metric으로 추가하는 작업은 운영 관측 단계에서
  진행합니다.
- Search API 인증은 전체 KnowledgeOS remote access policy와 함께 적용해야 합니다.
- Parser 또는 ranking weight 변경은 대표 Vault query set으로 회귀 평가해야 합니다.

## 다음 단계

C05에서 mobile search panel, loading/empty/error/result 상태, keyboard navigation과 search
result open flow를 구현합니다.
