# Incremental Implementation Plan

- 상태: Active
- 최종 갱신: 2026-07-19

## 실행 원칙

각 작업 단위는 하나의 명확한 동작만 추가하고, 독립적인 test와 완료 기준을 가진다. Upstream 코드를 먼저 복사하지 않는다. 해당 단위의 contract test를 작성한 뒤 필요한 동작만 KnowledgeOS 언어와 구조로 구현한다.

크기 기준:

- 한 단위는 반나절에서 이틀 안에 완료한다.
- backend와 frontend 변경을 가능하면 분리한다.
- 모든 단위는 rollback 가능한 commit 하나를 목표로 한다.
- 선행 단위가 완료되기 전에는 후속 기능을 섞지 않는다.

## Track A: Filesystem Core

### A01 Backend project skeleton

- 상태: Rust 전환 완료 (2026-07-12)
- 범위: Axum app, typed settings, structured logging, health endpoint
- 참고: Flatnotes의 storage/API 분리 개념
- 산출물: 실행 가능한 backend와 test harness
- 완료 기준: `/api/health` contract test 통과
- 비범위: file CRUD, auth, index

구현 결과:

- Rust 2024 edition, Axum, Tokio, Serde를 적용했다.
- 설정은 `KNOWLEDGEOS_` 환경 변수 prefix를 사용한다.
- `tracing` event를 JSON으로 표준 출력에 기록한다.
- `unsafe`를 금지하고 formatting, test, strict Clippy 검사를 통과했다.
- public API contract test는 실제 TCP port 없이 동일한 Router를 호출한다.

### A02 Canonical path policy

- 상태: 완료 (2026-07-12)
- 범위: relative path parsing과 canonicalization
- 계약: [Path Policy](path-policy.md)
- 허용: nested directory, Unicode, space, underscore system directory, `.md`
- 거부: absolute path, `.`, `..`, NUL/control, `%`, hidden segment, Windows separator, non-Markdown file
- 완료 기준: traversal, encoded traversal, Windows separator test 통과
- 비범위: filesystem read/write

구현 결과:

- OS path와 분리된 `CanonicalPath` value object를 추가했다.
- directory path와 Markdown file path를 `CanonicalPath`, `MarkdownPath` 타입으로 분리했다.
- 위험한 입력을 자동 normalization하지 않고 typed `PathError`로 거부한다.
- Unicode, underscore system directory, traversal, hidden segment, length, extension 경계 테스트를 통과했다.

### A03 Root containment and symlink policy

- 상태: 완료 (2026-07-12)
- 범위: resolved path가 `knowledge/` 내부인지 확인하고 symlink를 거부
- 계약: [Vault Policy](vault-policy.md)
- 완료 기준: root 밖 symlink와 nested symlink test 통과
- 비범위: file content validation

구현 결과:

- 설정 경로와 canonical root를 보관하는 `VaultRoot`를 추가했다.
- root symlink는 startup에서 한 번 해석하고 descendant symlink는 모두 거부한다.
- 존재 대상과 create parent 검증 인터페이스를 분리했다.
- network bind 전에 단일 활성 Vault를 검증하는 fail-fast startup을 적용했다.

### A04 Read file

- 상태: 완료 (2026-07-12)
- 범위: UTF-8 Markdown read, hash, size, modified time
- 참고: Flatnotes의 read contract
- 완료 기준: 정상, 없음, invalid UTF-8, oversized file test 통과

구현 결과:

- `GET /api/files/{*path}`와 공통 JSON 오류 계약을 추가했다.
- 최대 5 MiB UTF-8 원문, SHA-256 hash, byte size, UTC RFC3339 millisecond를 반환한다.
- blocking filesystem read는 Tokio blocking pool에서 실행한다.
- 읽기 중 metadata 변경을 감지하면 한 번 재시도하고 반복 변경 시 conflict를 반환한다.

### A05 Create file

- 상태: 완료 (2026-07-18)
- 범위: parent 확인, exclusive create, UTF-8, size limit
- 참고: Flatnotes의 overwrite 방지
- 완료 기준: create와 duplicate conflict test 통과

구현 결과:

- `POST /api/files`와 `201 Created` 문서 응답 계약을 추가했다.
- 기존 parent만 허용하고 `create_new(true)`로 동시 요청에서도 덮어쓰기를 방지한다.
- UTF-8 byte 제한, write·flush·file `fsync`, 실패한 불완전 파일 정리를 적용했다.
- malformed JSON, parent 오류, 중복, symlink, 크기 제한을 공통 JSON 오류로 반환한다.

