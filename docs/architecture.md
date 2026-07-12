# Architecture

## 결론

KnowledgeOS의 핵심 아키텍처는 파일 시스템 중심입니다. DB를 원본으로 두지 않습니다.

```text
Human
  ↓
Mobile PWA / Web UI
  ↓
Backend File API
  ↓
knowledge/*.md
  ↑
AI Agent / Codex / Claude / GPT
```

## 선택 이유

AI Agent 관점에서 가장 저렴하고 안정적인 인터페이스는 파일 직접 접근입니다.

```text
read file
write file
rename file
delete file
search text
git diff
```

MCP, REST API, DB model을 항상 통과시키면 메타데이터와 JSON wrapper 때문에 토큰 사용량이 증가합니다. 따라서 AI의 기본 경로는 filesystem이어야 합니다.

## 핵심 원칙

### 1. Markdown 파일이 원본

`knowledge/` 아래의 Markdown 파일만 영속 지식 원본입니다.

```text
knowledge/
├── ai/
├── projects/
├── linux/
└── daily/
```

파일명, 경로, 본문, frontmatter가 실제 상태입니다.

### 2. DB는 캐시

SQLite를 쓰더라도 용도는 제한합니다.

- 검색 캐시
- backlink 캐시
- tag 캐시
- 최근 파일 목록
- UI preference

DB를 삭제해도 `knowledge/`만 있으면 전체 상태를 복구할 수 있어야 합니다.

### 3. 인덱스는 재생성 가능

Typesense, SQLite FTS, Tantivy, Meilisearch 같은 검색 엔진을 붙일 수 있지만, 모두 재생성 가능한 파생 데이터여야 합니다.

### 4. AI는 앱을 몰라도 된다

AI Agent는 KnowledgeOS의 내부 API를 몰라도 됩니다.

```text
AI Agent
  ↓
/data/.../KnowledgeOS/knowledge
  ↓
Markdown files
```

MCP는 고급 검색, 요약, 링크 추천, batch operation이 필요할 때만 선택적으로 사용합니다.

## 컴포넌트

```text
Frontend PWA
  ↓ REST
Backend API
  ↓ validated filesystem operations
knowledge/
  ↑ direct file access
AI Agent

Backend API
  ↓ rebuildable projections
.knowledgeos/index.sqlite
```

### Frontend

- 모바일 우선 PWA
- 파일 트리
- Markdown editor
- 검색
- 파일 작업
- 태그/백링크 보기

후보 기술:

- Vue 3 또는 React
- CodeMirror 6
- PWA
- Tailwind 또는 기존 UI kit

### Backend

- 파일 시스템 API
- path traversal 방어
- Markdown 읽기/쓰기
- 파일 잠금 또는 optimistic concurrency
- 검색 인덱스 재생성
- Git 자동 백업
- 인증

선택 기술:

- Rust 2024 edition
- Axum과 Tokio
- Serde와 tracing
- SQLite FTS 또는 ripgrep
- notify crate
- Git CLI

### External AI Access

외부 AI는 기본적으로 `knowledge/` filesystem을 직접 공유합니다.

- Codex, Claude Code, local agent는 direct filesystem client입니다.
- backend 내부에 LLM provider나 AI framework를 포함하지 않습니다.
- 원격 AI 접근이 필요해질 때만 별도 MCP 또는 REST adapter를 검토합니다.

선택 adapter는 원본이 아니며 삭제해도 Markdown workspace가 완전해야 합니다.

### Storage Layer

- `knowledge/`: Markdown 원본입니다.
- `.knowledgeos/`: 설정, 캐시, lock, index를 둡니다.
- Git repository: 백업과 감사 로그입니다.

`knowledge/`와 `.knowledgeos/`는 의존 방향이 반대입니다. `.knowledgeos/`는 `knowledge/`에서 재생성할 수 있지만, `knowledge/`는 `.knowledgeos/`에 의존하면 안 됩니다.

## 데이터 흐름

### 사람이 수정

```text
PWA editor
  ↓ PUT /files/{path}
Backend validates path
  ↓
Write markdown file
  ↓
Invalidate/rebuild index
  ↓
Optional git commit
```

### AI가 수정

```text
AI writes knowledge/*.md
  ↓
Filesystem watcher detects change
  ↓
Invalidate/rebuild index
  ↓
UI reflects updated file
  ↓
Optional git commit
```

## 운영 고려사항

- AI 대량 수정 전에는 `git diff`와 snapshot을 남겨야 합니다.
- 삭제는 즉시 삭제보다 `.trash/` 이동이 안전합니다.
- 모바일 네트워크 끊김을 고려해 저장 실패 UI가 필요합니다.
- 동시 수정은 초기에는 last-write 방지용 `etag` 또는 file hash로 처리합니다.
- 나중에 CRDT/실시간 협업을 붙일 수 있지만 MVP 범위에서는 제외합니다.

## 아키텍처 대안

### Many Notes fork

장점은 모바일 UX를 빠르게 확보할 수 있다는 점입니다. 단점은 DB 중심 상태 모델을 filesystem-first로 바꾸는 비용이 큽니다.

### Flatnotes fork

장점은 파일 기반 철학이 이미 맞다는 점입니다. 단점은 flat folder 중심 UX라서 KnowledgeOS의 tree, attachment, AI workflow 요구와 차이가 있습니다.

### 신규 구현

장점은 `knowledge/`를 원본으로 하는 아키텍처를 처음부터 강제할 수 있다는 점입니다. 단점은 초기 구현량이 늘어납니다.

현재 결정은 신규 구현입니다. 두 프로젝트는 코드 병합 대상이 아니라 UX와 저장 철학의 참고 구현으로 사용합니다.
