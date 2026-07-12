# Reference Implementation Analysis

- 상태: Accepted Snapshot
- 최종 갱신: 2026-07-12

## 목적

Many Notes와 Flatnotes를 포크하거나 통째로 병합하지 않는다. 각 프로젝트에서 검증된 동작과 설계 판단만 추출하고, KnowledgeOS의 filesystem-first 아키텍처와 기술 스택에 맞게 재구현한다.

분석 기준일은 2026-07-11이다.

| 프로젝트 | Upstream | 분석 commit | License | 주 참고 영역 |
| --- | --- | --- | --- | --- |
| Many Notes | `brufdev/many-notes` | `33e89624b909b22b277096c68b5280c5bfd27265` | MIT | 모바일 shell, lazy tree, panel 상태, 파일 열기 UX |
| Flatnotes | `Dullage/flatnotes` | `0fdd0fd14a29f7dee69d38ac974fef936bc166fd` | MIT | filesystem 저장, 재생성 가능한 검색 index, draft 복구 |

Upstream 소스는 KnowledgeOS 저장소에 vendor하지 않는다. 분석 시 commit을 고정하고, 실제 코드를 복사한 경우에만 원본 파일과 license attribution을 별도로 기록한다.

## 채택 원칙

1. 먼저 사용자 동작과 불변 조건을 문서화한다.
2. KnowledgeOS API와 데이터 모델에 맞지 않는 식별자, DB 모델, framework 코드는 가져오지 않는다.
3. 50줄 이상의 연속 코드 또는 구조적으로 동일한 구현을 재사용하면 `NOTICE`에 출처를 기록한다.
4. 아이디어만 참고해 독립 구현한 경우에도 이 문서에 근거 파일과 판단을 남긴다.
5. 각 항목은 독립적으로 테스트하고 merge할 수 있는 크기로 나눈다.

## Many Notes 분석

### 채택 대상

#### 반응형 3영역 shell

근거 파일:

- `resources/js/pages/vault/Show.vue`
- `resources/js/stores/layout.ts`
- `resources/js/composables/useScreenSize.ts`

추출할 동작:

- Desktop에서는 tree, editor, inspector를 동시에 표시한다.
- Mobile에서는 좌우 panel을 overlay로 열고, 파일을 선택하면 panel을 닫는다.
- Desktop panel 선호 상태만 local storage에 보존한다.
- viewport가 breakpoint를 통과할 때 현재 panel 상태를 재조정한다.

KnowledgeOS 적용:

- `AppShell`, `NavigationDrawer`, `EditorPane`, `InspectorDrawer`로 재구현한다.
- Many Notes의 `1024px` breakpoint는 초기 참고값일 뿐이며 실제 기기 검증 후 결정한다.
- URL은 DB node ID가 아니라 percent-encoded relative path를 사용한다.

#### Lazy file tree

근거 파일:

- `resources/js/components/tree/VaultTree.vue`
- `resources/js/components/tree/VaultTreeNode.vue`
- `resources/js/stores/vaultTree.ts`
- `resources/js/composables/useVaultTreeActions.ts`

추출할 동작:

- 폴더를 처음 펼칠 때만 children을 요청한다.
- loading, loaded, expanded 상태를 분리한다.
- 폴더 우선, 파일 후순위로 정렬한다.
- 현재 파일의 ancestor를 자동으로 펼친다.
- 동일 폴더에 대한 중복 요청을 막는다.
- Mobile에서 파일을 선택하면 navigation drawer를 닫는다.

KnowledgeOS 적용:

- key는 숫자 ID가 아닌 canonical relative path다.
- tree state는 서버 DB가 아니라 client projection이다.
- drag-and-drop은 touch 접근성 대안이 준비될 때까지 MVP 후순위다.

#### 파일 상태와 최근 파일

근거 파일:

- `resources/js/stores/vaultRecentFile.ts`
- `resources/js/composables/useVaultActions.ts`
- `resources/js/components/vault/VaultFileUpdatingSpinner.vue`

추출할 동작:

- 최근 파일을 제한된 개수로 유지한다.
- 파일 열기와 tree selection을 하나의 사용자 동작으로 처리한다.
- 저장 중 상태를 전역적으로 노출한다.

KnowledgeOS 적용:

- 최근 파일은 재생성 가능한 UI preference로 취급한다.
- 저장 상태는 `clean`, `dirty`, `saving`, `conflict`, `error` 상태 머신으로 확장한다.
- content hash 기반 optimistic concurrency를 반드시 포함한다.

### 제외 대상

| 대상 | 제외 이유 |
| --- | --- |
| Laravel/Inertia backend | KnowledgeOS의 Rust/Axum REST 경계와 맞지 않음 |
| `VaultNode` DB hierarchy | filesystem path가 원본이라는 원칙을 위반함 |
| Typesense 필수 의존 | MVP 운영 복잡도가 크고 index는 선택 가능해야 함 |
| Reverb 실시간 event | 단일 사용자 MVP에 과도하며 watcher 이후 검토 대상 |
| collaboration/OAuth | 초기 제품 범위 밖 |
| Tiptap WYSIWYG 전체 구성 | Markdown source 보존과 mobile IME를 별도 검증해야 함 |

