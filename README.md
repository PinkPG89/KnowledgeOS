# 🌐 KnowledgeOS (지식 운영체제)

KnowledgeOS는 **사람(Human)**과 **AI Agent**가 동일한 로컬 파일 시스템 상의 **Markdown(`.md`) 저장소**를 함께 공유하고, 협업하여 지식을 읽고 쓰기 위해 설계된 **파일 기반(File-based) 지식 작업공간**입니다.

이 프로젝트는 초심자 개발자도 프로젝트의 구조와 작동 방식을 한눈에 이해하고 로컬에서 바로 실행해볼 수 있도록 돕기 위해 작성되었습니다.

---

## 💡 핵심 철학 및 원칙

KnowledgeOS의 설계 철학은 매우 명확하며 단순합니다.

```text
┌─────────────────────────────────────────────────────────────┐
│ Markdown files       =  Source of Truth (유일한 원천 데이터) │
│ Database / index     =  Rebuildable cache (재생성 가능한 캐시)│
│ UI (Frontend)        =  Human client (인간 사용자를 위한 화면) │
│ AI Agent             =  Direct filesystem client (직접 파일 접근)│
│ Git                  =  Version history and audit (이력 & 추적 도구)│
└─────────────────────────────────────────────────────────────┘
```

* **데이터의 주인은 파일입니다**: 데이터베이스는 언제든지 날아가거나 완전히 새로 구축해도 상관없는 '보조 캐시'일 뿐입니다. 모든 진짜 지식은 리눅스 파일 시스템 내부의 마크다운 파일(`.md`)로 영구 보존됩니다.
* **AI와 인간의 동등한 접근성**: 인간은 브라우저(UI)를 통해 깔끔한 화면으로 노트를 작성하고, AI Agent는 별도의 복잡한 API나 제약 사항 없이 로컬 파일 시스템을 통해 마크다운 파일을 직접 열어서 읽고 수정합니다.
* **안전한 변경 기록과 복제**: Git commit은 변경 이력과 복구 지점을 만들고, 별도 disk·NAS·server의 repository 복제 또는 encrypted offsite backup이 장치 장애에 대비합니다.

---

## 🎯 개발 목표

### 1차 MVP (Minimum Viable Product) 핵심 기능
- **모바일 우선(Mobile-First) PWA**: 스마트폰(iPhone, Android)과 PC에서 끊김 없는 사용자 경험 제공.
- **실제 디렉터리 기반 파일 트리**: 로컬 시스템의 폴더 구조를 그대로 반영하여 직관적인 탐색 가능.
- **마크다운 편집기**: 강력하고 부드러운 노트 작성 인터페이스.
- **파일 CRUD 및 Git version snapshot**: 파일 생성, 조회, 수정, 삭제, 이름 변경과 변경 이력을 기록하고 별도 저장소로 복제.
- **전문 검색(Full-text Search)**: 본문 텍스트 전체를 관통하는 빠른 검색 기능.

### 포함하지 않는 범위 (Non-Goals)
- 복잡한 데이터베이스 기반 Notion식 블록 DB 구현 (파일 본연의 단순함을 지향합니다).
- 초기 버전에서의 실시간 동시 편집이나 그래프 뷰 기능 배제 (기본적인 파일 조작 및 저장 성능에 집중합니다).

---

## 📂 프로젝트 구조 안내

```text
KnowledgeOS/
├── backend/     # Rust 기반 filesystem core와 REST API
├── docker/      # 컨테이너화 및 손쉬운 서비스 배포 구성 파일
├── docs/        # 시스템 아키텍처 및 상세 세부 설계 문서 모음
├── frontend/    # 사용자가 직접 보게 되는 모바일 우선 PWA 웹 UI (HTML, CSS, JS)
└── knowledge/   # 실제 사용자와 AI가 공동 작업할 로컬 Markdown 문서 저장소 (Core Vault)
```

Docker 개발 실행 방법은 [Docker Development Runtime](docs/docker.md)을 참고합니다.

현재 구현된 backend 기능은 application bootstrap, `/api/health`, canonical path 검증, 단일 활성 Vault containment, UTF-8 Markdown 읽기, 안전한 생성·수정과 lazy Tree API입니다. 다음 단계는 Vue 3 PWA 기반을 구성하고 이후 검색, Git version snapshot과 repository 복제, 인증을 진행합니다.

---

## 🧭 로드맵 작업 식별자 읽는 법

