# Roadmap

## Phase 0: 조사와 기준 확정

- Many Notes UI 구조 분석
- Flatnotes filesystem 저장 방식 분석
- CodeMirror 6 모바일 편집 UX 검증
- FastAPI 파일 API PoC
- KnowledgeOS 아키텍처 문서 확정
- 디렉터리 구조와 API 초안 확정
- 프론트엔드 컴포넌트 경계 확정

완료 기준:

- 포크할지, 새로 만들지 결정
- Markdown 파일 원본 원칙 확정
- MVP API 확정
- 구현 전에 문서가 Source of Truth 역할을 수행

## Phase 1: MVP

- 모바일 PWA shell
- 파일 트리
- Markdown editor
- 파일 읽기/저장
- 파일 생성/삭제/이름변경
- 검색
- Git 수동 백업

완료 기준:

- iPhone/Galaxy에서 30분 이상 실제 작성 가능
- PC에서는 VS Code로 같은 `knowledge/` 편집 가능
- AI가 파일 직접 수정 후 UI에서 반영 가능

## Phase 2: 안정화

- filesystem watcher
- index rebuild
- optimistic concurrency
- `.trash/` 복구
- 자동 Git backup
- 인증/권한
- Docker Compose

완료 기준:

- 외부 파일 수정, 삭제, 이동 후 인덱스 복구 가능
- AI 대량 수정 후 `git diff`로 검토 가능
- 서버 재시작 후 데이터 일관성 유지

## Phase 3: Knowledge 기능

- Wiki link
- Backlink
- tag index
- related notes
- duplicate detection
- note template
- daily note

완료 기준:

- Obsidian의 핵심 지식관리 흐름 일부 대체 가능
- DB를 지워도 모든 기능 재구성 가능

## Phase 4: AI 기능

- MCP server
- AI note summarizer
- AI link recommender
- AI tag recommender
- embedding search
- change review assistant

완료 기준:

- AI는 기본적으로 파일 직접 접근
- MCP는 보조 기능으로만 사용
- 토큰 비용이 큰 API wrapping을 기본 경로로 만들지 않음
