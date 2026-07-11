# Decision Record

## ADR-001: Markdown 파일을 Source of Truth로 둔다

### 결정

`knowledge/` 아래의 Markdown 파일을 유일한 원본으로 둔다.

### 이유

- AI Agent가 가장 적은 토큰으로 읽고 쓸 수 있다.
- VS Code, CLI, Git, 검색 도구와 자연스럽게 호환된다.
- 특정 노트앱 DB에 종속되지 않는다.
- 백업과 복구가 단순하다.

### 단점

- DB 중심 앱보다 고급 기능 구현이 느릴 수 있다.
- 파일 이동/삭제/충돌 처리를 직접 설계해야 한다.
- 모바일 UX는 별도 구현이 필요하다.

### 대안

- Joplin/Many Notes처럼 DB를 원본으로 사용
- GitJournal처럼 Git을 동기화 중심으로 사용
- WebDAV 기반 파일 편집 앱 조합

## ADR-002: DB는 캐시로만 사용한다

### 결정

SQLite나 검색 엔진은 재생성 가능한 캐시로만 사용한다.

### 이유

AI가 파일을 직접 수정해도 앱 상태가 복구 가능해야 한다.

### 단점

인덱스 재생성 비용이 발생한다.

### 운영 기준

`rm cache.db && rebuild-index` 후에도 앱이 정상 동작해야 한다.

## ADR-003: Git은 백업과 감사 로그로 사용한다

### 결정

Git은 필수 동기화 계층이 아니라 백업, diff, rollback 도구로 사용한다.

### 이유

모바일에서 Git UX는 좋지 않다. 다만 AI가 파일을 수정하는 환경에서는 변경 추적과 복구가 중요하다.

### 단점

실시간 동기화 문제는 Git이 해결하지 않는다.

## ADR-004: MVP는 직접 구현을 우선 검토한다

### 결정

Many Notes와 Flatnotes를 직접 합치는 대신, 두 프로젝트를 참고 구현으로 분석한다.

### 이유

- Many Notes는 UI가 좋지만 DB 중심이다.
- Flatnotes는 파일 중심이지만 flat folder 철학이라 트리 구조 요구와 충돌한다.
- 두 프로젝트를 억지로 합치면 유지보수 비용이 커진다.

### 대안

- Flatnotes fork 후 tree 구조 추가
- Many Notes fork 후 filesystem-first로 대수술
- 완전 신규 구현

