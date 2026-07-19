# API Draft

- 상태: Draft
- 구현 상태: `/api/health`, `GET·PUT /api/files/{*path}`, `POST /api/files` Implemented, 나머지는 Planned
- 최종 갱신: 2026-07-19

## 원칙

API는 UI를 위한 얇은 파일 시스템 래퍼입니다. 원본 상태는 API가 아니라 `knowledge/` 디렉터리에 있습니다.

모든 path는 `knowledge/` 내부 상대 경로만 허용합니다.

상세 validation 규칙은 [Path Policy](path-policy.md)를 따릅니다.

```text
허용: projects/agent/README.md
금지: ../../etc/passwd
금지: /absolute/path/file.md
```

## Endpoints

공통 오류 형식:

```json
{
  "error": {
    "code": "conflict",
    "message": "File changed on disk",
    "details": {
      "path": "projects/agent.md"
    }
  }
}
```

파일 읽기 오류 code:

- `invalid_path`: 400
- `path_not_allowed`: 403
- `file_not_found`: 404
- `read_conflict`: 409
- `file_too_large`: 413
- `not_a_markdown_file`, `not_a_regular_file`, `invalid_utf8`: 422
- `internal_error`: 500

파일 생성 오류 code:

- `invalid_request`, `invalid_path`: 400
- `path_not_allowed`: 403
- `parent_not_found`: 404
- `file_already_exists`: 409
- `file_too_large`: 413
- `not_a_markdown_file`, `parent_not_directory`: 422
- `internal_error`: 500

파일 수정 오류 code:

- `invalid_request`, `invalid_path`, `invalid_base_hash`: 400
- `path_not_allowed`: 403
- `file_not_found`: 404
- `write_conflict`: 409
- `file_too_large`: 413
- `not_a_markdown_file`, `not_a_regular_file`, `invalid_utf8`: 422
- `internal_error`: 500

### Health

상태: Implemented

backend는 이 endpoint를 제공하기 전에 설정된 단일 활성 Vault의 존재, directory 여부, 접근 가능성을 검증합니다.

```http
GET /api/health
```

응답:

```json
{
  "status": "ok",
  "version": "0.1.0",
  "knowledge_root": "knowledge"
}
```

### Tree

상태: Planned

```http
GET /api/tree
```

쿼리:

- `path`: 생략하거나 빈 값이면 Vault root, 값이 있으면 해당 directory의 직계 children을 조회합니다.
- 응답 깊이는 항상 1이며 recursive `depth` parameter를 제공하지 않습니다.

응답 예시:

```json
{
  "path": "projects",
  "entries": [
    {
      "type": "directory",
      "path": "projects/agent",
      "name": "agent",
      "modified_at": "2026-07-19T12:00:00.000Z"
    },
    {
      "type": "file",
      "path": "projects/README.md",
      "name": "README.md",
      "size": 1200,
      "modified_at": "2026-07-19T12:00:00.000Z"
    }
  ]
}
```

상세 구현 계약은 [A10 Lazy Tree API Plan](a10-lazy-tree-plan.md)을 따릅니다.

### Read File

상태: Implemented

```http
GET /api/files/{*path}
```

응답:

```json
{
  "path": "projects/agent.md",
  "content": "# Agent\n",
  "hash": "sha256:...",
  "size": 8,
  "modified_at": "2026-07-11T12:00:00.000Z"
}
```

- `content`는 parsing하지 않은 UTF-8 Markdown 원문입니다.
- `hash`는 원문 byte의 lowercase SHA-256입니다.
- `size`는 UTF-8 byte 수입니다.
- `modified_at`은 UTC RFC3339 millisecond 형식입니다.
- 기본 최대 크기는 5 MiB이며 `KNOWLEDGEOS_MAX_MARKDOWN_BYTES`로 변경할 수 있습니다.
- 읽기 전후 metadata가 다르면 한 번 재시도하고 반복 변경 시 `409 read_conflict`를 반환합니다.

### Write File

상태: Implemented

```http
PUT /api/files/{*path}
```

요청:

```json
{
  "content": "# Agent\nUpdated content\n",
  "base_hash": "sha256:..."
}
```

