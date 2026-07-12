# Roadmap

- 상태: Active
- 최종 갱신: 2026-07-12

## Phase 0: 조사와 기준 확정

- [x] Many Notes UI 구조 분석
- [x] Flatnotes filesystem 저장 방식 분석
- [x] 참고 구현 commit과 license 기준 고정
- [x] 채택·제외 항목 문서화
- [x] 작은 구현 단위와 완료 기준 정의
- CodeMirror 6 모바일 편집 UX 검증
- Rust/Axum 파일 API PoC 진행 중
- [x] Rust/Axum backend skeleton과 health contract test
- [x] Canonical path policy와 Rust value object
- [x] 단일 활성 Vault와 symlink containment policy
- [x] UTF-8 Markdown read API와 SHA-256 snapshot
- [x] KnowledgeOS 핵심 아키텍처 확정
- [x] 디렉터리 구조 확정
- API 초안과 구현 상태 동기화
- Frontend 기술과 컴포넌트 경계 ADR 확정

완료 기준:

- 포크할지, 새로 만들지 결정
- Markdown 파일 원본 원칙 확정
- MVP API 확정
- 구현 전에 문서가 Source of Truth 역할을 수행

상세 문서:

- [Reference Implementation Analysis](reference-implementation-analysis.md)
- [Incremental Implementation Plan](incremental-implementation-plan.md)

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
- `_trash/` 복구
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

## Phase 4: Optional Remote Access

- 원격 AI용 제한 접근 adapter
- directory allowlist와 인증
- 변경 audit log
- 선택적 MCP gateway
- remote change review

완료 기준:

- local AI는 filesystem 직접 접근을 유지
- 원격 adapter를 제거해도 Markdown workspace가 완전함
- MCP는 실제 client 요구가 확인된 경우에만 도입
