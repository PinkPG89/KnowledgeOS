# API Draft

- 상태: Draft
- 구현 상태: `/api/health`만 Implemented, 나머지는 Planned
- 최종 갱신: 2026-07-12

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

### Health

상태: Implemented

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

- `path`: 특정 디렉터리부터 조회합니다.
- `depth`: tree 깊이를 제한합니다.

응답 예시:

```json
{
  "root": "knowledge",
  "nodes": [
    {
      "type": "directory",
      "path": "projects",
      "name": "projects",
      "children": [
        {
          "type": "file",
          "path": "projects/agent.md",
          "name": "agent.md",
          "size": 1200,
          "modified_at": "2026-07-11T12:00:00Z"
        }
      ]
    }
  ]
}
```

### Read File

상태: Planned

```http
GET /api/files/{*path}
```

응답:

```json
{
  "path": "projects/agent.md",
  "content": "# Agent\n",
  "hash": "sha256:...",
  "modified_at": "2026-07-11T12:00:00Z"
}
```

### Write File

상태: Planned

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

응답:

```json
{
  "path": "projects/agent.md",
  "hash": "sha256:new...",
  "modified_at": "2026-07-11T12:05:00Z"
}
```

### Create File

상태: Planned

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
