# Directory Structure

- 상태: Accepted
- 최종 갱신: 2026-07-12

## 결론

KnowledgeOS는 앱 코드와 사용자 지식을 분리합니다. 앱은 `backend/`, `frontend/`에 있고, 사용자의 영속 지식 원본은 `knowledge/`에만 둡니다. 원격 AI adapter는 실제 요구가 생길 때 root `adapters/`로 추가합니다.

```text
KnowledgeOS/
├── backend/
├── docker/
├── docs/
├── frontend/
└── knowledge/
```

## 선택 이유

AI Agent, CLI, Git, 백업 도구가 같은 Markdown 파일을 직접 다루려면 원본 위치가 명확해야 합니다. `knowledge/`를 단일 vault로 고정하면 앱을 재작성하거나 DB를 삭제해도 사용자 지식은 그대로 유지됩니다.

## 루트 디렉터리

```text
KnowledgeOS/
├── backend/         # REST API, filesystem adapter, search indexer
├── docker/          # Compose, container build, deployment config
├── docs/            # Architecture and implementation notes
├── frontend/        # Mobile-first PWA
└── knowledge/       # User-owned Markdown source of truth
```

운영 환경에서는 `knowledge/`를 별도 volume으로 마운트합니다. 앱 배포와 사용자 데이터 백업 주기를 분리하기 위해서입니다.

## `knowledge/` 구조

초기 권장 구조는 강제 스키마가 아니라 기본 프리셋입니다.

```text
knowledge/
├── _attachments/
├── _templates/
├── _trash/
├── ai/
├── daily/
├── inbox/
├── projects/
└── references/
```

- `_attachments/`: 문서 공통 첨부파일 저장소입니다.
- `_templates/`: daily note, project note 같은 Markdown template을 둡니다.
- `_trash/`: 삭제된 파일을 날짜별로 이동합니다.
- `ai/`: AI 도구, 모델, 활용법에 관한 사용자 지식 문서를 선택적으로 분류합니다. 애플리케이션 adapter가 아닙니다.
- `daily/`: `YYYY-MM-DD.md` 형식의 일일 노트입니다.
- `inbox/`: 모바일에서 빠르게 캡처한 임시 노트입니다.
- `projects/`: 장기 작업과 제품 설계를 프로젝트별로 둡니다.
- `references/`: 외부 문서 요약, 링크, 연구 자료를 둡니다.

## 첨부파일 정책

첨부파일은 두 가지 위치를 허용합니다.

```text
knowledge/_attachments/{yyyy}/{filename}
knowledge/projects/knowledgeos/_attachments/{filename}
```

공유 가능성이 큰 이미지는 전역 `_attachments/`에 두고, 특정 문서에 강하게 종속된 파일은 같은 디렉터리의 `_attachments/`에 둡니다.

Markdown에서는 상대 경로를 우선합니다.

```markdown
![architecture](./_attachments/architecture.png)
![screenshot](../../_attachments/2026/mobile-editor.png)
```

## 설정 파일

사용자 지식과 앱 설정은 분리합니다.

```text
KnowledgeOS/
├── .knowledgeos/
│   ├── config.json
│   ├── index.sqlite
│   └── locks/
└── knowledge/
```

- `.knowledgeos/config.json`: 앱 설정, root path, backup 정책입니다.
- `.knowledgeos/index.sqlite`: 검색, backlink, tag 캐시입니다.
- `.knowledgeos/locks/`: 파일 저장 충돌 방지용 lock 또는 lease입니다.

`.knowledgeos/`는 재생성 가능해야 합니다. 단, 사용자 preference처럼 복구가 불가능한 값은 export/import 경로를 별도로 제공합니다.

## 파일명 규칙

- 자동화가 많은 문서는 `kebab-case.md`를 권장합니다.
- 사람이 주로 다루는 문서는 한글 파일명을 허용합니다.
- 날짜 문서는 `YYYY-MM-DD.md`를 사용합니다.
- 디렉터리명은 복수형보다 도메인 의미를 우선합니다.

예시:

```text
projects/knowledgeos/api-design.md
daily/2026-07-11.md
references/filesystem-security.md
```

## 운영 고려사항

- `knowledge/`는 반드시 backup 대상입니다.
- `.knowledgeos/index.sqlite`는 backup 대상이 아니어도 됩니다.
- 대량 변경 전에는 Git snapshot 또는 tar snapshot을 남깁니다.
- 외부 편집기와 AI가 동시에 파일을 바꿀 수 있으므로 저장 API는 hash 기반 충돌 감지를 사용합니다.
- `_trash/`도 용량 제한과 보존 기간 정책이 필요합니다.

## 대안

- Flat folder: 구현은 단순하지만 프로젝트와 지식의 계층 구조 표현이 약합니다.
- DB 중심 구조: 검색과 모바일 상태 관리는 쉽지만 AI 직접 접근성과 복구성이 떨어집니다.
- Git repository 자체를 root로 사용: 감사 추적은 좋지만 모바일 UX와 충돌 해결이 복잡해집니다.