### A06 Atomic update with conflict detection

- 상태: 완료 (2026-07-19)
- 범위: `base_hash` 비교, temp write, fsync, atomic replace
- 완료 기준: stale hash는 409, 정상 write 후 hash 변경, 실패 시 원본 보존

구현 결과:

- `PUT /api/files/{*path}`와 전체 문서 응답을 추가했다.
- backend write lock과 교체 직전 hash 재검증으로 동일 base hash 동시 저장을 차단한다.
- 같은 directory의 hidden temp 파일을 동기화한 뒤 atomic rename한다.
- stale hash, temp write 실패, oversized content에서는 원본을 보존한다.
- 외부 local process CAS race와 parent directory `fsync`는 후속 보안·운영 강화 범위로 유지한다.

### A07 Create directory

- 범위: nested directory 생성과 duplicate handling
- 완료 기준: parent 없음, 기존 file 충돌, 정상 생성 test 통과

### A08 Move file or directory

- 범위: rename, 목적지 충돌, subtree self-move 방지
- 참고: Flatnotes rename collision, Many Notes tree move UX
- 완료 기준: file/folder move와 모든 충돌 test 통과

### A09 Trash and restore

- 범위: `_trash/` 이동, collision-free trash name, restore metadata
- 완료 기준: delete 후 원본 부재, trash 존재, restore 성공
- 비범위: retention cleanup

### A10 Lazy tree endpoint

- 상태: 완료 (2026-07-19)
- 범위: 특정 path의 직계 children 조회, directory-first stable sort
- 참고: Many Notes lazy tree
- 계약: [A10 Lazy Tree API Plan](a10-lazy-tree-plan.md)
- 완료 기준: depth 1 응답, hidden/symlink 제외, stable ordering test 통과

구현 결과:

- `GET /api/tree`와 nested `path` query를 depth 1 lazy loading 계약으로 구현했다.
- directory와 lowercase Markdown file만 노출하고 hidden, symlink, special, non-Markdown entry는 제외한다.
- directory-first와 Rust 문자열 오름차순 정렬을 적용했다.
- blocking scan 격리, typed error mapping과 scan race 테스트를 추가했다.

## Track B: Frontend Foundation

### B01 PWA project skeleton

- 상태: 완료 (2026-07-21)
- 범위: Vue 3, TypeScript, router, Pinia, build/test/lint
- 완료 기준: installable shell과 offline fallback 표시
- 비범위: offline editing

구현 결과:

- Vue 3, TypeScript, Vite, Vue Router와 Pinia application foundation을 구성했다.
- app shell precache, install manifest, PWA icon과 사용자 확인 update prompt를 추가했다.
- online/offline 상태를 텍스트로 노출하고 API response는 cache하지 않는다.
- lint, format, type check, component/store test와 production PWA build를 검증했다.

### B02 Responsive app shell

- 상태: 완료 (2026-07-22)
- 범위: desktop 3영역, mobile drawer와 backdrop
- 참고: Many Notes `Show.vue`, `layout.ts`
- 완료 기준: breakpoint 전환과 panel preference component test 통과

구현 결과:

- `64rem` 기준 desktop 3영역과 mobile overlay drawer를 구현했다.
- desktop panel preference만 local storage에 보존하고 mobile drawer 상태는 저장하지 않는다.
- breakpoint 전환 시 일시적인 mobile overlay를 정리한다.
- panel 접근성 속성, 44px touch target과 reduced motion 처리를 추가했다.
- layout store와 workspace component 상태 전환을 unit test로 검증했다.

### B03 Tree state model

- 상태: 완료 (2026-07-22)
- 범위: `nodesByPath`, children, loading, loaded, expanded, selected
- 참고: Many Notes `vaultTree.ts`
- 완료 기준: 중복 load 방지, sort, collapse/expand unit test 통과

구현 결과:

- canonical relative path 기반 node projection과 directory별 lazy state를 구현했다.
- HTTP parsing과 Pinia 상태 전이를 분리하고 API JSON을 runtime validation한다.
- 진행 중 Promise 공유와 loaded 상태 확인으로 동일 directory 중복 요청을 방지한다.
- directory-first Unicode ordering, collapse/expand, failure/retry와 selected node를 unit test로 검증했다.

### B04 Lazy tree UI

