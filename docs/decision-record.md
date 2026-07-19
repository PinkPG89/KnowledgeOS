# Decision Record

- 상태: Active
- 최종 갱신: 2026-07-12

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

## ADR-003: Git은 version history와 감사 로그로 사용한다

### 결정

Git은 필수 동기화 계층이 아니라 version snapshot, diff, rollback 도구로 사용한다. 장치 장애 복구는 별도 disk, NAS 또는 server의 repository 복제와 선택적 offsite backup으로 담당한다.

### 이유

모바일에서 Git UX는 좋지 않다. 다만 AI가 파일을 수정하는 환경에서는 변경 추적과 복구가 중요하다.

### 단점

실시간 동기화 문제는 Git이 해결하지 않는다.

같은 disk의 local commit은 disk 장애에 대한 backup이 아니다. application source repository와 사용자 Vault repository도 분리해야 한다.

### 운영 기준

- 개인 server MVP는 별도 Git service 없이 bare repository를 사용할 수 있다.
- Forgejo나 Gitea는 Web UI, 다중 사용자, 권한 관리가 필요할 때만 도입한다.
- 상세 정책은 [Git Versioning and Backup Policy](git-backup.md)를 따른다.

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

## ADR-005: Backend는 Python과 FastAPI를 사용한다

상태: ADR-006으로 대체됨

### 결정

- Python 최소 버전은 3.12로 한다.
- FastAPI와 Pydantic Settings를 사용한다.
- dependency와 lock file은 `uv`로 관리한다.
- package는 `src` layout으로 구성한다.
- pytest, Ruff, mypy를 필수 품질 도구로 사용한다.

### 선택 이유

- filesystem 처리와 AI, MCP, PydanticAI 계층을 같은 생태계에서 운영할 수 있다.
- Pydantic 모델을 API 입력 검증과 설정 검증에 함께 활용할 수 있다.
- `uv`는 개발 환경과 container build에서 동일 lock을 빠르게 재현할 수 있다.
- `src` layout은 설치되지 않은 로컬 package를 실수로 import하는 문제를 줄인다.

### 장점

- OpenAPI contract를 자동 생성할 수 있다.
- strict typing과 runtime validation을 함께 적용할 수 있다.
- AI 기능 추가 시 언어 경계를 만들지 않아도 된다.

### 단점

- CPU 집약 작업과 대규모 동시성에서는 Go 또는 Rust보다 불리할 수 있다.
- Python dependency와 type checker 설정을 지속적으로 관리해야 한다.

### 대안

- Go와 Chi 또는 Fiber로 filesystem API 구현
- TypeScript와 Fastify로 frontend와 언어 통일
- Django와 Django REST Framework 사용

### 운영 기준

- production과 CI는 `uv.lock`을 frozen mode로 설치한다.
- application log는 JSON 형식으로 표준 출력에 기록한다.
- 환경 설정은 `KNOWLEDGEOS_` prefix를 사용하고 secret을 repository에 저장하지 않는다.
- API process는 Markdown 원본과 index cache의 수명을 분리한다.

## ADR-006: Core Backend는 Rust와 Axum을 사용한다

### 결정

- core backend는 Rust 2024 edition과 Axum을 사용한다.
- async runtime은 Tokio, serialization은 Serde를 사용한다.
- application log는 `tracing`을 사용해 JSON으로 출력한다.
- `unsafe` code는 project lint로 금지한다.
- formatting, test, strict Clippy를 필수 품질 gate로 사용한다.
- Python AI runtime, PydanticAI, MCP는 core backend dependency에 포함하지 않는다.

### 선택 이유

- KnowledgeOS backend의 주 책임은 AI orchestration이 아니라 안전한 filesystem 작업이다.
- path 검증, atomic write, watcher, hash, SQLite, 동시성 제어는 Rust의 type과 ownership model에 적합하다.
- 외부 AI는 `knowledge/`를 직접 공유하므로 backend 내부 Python AI 생태계가 필수가 아니다.
- 단일 binary와 낮은 memory 사용량은 self-hosted 운영에 유리하다.

### 장점

- compile time에 type, lifetime, thread safety 문제를 조기에 발견한다.
- garbage collector 없이 예측 가능한 memory와 latency를 제공한다.
- runtime이 포함된 단일 binary로 배포할 수 있다.
- `Result` 기반 오류 처리를 통해 실패 경로를 명시한다.

### 단점

- Python 또는 Go보다 초기 개발 속도가 느릴 수 있다.
- ownership, lifetime, trait, async에 대한 학습 비용이 있다.
- compile time과 dependency build cache를 관리해야 한다.

### 대안

- Go: 개발 속도와 운영 효율의 균형이 좋지만 Rust보다 type과 memory safety 보장이 약하다.
- C#과 ASP.NET Core: 생산성과 framework 성숙도가 높지만 배포 runtime과 규모가 더 크다.
- Python과 FastAPI: PoC에는 빠르지만 core storage server의 장기 효율성과 compile-time 안전성이 낮다.

### 운영 기준

- CI와 release build는 `Cargo.lock`을 `--locked`로 검증한다.
- `cargo fmt --check`, `cargo test`, `cargo clippy -- -D warnings`를 통과해야 한다.
- panic에 의존하지 않고 운영 오류는 typed `Result`로 반환한다.
- local external AI 접근은 filesystem을 기본으로 하고 원격 adapter는 별도 component로 분리한다.
- 학습을 돕는 주석은 Rust 문법 설명보다 ownership, error boundary, architecture 선택 이유에 집중한다.

## ADR-007: MVP는 단일 활성 Vault를 사용한다

### 결정

- process 하나당 하나의 활성 Vault만 사용한다.
- `KNOWLEDGEOS_KNOWLEDGE_ROOT` 변경 후 재시작해 Vault를 바꾼다.
- 설정된 root symlink는 startup에서 한 번 해석한다.
- Vault 내부 descendant symlink는 대상 위치와 관계없이 모두 거부한다.

### 선택 이유

- 외부 AI와 UI가 동일한 Markdown directory를 명확한 경계로 공유해야 한다.
- watcher, index, Git backup을 하나의 canonical root에 연결하면 운영 상태가 단순해진다.
- root symlink만 허용하면 mount 유연성을 유지하면서 내부 path 정책을 일관되게 적용할 수 있다.

### 장점

- 잘못된 root 설정을 network bind 전에 발견한다.
- 모든 filesystem use case가 동일한 containment 정책을 공유한다.
- Vault 선택과 내부 symlink 허용 여부를 분리할 수 있다.

### 단점

- Vault 변경 시 process 재시작이 필요하다.
- 여러 Vault의 동시 검색과 실행 중 전환을 지원하지 않는다.
- 적대적 local process에 대한 TOCTOU 가능성은 별도 강화가 필요하다.

### 대안

- 실행 중 단일 Vault 전환
- 여러 Vault 동시 활성화
- root를 포함한 모든 symlink 거부
- containment를 만족하는 descendant symlink 허용

### 운영 기준

- startup에서 root 존재, directory 여부, canonical path, 열람 가능성을 검증한다.
- index와 watcher는 startup에서 고정한 canonical root만 사용한다.
- 고위험 다중 사용자 운영이 필요하면 `openat2` 또는 capability handle 방식을 도입한다.