설계 문서와 작업 계획에서 사용하는 `A03`, `B01` 같은 표기는 **KnowledgeOS 프로젝트 전용 작업 식별자**입니다. Rust나 소프트웨어 업계의 표준 용어가 아니라, 여러 작업 흐름을 작고 추적 가능한 단위로 관리하기 위한 규칙입니다.

```text
A03
│└─ 03: 해당 Track 안에서의 작업 순서
└── A: 작업 영역(Track)
```

| Track | 작업 영역 | 주요 범위 | 예시 |
|---|---|---|---|
| `Axx` | Filesystem Core | Rust backend, 경로 보안, Vault, Markdown CRUD, Tree API | `A03` Vault containment |
| `Bxx` | Frontend Foundation | Vue 3 PWA, 반응형 화면, 파일 트리, editor 상태 | `B01` PWA project skeleton |
| `Cxx` | Search Projection | SQLite FTS5 index, Markdown projection, 검색 API와 UI | `C01` index schema |
| `Dxx` | External Change Safety | 외부 파일 변경 감지, UI 갱신, 충돌 검토 | `D01` filesystem watcher |

- 숫자는 프로젝트 전체의 절대 순서가 아니라 **같은 Track 내부의 순서**입니다. 따라서 선행 조건이 충족되면 `Axx`와 `Bxx` 작업을 병렬로 진행할 수 있습니다.
- 번호는 작업의 식별과 이력 추적을 위해 유지합니다. 작업 순서가 바뀌어도 기존 번호의 의미를 다른 기능으로 재사용하지 않습니다.
- `Phase`는 제품 수준의 큰 목표(MVP, 안정화 등)를 나타내고, `Track`은 기술 영역별 구현 흐름을 나타냅니다. 하나의 Phase에 여러 Track 작업이 포함될 수 있습니다.
- 새로운 알파벳 Track은 의미와 범위를 문서에 먼저 정의한 뒤 사용합니다.

전체 작업 목록과 현재 완료 상태는 [Incremental Implementation Plan](docs/incremental-implementation-plan.md), 제품 단계는 [Roadmap](docs/roadmap.md)에서 확인할 수 있습니다.

---

## ⚙️ 백엔드(Rust) 아키텍처 및 소스 파일 상세 가이드

백엔드는 가볍고 극도로 안전하며 빠른 동작 속도를 자랑하는 **Rust 언어**와 **Axum 웹 프레임워크**로 개발되었습니다. 초심자를 위해 파일별 코드 동작 원리와 사용된 핵심 개념을 하나씩 해부해 드립니다.

### 1. `backend/src/main.rs` (서버 실행 진입점)
* **역할**: 백엔드 애플리케이션의 시작점이자 본체입니다.
* **핵심 기능**:
  - 환경 변수에서 로컬 주소와 로그 설정 정보를 로드합니다.
  - 로그 라이브러리(`tracing`)를 JSON 형식으로 구동하여 Docker나 클라우드 환경에서 분석하기 편리하게 로그를 표준 출력(Stdout)합니다.
  - 지정한 IP/Port(기본 `127.0.0.1:3000`)에 소켓 리스너를 결합(`bind`)하고 Axum HTTP 서버를 기동합니다.
  - `Graceful Shutdown` 기술을 탑재하여 Ctrl+C 나 SIGTERM(컨테이너 종료 신호)을 수신하면 실행 중인 사용자 요청을 안전하게 다 완료한 뒤에 프로세스를 안전하게 닫습니다.

### 2. `backend/src/lib.rs` (모듈 조립소)
* **역할**: 애플리케이션의 핵심 라이브러리 루트입니다.
* **핵심 기능**:
  - 하위 모듈(`api`, `config`, `error`)들을 선언하여 외부(및 main.rs)에서 접근 가능하도록 문을 열어줍니다.
  - `build_router` 함수를 가지고 있어, 웹 서버에 장착될 전체 HTTP API 경로(Route)와 전역 상태(AppConfig)를 조립합니다.
  - *왜 main.rs와 lib.rs를 나눌까요?* 실행 파일과 조립 공식을 분리해두면, 실제 컴퓨터에 네트워크 포트를 할당해 열지 않고도 메모리 상에서 API 통신을 모방하여 정교한 통합 테스트를 빠르게 수행할 수 있기 때문입니다.