- 범위: root load, folder expand, loading/error/retry
- 완료 기준: keyboard 탐색과 mobile touch target 검증

### B05 Open file flow

- 범위: path route, file fetch, selected tree sync, mobile drawer close
- 참고: Many Notes file open behavior
- 완료 기준: deep link 새로고침과 mobile drawer test 통과

### B06 CodeMirror Markdown editor spike

- 범위: large document, Korean IME, iOS composition, toolbar prototype
- 완료 기준: 실기기 검증 기록과 editor 채택 ADR
- 비범위: production save integration

### B07 Editor save state machine

- 범위: clean, dirty, saving, conflict, error
- 완료 기준: double save 방지, retry, conflict component test 통과

### B08 Browser draft recovery

- 범위: path+base hash draft, resume/discard UI
- 참고: Flatnotes `Note.vue`
- 완료 기준: reload recovery와 remote-change conflict test 통과

## Track C: Search Projection

### C01 Index schema and lifecycle

- 범위: SQLite FTS5 schema version, create, destroy, rebuild
- 참고: Flatnotes index lifecycle
- 완료 기준: DB 삭제 후 full rebuild test 통과

### C02 Markdown projection parser

- 범위: title, body, frontmatter tags, links, hash 추출
- 완료 기준: malformed frontmatter가 file CRUD를 막지 않음

### C03 Incremental index sync

- 범위: create/update/delete/move projection 갱신
- 완료 기준: 원본과 index drift reconciliation test 통과

### C04 Search API

- 범위: query, path prefix, limit, snippet, score
- 완료 기준: API contract와 escaping test 통과

### C05 Search UI

- 범위: mobile search panel, result keyboard navigation, open result
- 참고: Flatnotes search interaction
- 완료 기준: empty/loading/error/result 상태 test 통과

## Track D: External Change Safety

### D01 Filesystem watcher

- 범위: external create/update/delete/move event 정규화
- 완료 기준: burst debounce와 atomic replace event test 통과

### D02 UI invalidation channel

- 범위: SSE 기반 path invalidation
- 완료 기준: 열린 파일 외부 변경 알림과 tree refresh test 통과

### D03 Conflict review flow

- 범위: local draft, server content, base content 비교 UI
- 완료 기준: overwrite 없이 사용자 선택으로만 해결

## 우선 실행 순서

다음 순서는 세로 slice보다 기반 위험을 먼저 제거한다.

1. A01 Backend project skeleton — 완료
2. A02 Canonical path policy — 완료
3. A03 Root containment and symlink policy — 완료
4. A04 Read file — 완료
5. A05 Create file — 완료
6. A06 Atomic update with conflict detection — 완료
7. A10 Lazy tree endpoint — 완료
8. B01 PWA project skeleton — 완료
9. B02 Responsive app shell — 완료
10. B03 Tree state model — 완료
11. B04 Lazy tree UI — 다음 단계
12. B05 Open file flow

첫 milestone은 `knowledge/`를 안전하게 탐색하고 하나의 Markdown 파일을 읽는 vertical slice다. 쓰기 기능은 path policy와 read contract가 검증된 뒤 추가한다.

## 병렬 진행 원칙

다음 track은 소유 파일과 contract가 분리된 이후 병렬 진행할 수 있다.

- Track A Filesystem Core: `backend/src/domain`, `backend/src/application`, `backend/src/infrastructure`
- Track B Frontend Foundation: `frontend/`
- Track C Search Projection: A04 read contract 확정 이후 시작
- Track D External Change Safety: A06 atomic update contract 확정 이후 시작
- 운영 구성: backend 실행 contract 확정 후 `docker/`에서 병렬 진행

A02와 A03은 같은 path invariant를 다루므로 순차 진행한다. A04와 frontend B01은 병렬 진행할 수 있으며, C와 D는 filesystem contract가 안정되기 전에 시작하지 않는다.

## 첫 단위 결정

A01 시작 전에 아래 결정을 ADR로 고정한다.

- Rust edition과 최소 compiler version
- Axum과 Tokio runtime
- Cargo package layout
- environment configuration precedence
- rustfmt, test, Clippy 품질 gate

결정안은 Rust 2024 edition, Axum, Tokio, Serde, tracing이다. 장점은 낮은 runtime overhead와 compile-time 안전성이다. 단점은 학습 비용과 compile 시간이 증가한다. 대안은 Go이며 개발 속도가 실제 병목으로 확인될 때 ADR을 다시 검토한다.