## Flatnotes 분석

### 채택 대상

#### 파일을 원본으로 두는 storage contract

근거 파일:

- `server/notes/base.py`
- `server/notes/file_system/file_system.py`
- `server/notes/models.py`

추출할 동작:

- create는 기존 파일을 덮어쓰지 않는다.
- read 결과에 수정 시간을 포함한다.
- rename은 목적지 충돌을 검사한다.
- index는 파일에서 동기화하고 다시 만들 수 있다.
- storage 구현을 API handler와 분리한다.

KnowledgeOS 적용:

- `KnowledgeRepository` protocol과 `FileSystemKnowledgeRepository` 구현으로 분리한다.
- title 기반 flat path 대신 중첩 relative path를 사용한다.
- 단순 filename 문자 검사 대신 resolve 후 root containment를 검증한다.
- UTF-8을 명시하고 symlink, hidden path, extension, file size 정책을 강제한다.
- write는 임시 파일과 atomic replace를 사용한다.
- delete는 실제 삭제가 아니라 `_trash/` 이동으로 구현한다.

#### 재생성 가능한 검색 projection

근거 파일:

- `server/notes/file_system/file_system.py`의 `_load_index`, `_sync_index`, `_sync_index_with_retry`

추출할 동작:

- schema version이 바뀌면 기존 index를 폐기하고 재생성한다.
- 새 파일, 변경 파일, 삭제 파일을 원본 디렉터리와 대조한다.
- index lock은 제한된 횟수로 retry한다.
- 검색 결과에는 snippet과 tag match를 제공한다.

KnowledgeOS 적용:

- 초기에는 SQLite FTS5를 사용하며 `.knowledgeos/index.sqlite`만 삭제해 재구축할 수 있어야 한다.
- `mtime`만으로 변경을 확정하지 않고 size와 content hash를 함께 관리한다.
- nested directory와 frontmatter tag를 지원한다.
- 검색 index 장애가 파일 CRUD를 막지 않게 한다.

#### Browser draft 복구

근거 파일:

- `client/views/Note.vue`

추출할 동작:

- 저장하지 않은 편집 내용을 browser에 임시 보존한다.
- 동일 노트를 다시 열면 server 본문과 draft 중 선택하게 한다.
- 페이지 이탈 전에 dirty 상태를 확인한다.

KnowledgeOS 적용:

- draft key는 canonical path와 server base hash 조합으로 만든다.
- server hash가 달라졌으면 자동 복원하지 않고 conflict 화면을 표시한다.
- draft는 cache이므로 사용자가 명시적으로 폐기할 수 있어야 한다.

### 제외 대상

| 대상 | 제외 이유 |
| --- | --- |
| flat folder title model | KnowledgeOS는 실제 directory tree를 원본으로 사용함 |
| Whoosh 구현 자체 | 유지보수 상태와 배포 의존성보다 SQLite FTS5가 적합함 |
| `os.path.join` 중심 검증 | nested path traversal과 symlink 방어에 충분하지 않음 |
| direct overwrite | crash 시 partial write 위험과 동시 수정 감지가 없음 |
| hard delete | KnowledgeOS 복구 정책과 맞지 않음 |
| Toast UI Editor | editor 선택은 CodeMirror 6 mobile PoC 후 결정함 |

## 비교 결론

| KnowledgeOS 영역 | 주 참고 구현 | 가져올 것 | 가져오지 않을 것 |
| --- | --- | --- | --- |
| Mobile shell | Many Notes | panel 전환 동작, 파일 선택 후 drawer 닫기 | Inertia page 구조 |
| File tree | Many Notes | lazy load 상태 모델, ancestor expansion | DB node ID |
| File repository | Flatnotes | storage contract, create collision | flat title model |
| Search | Flatnotes | rebuild/sync lifecycle, result metadata | Whoosh code |
| Draft recovery | Flatnotes | browser draft UX | hash 없는 자동 복원 |
| Editor | 양쪽 모두 일부 | toolbar/dirty state 요구사항 | editor library 직접 채택 |

## 운영 시 고려사항

- Upstream 업데이트는 자동 병합하지 않고 분기별로 기능 변화만 재검토한다.
- 보안 관련 구현은 참고 프로젝트보다 KnowledgeOS 정책을 우선한다.
- 복사한 코드는 dependency license까지 확인하고 `NOTICE`에 기록한다.
- mobile UX는 emulator만으로 승인하지 않고 iPhone Safari와 Android Chrome 실기기에서 검증한다.
- 외부 AI가 파일을 수정하는 흐름은 UI 저장 충돌 테스트에 항상 포함한다.
