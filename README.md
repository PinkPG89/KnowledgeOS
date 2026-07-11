# KnowledgeOS

KnowledgeOS는 사람과 AI Agent가 같은 Markdown 저장소를 함께 읽고 쓰기 위한 파일 기반 지식 작업공간입니다.

핵심 원칙은 단순합니다.

```text
Markdown files = Source of Truth
Database / index = Rebuildable cache
UI = Human client
AI = Direct filesystem client
Git = Backup and audit trail
```

## 목표

- iPhone, Android, PC에서 편하게 Markdown 노트를 작성한다.
- AI Agent는 별도 노트앱 API를 거치지 않고 Markdown 파일을 직접 읽고 쓴다.
- 모든 지식의 원본은 Linux filesystem의 `.md` 파일이다.
- 검색, 태그, 백링크, 인덱스는 언제든 재생성 가능한 보조 데이터로만 사용한다.
- Git은 동기화의 중심이 아니라 백업, 변경 추적, 복구 수단으로 사용한다.

## 비목표

- Obsidian 전체 기능 복제
- Notion식 block database 구현
- DB를 지식 원본으로 사용하는 구조
- AI가 모든 작업을 MCP/API를 통해서만 수행하는 구조
- 초기 버전에서 graph view, plugin marketplace, 실시간 공동 편집 구현

## 1차 MVP

- 모바일 우선 PWA
- 실제 디렉터리 기반 파일 트리
- Markdown 편집기
- 파일 생성, 읽기, 수정, 삭제, 이름 변경
- 전문 검색
- Git 자동 백업
- AI가 접근할 `knowledge/` 디렉터리 보존

## 프로젝트 구조

```text
KnowledgeOS/
├── ai/          # MCP, PydanticAI, embedding 등 선택 기능
├── backend/     # 파일 시스템 API, 검색, 인증, 백업
├── docker/      # 배포 구성
├── docs/        # 설계 문서
├── frontend/    # 모바일 우선 웹 UI
└── knowledge/   # 개발용 Markdown 원본 저장소
```

## 설계 문서

- [Architecture](docs/architecture.md): Source of Truth, DB 역할, AI 접근 방식
- [Directory Structure](docs/directory-structure.md): `knowledge/`, 첨부파일, 설정, 캐시 정책
- [API Draft](docs/api.md): REST API 경계와 요청/응답 계약
- [Frontend Components](docs/frontend-components.md): 트리, 에디터, 검색, 모바일 UI 설계
- [Data Model](docs/data-model.md): Markdown 규약, 캐시 DB, 충돌 처리
- [Decision Record](docs/decision-record.md): 주요 아키텍처 결정
- [Roadmap](docs/roadmap.md): MVP부터 AI 기능까지의 단계

## 참고 방향

- Many Notes: 모바일 UI, 트리 UX, 노트앱 사용성 참고
- Flatnotes: database-less, filesystem-first 철학 참고
- VS Code: 파일 트리와 에디터 분리 UX 참고
- Obsidian: Markdown 링크, backlink, vault 개념 참고
