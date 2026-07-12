# Frontend Components

- 상태: Proposed
- 최종 갱신: 2026-07-12

## 결론

KnowledgeOS의 frontend는 모바일 우선 파일 작업공간입니다. 첫 화면은 랜딩 페이지가 아니라 파일 트리, 검색, Markdown editor에 바로 접근하는 작업 화면이어야 합니다.

## 선택 이유

목표 사용자는 짧은 모바일 캡처와 긴 PC 편집을 모두 수행합니다. 따라서 UI는 노트앱처럼 가볍게 열리고, 코드 에디터처럼 경로와 변경 상태를 명확히 보여줘야 합니다.

## 화면 구조

```text
AppShell
├── TopBar
├── NavigationRail / BottomNav
├── FileTreePanel
├── EditorPane
├── SearchPanel
└── InspectorPanel
```

모바일에서는 패널을 동시에 펼치지 않습니다.

```text
Mobile tabs:
Tree | Edit | Search | Info
```

데스크톱에서는 `FileTreePanel`, `EditorPane`, `InspectorPanel`을 동시에 볼 수 있습니다.

## 핵심 컴포넌트

### AppShell

역할:

- 인증 상태 확인
- 현재 workspace 상태 로드
- 패널 layout 관리
- offline/online 상태 표시

상태:

- `activePath`
- `activePanel`
- `dirtyFiles`
- `syncStatus`

### FileTreePanel

역할:

- `GET /api/tree` 결과 렌더링
- 디렉터리 접기/펼치기
- 파일 생성, 폴더 생성, rename, move, delete
- 모바일에서 빠른 note capture 진입점 제공

설계 기준:

- 터치 target은 최소 44px 높이를 유지합니다.
- rename과 delete는 즉시 실행하지 않고 확인 단계를 둡니다.
- drag and drop은 데스크톱 우선 기능으로 두고, 모바일은 action sheet를 사용합니다.

### EditorPane

역할:

- Markdown 편집
- 저장 상태 표시
- hash 기반 충돌 감지
- preview 전환

후보 기술:

- CodeMirror 6
- Markdown parser는 preview 전용으로 사용
- autosave는 debounce 후 `PUT /api/files/{path}` 호출

저장 상태:

```text
clean
dirty
saving
conflict
error
```

충돌이 발생하면 현재 입력 내용과 서버 최신 내용을 모두 보존합니다. 자동 overwrite는 하지 않습니다.

### SearchPanel

역할:

- 전체 텍스트 검색
- path, tag, heading 필터
- 최근 검색어 표시
- 검색 결과에서 editor로 이동

검색 결과는 path, title, snippet, modified time을 보여줍니다. 모바일에서는 snippet보다 path와 제목을 우선합니다.

### InspectorPanel

역할:

- frontmatter 표시
- backlinks
- outgoing links
- tags
- file metadata

MVP에서는 read-only로 시작합니다. frontmatter 편집 UI는 Markdown 본문과 충돌할 수 있으므로 별도 설계 후 추가합니다.

### CommandPalette

역할:

- 파일 열기
- 새 노트
- 검색
- Git backup
- reindex

모바일에서는 command palette보다 bottom sheet 형태가 적합합니다.

## 모바일 UX 기준

- 첫 화면에서 최근 파일 또는 파일 트리를 즉시 보여줍니다.
- 새 노트 생성은 두 번 이하의 터치로 가능해야 합니다.
- editor는 화면 폭 전체를 사용합니다.
- toolbar는 키보드가 올라온 상태에서도 접근 가능해야 합니다.
- 긴 파일명은 middle ellipsis로 줄입니다.
- 저장 실패와 충돌은 toast만으로 끝내지 않고 복구 action을 제공합니다.

## 상태 관리

서버 상태와 UI 상태를 분리합니다.

```text
Server state:
- tree
- file content
- search result
- metadata

Client state:
- open panels
- expanded directories
- editor draft
- last active path
- theme
```

서버 상태는 query cache로 관리하고, editor draft는 별도 local state로 관리합니다. 저장 중 실패해도 사용자의 입력을 잃지 않기 위해서입니다.

## 접근성

- 모든 icon button은 accessible label을 가져야 합니다.
- 키보드만으로 tree와 editor 이동이 가능해야 합니다.
- 색상만으로 저장 상태를 표현하지 않습니다.
- 모바일 터치 target과 desktop keyboard shortcut을 모두 고려합니다.

## 대안

- Native app: 모바일 UX는 좋지만 배포와 AI filesystem 접근성이 떨어집니다.
- Desktop-first web editor: PC 편집은 좋지만 Many Notes 수준의 모바일 사용성을 달성하기 어렵습니다.
- 기존 note UI fork: 초기 화면은 빠르지만 filesystem-first와 충돌하는 상태 모델을 계속 끌고 가게 됩니다.

## 운영 시 고려사항

- PWA cache가 오래된 API schema를 잡고 있을 수 있으므로 version endpoint가 필요합니다.
- 대용량 vault에서는 tree 전체 로딩 대신 lazy loading이 필요합니다.
- 모바일 브라우저 background 상태에서 autosave가 중단될 수 있습니다.
- 오프라인 편집은 별도 conflict queue 설계 전까지 MVP 범위에서 제외합니다.
