# Stabilization Refactor Plan

## 상태

- 진행 상태: Completed
- 기준 commit: `605928a`
- 목적: A07 Create directory, A08 Move/Rename, A09 Trash/Restore를 받기 전 frontend tree
  경계 안정화
- 비범위: 신규 사용자 기능, backend contract 변경, UI framework 도입, Pinia 전면 재설계

## 문제점

### FileTreePanel 책임 집중

`FileTreePanel.vue`가 다음 책임을 동시에 가지고 있습니다.

- expanded directory에서 visible row projection 생성
- focused/selected node에서 create parent 결정
- root/nested load와 retry orchestration
- keyboard focus 이동과 file/directory activation
- toolbar, tree row, directory state와 inline create markup/style

이 상태에서 directory create, move와 context action을 추가하면 component가 filesystem command
규칙까지 소유하게 됩니다.

### Document navigation 후처리 분산

생성 성공 후 parent refresh, route push, mobile drawer close가 `WorkspaceView`의 개별 함수에
결합되어 있습니다. 이후 move/rename에서는 같은 route/tree reconciliation이 더 복잡해집니다.

### 표시와 command 확장 지점 혼재

tree toolbar와 row markup이 orchestration code와 같은 파일에 있어 `⋮` action menu, long press와
keyboard menu를 추가할 때 회귀 범위가 넓습니다.

## 리팩터링 원칙

- behavior-preserving change만 수행합니다.
- pure function을 먼저 추출하고 characterization test를 작성합니다.
- component는 domain/API 규칙을 새로 만들지 않습니다.
- A07–A09 구현을 미리 추상화하지 않습니다.
- public API response, route, Pinia state와 DOM accessibility contract를 유지합니다.

## 단계

### R01 Tree view projection과 action context

- visible tree item projection을 pure function으로 이동
- cycle protection과 expanded subtree 규칙 유지
- focused node, selected node, root fallback으로 create parent를 결정하는 pure function 추출
- projection/action context unit test 추가

### R02 Workspace document navigation workflow

- file route push와 mobile drawer close를 composable로 이동
- document create 후 parent refresh와 open 순서를 하나의 workflow로 이동
- 기존 route guard와 route watcher는 `WorkspaceView`에 유지

### R03 FileTree 표시 component

- toolbar를 `FileTreeToolbar`로 분리
- ARIA treeitem row를 `TreeItemRow`로 분리
- nested loading/error/empty row를 `TreeDirectoryStateRow`로 분리
- `FileTreePanel`은 store/client orchestration과 keyboard policy만 유지

## 선택 이유

- A07–A09가 사용할 tree action 위치와 표시 경계를 먼저 확보할 수 있습니다.
- pure projection은 Vue lifecycle과 분리되어 edge case를 빠르게 검증할 수 있습니다.
- route/tree reconciliation을 한 위치에 두면 move/rename 후속 처리의 기준점이 생깁니다.

## 장점

- `FileTreePanel` 변경 면적과 component 책임이 줄어듭니다.
- long press, `⋮`와 keyboard action이 같은 row component에 연결될 수 있습니다.
- action context 규칙을 UI rendering 없이 검증할 수 있습니다.
- 기존 integration test가 리팩터링 회귀 방어막으로 유지됩니다.

## 단점

- 파일 수와 component 간 event forwarding이 증가합니다.
- 단기적으로 신규 기능 개발이 중단됩니다.
- 지나친 generic command abstraction을 추가하면 오히려 추적성이 떨어질 수 있습니다.

## 대안

- 현재 구조 유지: 단기 속도는 빠르지만 A07–A09에서 tree component 결합도가 커집니다.
- 전역 command bus: 확장성은 있지만 현재 규모에서는 event 흐름이 불투명해집니다.
- tree library 도입: drag/drop은 빨라질 수 있으나 현재 ARIA, lazy state와 design system을
  재검증해야 합니다.

## 운영 고려사항

- production bundle과 PWA precache가 정상 생성되어야 합니다.
- Compose frontend만 교체하고 backend/Vault는 변경하지 않습니다.
- 기존 mobile drawer, search, editor와 browser draft 회귀를 전체 test로 확인합니다.
- Markdown editor chunk size warning은 기존 상태로 유지하며 이번 범위에서 다루지 않습니다.

## 종료 기준

- frontend lint, type-check, unit test, production/PWA build 통과
- 기존 DOM accessibility와 keyboard test 통과
- 실제 Compose frontend/backend healthy
- 사용자 보호 파일 staging 제외
- `FileTreePanel`에서 projection과 주요 표시 markup 분리
- A07–A09가 사용할 tree action context 경계 확보

## 구현 결과

- visible tree projection과 document create parent 결정을 `treeView` pure model로 분리했습니다.
- expanded subtree, ARIA position, cycle guard와 focus/selected/root fallback을 unit test로
  고정했습니다.
- created document의 parent refresh, route push와 mobile drawer close를
  `useWorkspaceDocumentNavigation`으로 이동했습니다.
- toolbar, ARIA treeitem과 nested directory state를 각각 독립 component로 분리했습니다.
- `FileTreePanel`은 546줄에서 354줄로 감소하고 store/client orchestration과 keyboard policy에
  집중합니다.
- API contract, route, Pinia state와 사용자 interaction은 변경하지 않았습니다.