### 3. `backend/src/config.rs` (설정 가동 및 검증)
* **역할**: 환경 변수를 파싱하고 올바른 형식인지 검증합니다.
* **핵심 기능**:
  - `KNOWLEDGEOS_BIND_ADDRESS`, `KNOWLEDGEOS_KNOWLEDGE_ROOT`, `KNOWLEDGEOS_LOG`, `KNOWLEDGEOS_MAX_MARKDOWN_BYTES` 환경 변수를 시스템으로부터 탐색합니다.
  - 값이 지정되지 않은 경우 적절한 기본값(예: 로컬 3000포트, `../knowledge` 디렉터리)으로 즉시 채워줍니다.
  - 주소 정보 문자열을 실제 주소 객체(`SocketAddr`)로 파싱하는데, 이 형식이 잘못되었다면 첫 클라이언트가 접속을 시도할 때가 아니라 **서버가 켜지는 즉시 에러를 내며 멈추도록(Fail-Fast)** 설계되었습니다.

### 4. `backend/src/error.rs` (중앙 집중식 에러 관리소)
* **역할**: 서버 구동과 요청 처리 중 발생할 수 있는 모든 오류를 통제합니다.
* **핵심 기능**:
  - Rust 진영에서 널리 쓰이는 `thiserror` 라이브러리를 통해 프로젝트 전용 에러 열거형(`AppError`)을 구성합니다.
  - 입출력 에러(`io::Error`), 주소 파싱 에러(`AddrParseError`) 등 서드파티 라이브러리 에러들을 `#[from]` 어노테이션을 통해 `AppError`로 자동 변환하여 깔끔하고 통합된 에러 핸들링 코드를 작성할 수 있게 돕습니다.

### 5. `backend/src/api/mod.rs` (API 진입 채널)
* **역할**: HTTP 웹 통신의 최전선 게이트웨이입니다.
* **핵심 기능**:
  - 들어오는 HTTP 요청을 받고 응답 데이터를 다듬는 로직만을 수행합니다.
  - 파일 조작이나 분석 등 실제 알맹이 비즈니스 로직은 향후 이 모듈 밖에 독립적으로 정의함으로써, Axum 웹 프레임워크와의 결합도를 최소화하고 깔끔한 아키텍처를 유지하게 만듭니다.

### 6. `backend/src/api/health.rs` (헬스체크 어댑터)
* **역할**: 서버 상태 모니터링을 담당합니다.
* **핵심 기능**:
  - `/api/health` 경로로 들어오는 HTTP GET 요청을 처리합니다.
  - 서버의 현재 작동 여부("ok"), 컴파일된 백엔드 버전, 그리고 바라보고 있는 저장소 디렉터리 경로를 JSON 포맷으로 생성하여 응답합니다.
  - `serde::Serialize` 매크로를 사용하여 복잡한 문자열 처리 없이 Rust 구조체 데이터를 컴퓨터 간 통신 언어인 JSON으로 빠르게 자동 인코딩합니다.

### 7. `backend/tests/health_contract.rs` (통합 및 계약 테스트)
* **역할**: 백엔드 API가 의도한 사양대로 제대로 응답하는지 검사합니다.
* **핵심 기능**:
  - 가짜 설정(`AppConfig::for_test()`)을 이용해 메모리에 라우터를 띄우고 가짜 요청을 주입합니다.
  - 서버 상태 코드가 정확히 `200 OK`인지, 결과 JSON 데이터 안에 담긴 키-값들이 기획된 약속대로 반환되었는지 검사합니다.
  - 실제 네트워크 자원을 전혀 소모하지 않으므로 컴퓨터 사양과 무관하게 1초 미만의 속도로 안전하게 테스트가 끝납니다.

### 8. `backend/src/infrastructure/markdown.rs` (안정적인 Markdown Reader)
* Vault containment를 통과한 regular file만 최대 5 MiB까지 읽습니다.
* 읽기 전후 metadata를 비교하고 변경 시 한 번 재시도합니다.
* UTF-8 원문, SHA-256 hash, byte size와 수정 시각을 생성합니다.

### 9. `backend/src/api/files.rs`, `backend/src/api/error.rs` (파일 API)
* `GET /api/files/{*path}` wildcard route로 중첩 Markdown 경로를 읽습니다.
* blocking filesystem 작업은 Tokio blocking pool에서 실행합니다.
* 오류를 안정적인 HTTP status와 JSON error code로 변환하며 절대 filesystem 경로를 응답에 노출하지 않습니다.

---

## 🛠️ 개발 환경 구축 및 로컬 실행 방법 (Step-by-Step)

초심자 개발자가 로컬 컴퓨터에서 백엔드 서버를 켜기 위한 실습 가이드입니다.

### 1단계: Rust 컴파일러 설치
Rust 프로그램의 빌드와 패키지 관리를 위해 `rustup`을 설치해야 합니다.

