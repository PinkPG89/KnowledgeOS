# Data Model

## Source of Truth

원본 데이터는 오직 Markdown 파일입니다.

```text
knowledge/**/*.md
```

## Markdown 문서 규약

초기에는 일반 Markdown을 허용합니다. 선택적으로 YAML frontmatter를 지원합니다.

```markdown
---
title: MCP Server Design
tags:
  - mcp
  - ai
status: draft
---

# MCP Server Design

본문...
```

필수 frontmatter는 두지 않습니다. AI와 외부 편집기가 쉽게 작성할 수 있어야 하기 때문입니다.

권장 frontmatter:

```yaml
title: MCP Server Design
tags:
  - mcp
  - ai
status: draft
created_at: 2026-07-11
updated_at: 2026-07-11
```

`updated_at`은 신뢰 가능한 원본이 아닙니다. 파일 시스템의 `mtime`과 Git history가 더 우선합니다.

## 경로 규칙

- 디렉터리는 주제 기반으로 구성합니다.
- 파일명은 소문자 kebab-case를 권장합니다.
- 한글 파일명은 허용하되, AI/CLI 자동화가 많은 영역은 영문 파일명을 권장합니다.
- 첨부파일은 문서 근처 또는 `_attachments/`에 저장합니다.

예시:

```text
knowledge/
├── ai/
│   ├── mcp-server-design.md
│   └── pydantic-ai-patterns.md
├── projects/
│   └── knowledgeos/
│       ├── README.md
│       └── architecture.md
├── daily/
│   └── 2026-07-11.md
└── _attachments/
```

## 캐시 DB

SQLite를 사용할 경우 테이블은 원본이 아니라 캐시입니다.

예시:

```text
documents
- path
- title
- hash
- modified_at
- indexed_at
- frontmatter_json

tags
- path
- tag

links
- source_path
- target_ref
- target_path
- resolved

search_index
- path
- content_fragment
```

중요한 제약:

- DB의 `content`를 원본으로 사용하지 않습니다.
- DB는 삭제 후 전체 재생성할 수 있어야 합니다.
- 외부 파일 변경을 감지하면 해당 path의 캐시를 무효화합니다.

## Conflict 정책

초기 버전은 단순한 optimistic concurrency를 사용합니다.

```text
파일 읽기 시 hash 반환
저장 시 base_hash 제출
현재 hash와 다르면 저장 거부
```

AI가 직접 파일을 수정한 경우 UI 저장 시 충돌을 감지할 수 있습니다.

## Link 규칙

초기에는 일반 Markdown link와 wiki link를 모두 인식합니다.

```markdown
[MCP](../ai/mcp.md)
[[pydantic-ai-patterns]]
```

Markdown link는 명시적 경로로 해석합니다. Wiki link는 파일명 stem 기준으로 검색하고, 중복될 경우 unresolved 상태로 둡니다.

## Tag 규칙

태그는 frontmatter와 inline hashtag를 모두 인식합니다.

```markdown
---
tags:
  - ai
---

본문의 #mcp 태그
```

저장 시 자동으로 본문을 재작성하지 않습니다. tag index는 파일 내용을 읽어서 재생성합니다.

## 삭제 정책

MVP에서는 실제 삭제 대신 `.trash/` 이동을 기본으로 합니다.

```text
knowledge/.trash/2026-07-11/projects-old.md
```

이유:

- AI 실수 복구
- 모바일 조작 실수 복구
- Git 백업 전 삭제 위험 완화
