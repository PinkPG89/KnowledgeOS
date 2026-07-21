# B03 Tree State Model Plan

- 상태: Completed
- 시작일: 2026-07-22
- 완료일: 2026-07-22

## Summary

Backend의 depth-1 Tree API를 frontend에서 재생성 가능한 client projection으로 관리합니다. Tree node identity는 canonical relative path이며, directory별 child 목록과 lazy-load 상태를 분리합니다.

## 선택 이유

- filesystem path가 Source of Truth이므로 별도 숫자 ID를 만들지 않습니다.
- node data와 directory UI state를 분리하면 동일 node metadata를 중복 저장하지 않습니다.
- `loading`, `loaded`, `expanded`, `error`를 분리해야 loading 중 collapse와 실패 후 retry를 명확하게 처리할 수 있습니다.
- HTTP parsing을 store 밖에 두어 API schema 검증과 UI 상태 전이를 독립적으로 테스트합니다.

## State Contract

```text
nodesByPath[path]             -> file 또는 directory metadata
directoriesByPath[path]       -> children, loadStatus, expanded, error
selectedPath                  -> 현재 선택한 canonical relative path
pendingLoads (non-reactive)   -> directory별 진행 중 Promise
```

- Vault root listing key는 빈 문자열 `""`입니다.
- 빈 문자열은 실제 node path가 아니며 `nodesByPath`에는 저장하지 않습니다.
- directory child는 path 배열로 보관하고 실제 metadata는 `nodesByPath`에서 조회합니다.
- root directory state는 store 생성 시 존재하며 expanded 상태입니다.

## Lazy Loading

- 처음 expand한 directory만 API를 호출합니다.
- 이미 loaded인 directory는 명시적인 refresh가 아니면 다시 요청하지 않습니다.
- 같은 directory의 요청이 진행 중이면 기존 Promise를 반환합니다.
- loading 실패는 기존 children을 보존하고 retry 가능한 error state로 전환합니다.
- collapse는 loaded data를 제거하지 않습니다.

## API Boundary

- `GET /api/tree`와 nested `path` query를 상대 URL로 호출합니다.
- query는 `URLSearchParams`로 percent-encoding합니다.
- 성공 JSON의 type, path, name, size, timestamp와 depth-1 관계를 런타임 검증합니다.
- backend error envelope를 안전한 frontend error로 변환합니다.
- 응답 path가 요청 path와 다르면 schema error로 거부합니다.

## Ordering

- directory를 file보다 먼저 배치합니다.
- 같은 type 안에서는 Unicode code point 기준 name 오름차순으로 정렬합니다.
- 정렬 결과가 같으면 canonical path를 tie-breaker로 사용합니다.

## 비범위

- 실제 tree component 렌더링, keyboard navigation과 touch interaction은 B04 범위입니다.
- file open route와 mobile drawer close는 B05 범위입니다.
- watcher invalidation과 background refresh는 Track D 범위입니다.
- offline tree response cache는 conflict policy가 정해질 때까지 추가하지 않습니다.

## Test Plan

- root와 Unicode nested path URL 생성
- 정상 API response runtime parsing
- malformed response와 backend error envelope 처리
- node upsert와 directory-first stable ordering
- 동일 directory 동시 load 요청 deduplication
- loaded directory 재요청 방지
- expand/collapse와 breakpoint와 무관한 상태 유지
- 실패 상태와 retry 성공
- selected path 상태

## 운영 시 고려사항

- projection은 browser memory에만 존재하므로 reload 시 backend에서 다시 구성합니다.
- 대규모 Vault의 stale node 정리는 refresh/invalidation 정책과 함께 확장합니다.
- frontend가 schema 이상을 감지해도 backend response 원문은 사용자 화면에 노출하지 않습니다.

## Implementation Result

- `HttpTreeClient`가 relative Tree API URL 생성, backend error mapping과 성공 JSON runtime validation을 담당합니다.
- Pinia `tree` store가 node metadata, directory별 lazy state와 selected path를 분리 관리합니다.
- 진행 중 Promise는 reactive state 밖에서 directory path별로 공유해 중복 HTTP 요청을 차단합니다.
- loaded directory는 재요청하지 않고 명시적인 refresh에서만 다시 조회합니다.
- directory-first 및 Unicode code point 정렬과 canonical path tie-breaker를 적용했습니다.
- adapter의 동기 throw를 포함한 실패 상태를 typed result로 변환하고 retry 가능 여부를 제공합니다.
- file node를 directory처럼 toggle하는 잘못된 상태 전이를 거부합니다.

## Validation Result

- Tree client와 store unit test 10개를 추가했습니다.
- 전체 frontend unit test 21개가 통과했습니다.
- lint, format, type-check, production build와 PWA 검증이 통과했습니다.

## Next Step

B04 Lazy Tree UI에서 이 store를 navigation panel에 연결하고 loading, error, retry, keyboard와 touch interaction을 구현합니다.
