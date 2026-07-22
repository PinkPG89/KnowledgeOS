# B05 Open File Flow Plan

- 상태: Completed
- 시작일: 2026-07-22
- 완료일: 2026-07-22

## Summary

파일 트리에서 선택한 Markdown 파일을 URL deep link로 이동하고, route path를 기준으로 Read API를 호출해 editor pane에 UTF-8 원문을 표시합니다. 직접 deep link로 새로고침해도 tree ancestor를 lazy load하여 선택 상태를 복원합니다.

## 선택 이유

- 현재 문서를 URL로 표현하면 bookmark, 새로고침과 링크 공유가 가능합니다.
- route를 Source of Truth로 두면 tree click과 browser navigation이 동일한 open flow를 사용합니다.
- document state를 tree state와 분리해 파일 content와 navigation projection의 수명 주기를 독립적으로 관리합니다.
- 빠르게 파일을 전환할 때 이전 응답이 최신 문서를 덮지 않도록 request generation과 abort를 함께 사용합니다.

## Route Contract

- workspace root: `/`
- Markdown document: `/files/:path(.*)`
- route parameter는 decoded canonical relative Markdown path로 해석합니다.
- route 생성 시 각 path segment를 router가 처리하며 직접 API URL을 만들 때는 segment별 percent-encoding을 적용합니다.
- invalid path는 backend 요청 전에 document error state로 전환합니다.

## Open Flow

```text
Tree file activation
  -> file selection
  -> named route push
  -> route watcher
     -> Markdown Read API
     -> tree ancestor reveal
     -> selected tree node sync
  -> mobile navigation drawer close
```

- Desktop navigation panel은 열린 상태를 유지합니다.
- Mobile navigation drawer는 route 이동 성공 후 닫습니다.
- browser back 또는 root route 이동 시 active document와 tree selection을 해제합니다.

## Document State

```text
status: idle | loading | loaded | error
activePath: string | null
document: MarkdownDocument | null
error: DocumentLoadError | null
```

- 새 파일을 열 때 이전 document를 즉시 제거해 path와 content 불일치를 방지합니다.
- 최신 request generation만 store state를 변경할 수 있습니다.
- superseded request는 `AbortController`로 취소합니다.
- retry는 현재 active path를 다시 읽습니다.

## API Boundary

- `GET /api/files/{*path}`를 사용합니다.
- response의 path, UTF-8 content, SHA-256, byte size와 UTC millisecond timestamp를 runtime validation합니다.
- `size`는 `TextEncoder`로 계산한 content byte 수와 일치해야 합니다.
- backend error envelope는 absolute filesystem 정보 없이 frontend typed error로 변환합니다.

## Tree Synchronization

- root부터 file parent까지 directory listing을 순서대로 lazy load합니다.
- 각 ancestor directory를 expanded 상태로 전환합니다.
- 최종 file node가 projection에 존재할 때 selected path로 설정합니다.
- document read와 tree reveal은 서로를 차단하지 않고 병렬로 실행합니다.

## UI State

- route 없음: 문서 선택 안내
- loading: 현재 path와 loading status
- loaded: raw Markdown content, hash, byte size와 modified time
- error: public message와 retry action
- B05는 read-only surface이며 editor integration은 B06 이후 진행합니다.

## 비범위

- Markdown editing, preview와 syntax highlighting
- save, autosave, draft recovery와 conflict UI
- recent file persistence
- offline content cache
- route 기반 search와 inspector metadata 확장

## Test Plan

- nested Unicode API path encoding과 document response parsing
- hash, byte size, timestamp와 malformed response 검증
- backend error와 network error mapping
- 빠른 file 전환에서 stale response 차단
- route deep link에서 document load와 ancestor reveal
- tree click의 named route 이동
- mobile drawer close와 desktop panel 유지
- root route에서 document와 selection clear
- loading, loaded, error와 retry rendering

## 운영 시 고려사항

- production은 frontend와 backend를 same-origin reverse proxy로 제공합니다.
- PWA service worker는 Markdown API response를 cache하지 않습니다.
- URL은 filesystem absolute path를 포함하지 않고 Vault relative path만 사용합니다.

## Implementation Result

- `/files/:path(.*)` named route와 tree file activation을 연결했습니다.
- `HttpMarkdownClient`가 segment별 URL encoding, error mapping과 document snapshot runtime validation을 담당합니다.
- Pinia document store가 request abort, stale response 차단, error와 retry state를 관리합니다.
- route watcher가 document read와 tree ancestor reveal을 병렬로 시작합니다.
- deep link에서 root부터 parent directory까지 lazy load하고 최종 file selection을 복원합니다.
- mobile에서는 route 이동 성공 후 navigation drawer를 닫고 desktop panel preference는 유지합니다.
- read-only document pane에 idle, loading, loaded, error와 retry 상태를 구현했습니다.
- Tree와 Markdown client가 같은 canonical path 검증 utility를 사용하도록 중복 규칙을 통합했습니다.

## Validation Result

- 전체 frontend unit/component/integration test 39개가 통과했습니다.
- lint, format, type-check, production build와 PWA 검증이 통과했습니다.
- 실제 Rust backend `39175`와 Vite `39176` 임시 실행에서 deep link, proxied `/api/tree`와 `/api/files/README.md`가 모두 HTTP 200을 반환했습니다.
- 임시 검증 process는 smoke test 후 종료했습니다.

## Next Step

B06 CodeMirror Markdown Editor Spike에서 large document, Korean IME, mobile composition과 toolbar prototype을 검증하고 editor 채택 ADR을 확정합니다.
