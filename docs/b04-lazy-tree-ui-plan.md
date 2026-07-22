# B04 Lazy Tree UI Plan

- 상태: Completed
- 시작일: 2026-07-22
- 완료일: 2026-07-22

## Summary

B03 Tree store를 responsive shell의 navigation panel에 연결합니다. Root load, directory lazy expand, loading, empty, error와 retry 상태를 제공하고 keyboard와 touch로 동일한 기능에 접근할 수 있게 합니다.

## 선택 이유

- 재귀 component마다 focus를 관리하는 대신 visible node를 평탄화하면 keyboard의 이전·다음 항목 계산이 결정적입니다.
- Store가 tree hierarchy를 소유하므로 component는 현재 expanded 상태에서 보이는 projection만 계산합니다.
- File open route는 B05 범위로 분리하고 B04에서는 node selection까지만 수행합니다.

## Interaction Contract

- `ArrowDown`, `ArrowUp`: 다음·이전 visible node로 이동
- `Home`, `End`: 첫 번째·마지막 visible node로 이동
- `ArrowRight`: 닫힌 directory를 펼치고, 열린 directory에서는 첫 child로 이동
- `ArrowLeft`: 열린 directory를 닫고, 닫힌 directory 또는 file에서는 parent로 이동
- `Enter`, `Space`: directory toggle 또는 file selection
- Pointer/touch: directory toggle 또는 file selection

## Accessibility

- container는 `role="tree"`, node는 `role="treeitem"`을 사용합니다.
- visible node 한 개만 `tabindex="0"`인 roving tabindex를 적용합니다.
- node에 `aria-level`, `aria-posinset`, `aria-setsize`를 제공합니다.
- directory는 `aria-expanded`, selected file은 `aria-selected`를 제공합니다.
- loading은 `aria-busy`와 status text, error는 alert와 명시적 retry button을 사용합니다.
- 모든 row와 action은 최소 `44px` touch target을 유지합니다.

## State Rendering

- Root `idle/loading`: 초기 loading 상태
- Root `error`: 전체 error와 retry
- Root `loaded` + empty: 빈 Vault 안내
- Expanded directory `loading`: child loading row
- Expanded directory `error`: inline error와 retry
- Loaded directory `empty`: 빈 폴더 안내

## Development Runtime

- Vite development server의 `/api` 요청을 기본 backend `http://127.0.0.1:3000`으로 proxy합니다.
- `VITE_BACKEND_ORIGIN`으로 development backend origin을 변경할 수 있습니다.
- Production reverse proxy와 container 통합은 frontend 배포 단계에서 별도 확정합니다.

## 비범위

- 파일 content fetch와 editor route 이동
- mobile에서 file 선택 후 navigation drawer close
- rename, move, delete와 context menu
- drag and drop
- watcher 기반 자동 refresh

## Test Plan

- mount 시 root load와 loading 상태
- root error, retry와 empty 상태
- directory expand 시 최초 1회 lazy load
- loading/error/empty child 상태
- directory/file accessible attribute
- Arrow Up/Down/Home/End focus 이동
- Arrow Right/Left expand, child와 parent 이동
- Enter/Space toggle과 file selection
- 최소 44px touch target CSS contract

## Implementation Result

- B03 store의 visible node를 평탄화하는 `FileTreePanel`을 navigation panel에 연결했습니다.
- root와 nested directory의 loading, error, retry, empty 상태를 각각 렌더링합니다.
- roving tabindex와 Arrow, Home, End, Enter, Space keyboard interaction을 구현했습니다.
- `role="tree"`, `treeitem` metadata, expanded, selected와 busy 상태를 제공합니다.
- row, refresh와 retry action에 최소 44px touch target을 적용했습니다.
- Vite development server에 configurable backend proxy를 추가했습니다.

## Validation Result

- FileTreePanel component test 7개를 추가했습니다.
- 전체 frontend unit test 28개가 통과했습니다.
- lint, format과 type-check가 통과했습니다.
- 실제 Rust backend `39173`과 Vite `39174` 임시 실행에서 frontend와 proxied `/api/tree`가 모두 HTTP 200을 반환했습니다.
- 임시 검증 process는 smoke test 후 종료했습니다.

## Next Step

B05 Open File Flow에서 selected file path를 URL route와 동기화하고 Markdown Read API 결과를 editor pane에 표시합니다. Mobile에서는 파일 선택 성공 후 navigation drawer를 닫습니다.