`base_hash`가 현재 파일 hash와 다르면 `409 Conflict`를 반환합니다.

성공 시 `200 OK`:

```json
{
  "path": "projects/agent.md",
  "content": "# Agent\nUpdated content\n",
  "hash": "sha256:new...",
  "size": 24,
  "modified_at": "2026-07-19T12:05:00.000Z"
}
```

- `base_hash`는 `sha256:`와 64자리 lowercase hexadecimal 형식이어야 합니다.
- 현재 hash가 다르면 원본을 유지하고 `409 write_conflict`와 `current_hash`를 반환합니다.
- 같은 directory의 temp 파일에 write·flush·file `fsync`한 뒤 atomic rename합니다.
- backend 내부 동시 write는 직렬화하며 같은 base hash를 사용한 요청 중 하나만 성공합니다.
- 외부 local process가 최종 hash 확인과 rename 사이에 파일을 바꾸는 좁은 TOCTOU race는 별도 보안 강화 단계에서 다룹니다.
- parent directory `fsync`는 수행하지 않으므로 전원 장애까지 포함한 rename 영속성 강화는 후속 운영 단계입니다.

### Create File

상태: Implemented

```http
POST /api/files
```

요청:

```json
{
  "path": "projects/new-note.md",
  "content": "# New Note\n"
}
```

성공 시 `201 Created`:

```json
{
  "path": "projects/new-note.md",
  "content": "# New Note\n",
  "hash": "sha256:...",
  "size": 11,
  "modified_at": "2026-07-18T12:00:00.000Z"
}
```

- 부모 directory는 미리 존재해야 하며 자동 생성하지 않습니다.
- 기존 file 또는 directory를 덮어쓰지 않고 `409 file_already_exists`를 반환합니다.
- 생성 content는 읽기 API와 동일한 UTF-8 byte 제한을 적용합니다.
- write, flush, file `fsync`가 완료된 뒤 성공을 반환합니다.

### Create Directory

상태: Planned

```http
POST /api/directories
```

```json
{
  "path": "projects/new-project"
}
```

### Rename

상태: Planned

```http
POST /api/move
```

```json
{
  "from": "projects/old.md",
  "to": "projects/new.md"
}
```

### Delete

상태: Planned

```http
DELETE /api/files/{*path}
```

초기 정책은 실제 삭제가 아니라 `_trash/` 이동입니다.

응답:

```json
{
  "path": "projects/old.md",
  "trashed_path": "_trash/2026-07-12/projects-old.md"
}
```

### Search

상태: Planned

```http
GET /api/search?q=architecture
```

쿼리:

- `q`: 검색어입니다.
- `path_prefix`: 특정 디렉터리로 제한합니다.
- `limit`: 결과 수를 제한합니다.

응답:

```json
{
  "query": "architecture",
  "results": [
    {
      "path": "projects/knowledgeos/architecture.md",
      "title": "KnowledgeOS Architecture",
      "snippet": "Filesystem-first architecture...",
      "score": 0.92
    }
  ]
}
```

### Reindex

상태: Planned

```http
POST /api/index/rebuild
```

DB/index가 깨져도 이 endpoint로 복구 가능해야 합니다.

### Metadata

상태: Planned

```http
GET /api/metadata/{*path}
```

응답:

```json
{
  "path": "projects/agent.md",
  "title": "Agent",
  "tags": ["architecture", "knowledgeos"],
  "links": ["projects/knowledgeos/api-design.md"],
  "backlinks": ["daily/2026-07-11.md"]
}
```

### Git Backup

상태: Planned

```http
POST /api/git/commit
```

요청:

```json
{
  "message": "Manual backup"
}
```

MVP에서는 내부 관리자 전용으로 제한합니다.

## 보안 기준

- 모든 path는 lexical validation 후 `knowledge/` 내부 containment를 확인합니다.
- dot으로 시작하는 hidden segment는 API에서 허용하지 않습니다.
- symlink는 MVP에서 허용하지 않습니다.
- 파일 크기 제한과 확장자 allowlist를 둡니다.
- Git commit, reindex 같은 운영 endpoint는 관리자 권한이 필요합니다.
