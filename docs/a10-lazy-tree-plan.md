# A10 Lazy Tree API Plan

- 상태: Completed
- 최종 갱신: 2026-07-19

## Summary

- `GET /api/tree`는 활성 Vault root의 직계 children만 반환합니다.
- `GET /api/tree?path=projects`는 검증된 하위 directory의 직계 children만 반환합니다.
- client가 folder를 expand할 때마다 한 번 호출하는 lazy loading contract이며 nested tree와 `depth` parameter는 제공하지 않습니다.

## API Contract

root 요청은 `path`를 생략하거나 빈 문자열로 전달합니다.

```http
GET /api/tree?path=projects
```

```json
{
  "path": "projects",
  "entries": [
    {
      "type": "directory",
      "name": "agent",
      "path": "projects/agent",
      "modified_at": "2026-07-19T12:00:00.000Z"
    },
    {
      "type": "file",
      "name": "README.md",
      "path": "projects/README.md",
      "size": 1200,
      "modified_at": "2026-07-19T12:00:00.000Z"
    }
  ]
}
```

- `type`은 `directory` 또는 `file`입니다.
- `size`는 Markdown file entry에만 존재하며 UTF-8 검증이나 content read를 수행하지 않은 filesystem byte size입니다.
- `modified_at`은 UTC RFC3339 millisecond 형식입니다.
- directory를 먼저 배치하고 각 group 안에서 원본 UTF-8 name의 Rust 문자열 오름차순으로 정렬합니다.

## Filesystem Rules

- root는 `Option<CanonicalPath>`의 `None`, 하위 directory는 `Some(CanonicalPath)`로 표현해 빈 `CanonicalPath`를 도입하지 않습니다.
- 하위 directory는 `VaultRoot.resolve_existing`으로 containment와 descendant symlink 정책을 적용합니다.
- 직접 child는 `symlink_metadata`로 검사하며 symlink, dot-hidden entry, special file, lowercase `.md`가 아닌 file을 응답에서 제외합니다.
- `_trash`처럼 underscore로 시작하는 directory는 기존 path policy에 따라 표시합니다.
- scan 중 child가 사라진 `NotFound` race는 해당 entry만 제외하고, 그 외 directory read 또는 metadata 오류는 전체 요청을 실패시킵니다.
- filesystem scan은 `tokio::task::spawn_blocking`에서 실행합니다.

## Error Contract

- `invalid_path` → 400
- `path_not_allowed` → 403
- `directory_not_found` → 404
- `not_a_directory` → 422
- 내부 directory read, metadata, task 오류 → 500 `internal_error`
- public 오류에 configured/canonical absolute filesystem path를 포함하지 않습니다.

## Test Plan

- root 및 nested directory의 depth-1 listing
- directory-first, UTF-8 name stable ordering
- file `size`와 directory/file RFC3339 millisecond timestamp
- absent path와 empty path의 root contract
- missing directory 404와 file target 422
- traversal, hidden query path, descendant directory symlink 거부
- child symlink, hidden entry, non-Markdown file, special file 제외
- `_trash`, 한글, 공백 포함 directory와 lowercase `.md` 허용
- scan 중 사라진 child 제외와 그 외 I/O 실패 처리
- 기존 A02–A06, health, Docker smoke 회귀 유지

## Assumptions

- A10은 Markdown navigation 전용이며 attachment listing은 후속 data model 단계에서 별도 확장합니다.
- pagination, recursive depth, hash, content, title/frontmatter parsing은 포함하지 않습니다.
- directory listing은 filesystem transaction snapshot이 아니며 요청 시점의 best-effort depth-1 view입니다.
- A10 완료 후 B01 Vue 3 PWA skeleton을 시작합니다.

## Implementation Result

- domain listing model, filesystem `TreeReader`, Axum adapter를 분리했습니다.
- root는 빈 문자열 응답으로 표현하고 내부에서는 `Option<CanonicalPath>::None`을 유지합니다.
- scan 중 사라진 child는 제외하고, 나머지 metadata와 directory read 오류는 500으로 처리합니다.
- unit 3개와 API contract 4개를 추가해 depth 1, 정렬, 필터, Unicode query와 오류 계약을 검증했습니다.