* **Linux / macOS**:
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
  설치가 끝나면 터미널을 다시 켜거나 `source $HOME/.cargo/env`를 실행하여 환경 설정을 갱신합니다.

* **Windows**:
  [Rust 공식 웹사이트(rust-lang.org)](https://www.rust-lang.org/tools/install)에서 인스턴스 설치 파일을 받아 실행합니다.

### 2단계: 백엔드 빌드 및 컴파일
소스 코드가 있는 프로젝트 폴더의 백엔드 디렉터리로 들어갑니다.

```bash
cd backend
```

아래 명령어를 입력하면 필요한 패키지 라이브러리들을 자동으로 인터넷에서 다운로드하고 컴파일하여 실행 파일을 생성합니다.

```bash
cargo build
```

### 3단계: 가상 통합 테스트 실행하기
코드가 설계 사양대로 정상 작동하는지 계약 테스트 코드를 수행합니다.

```bash
cargo test
```
*성공적으로 실행되면 `health_endpoint_matches_public_contract ... ok` 메시지가 터미널에 나타납니다.*

### 4단계: 서버 가동 (실행)
기본 설정값을 사용해 서버를 켭니다.

```bash
cargo run
```
서버가 켜지면 터미널에 아래와 같은 JSON 로그 메시지가 나타납니다.
```json
{"timestamp":"...","level":"INFO","fields":{"message":"KnowledgeOS backend started","address":"127.0.0.1:3000"},"target":"knowledgeos_backend"}
```

### 💡 보너스: 환경 변수 커스터마이징 실행
만약 기본 디렉터리가 아닌 별도의 마크다운 디렉터리를 설정하고 싶거나 포트 번호를 8080으로 바꾸고 싶다면, 실행할 때 아래와 같이 임시 환경 변수를 부여할 수 있습니다.

* **Linux / macOS (Bash)**:
  ```bash
  KNOWLEDGEOS_BIND_ADDRESS="127.0.0.1:8080" KNOWLEDGEOS_KNOWLEDGE_ROOT="/Users/myusername/my-markdown-vault" cargo run
  ```

* **Windows (PowerShell)**:
  ```powershell
  $env:KNOWLEDGEOS_BIND_ADDRESS="127.0.0.1:8080"
  $env:KNOWLEDGEOS_KNOWLEDGE_ROOT="C:\my-markdown-vault"
  cargo run
  ```

---

## 📖 시스템 설계 세부 문서(docs/) 읽어보기

프로젝트의 본격적인 구조 변경과 확장을 원하신다면 `docs/` 내 설계 파일들이 나침반 역할을 해줍니다.
- [Architecture.md](docs/architecture.md): 마크다운 파일 중심의 데이터 영속성 처리 및 AI의 격리 수준 설계.
- [Directory Structure.md](docs/directory-structure.md): 마크다운 첨부파일 저장 원칙 및 캐싱 데이터 무효화 주기 규칙.
- [API Draft.md](docs/api.md): CRUD 요청 시 주고받는 세부 JSON 통신 스펙 정의서.
- [Frontend Components.md](docs/frontend-components.md): 모바일 환경에 특화된 네비게이션 트리 뷰 및 에디터 화면 UI 청사진.
- [Data Model.md](docs/data-model.md): 위키 링크(WikiLinks) 파싱 규약 및 동시 수정 충돌 시 병합 전략.
- [Decision Record.md](docs/decision-record.md): DB 대신 파일 중심 구조를 왜 택했는지에 대한 핵심 의사결정 사유 기록(ADR).
- [Roadmap.md](docs/roadmap.md): MVP 제작부터 파일 인덱싱 검색, 최종 AI Agent 연동까지의 연차별 마일스톤.
- [Reference Implementation Analysis.md](docs/reference-implementation-analysis.md): 유사 경쟁 오픈소스들의 벤치마킹 장단점 분석서.
- [Incremental Implementation Plan.md](docs/incremental-implementation-plan.md): 점진적으로 모듈을 빌드하고 유닛 테스트로 검증하기 위한 꼼꼼한 마일스톤 계획서.
- [Path Policy.md](docs/path-policy.md): API와 domain layer가 공유하는 안전한 상대 경로 규칙.
- [Vault Policy.md](docs/vault-policy.md): 단일 활성 Vault 선택과 symlink·containment 보안 경계.
- [Git Backup Policy.md](docs/git-backup.md): Git version snapshot, 별도 repository 복제와 선택적 offsite backup 운영 기준.
