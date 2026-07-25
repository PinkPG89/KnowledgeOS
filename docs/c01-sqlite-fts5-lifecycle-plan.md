# C01 SQLite FTS5 Index Schema And Lifecycle

- 상태: Implementation Complete
- 완료일: 2026-07-24

## Summary

검색 projection을 Vault 외부 application state에 저장하는 SQLite FTS5 schema와 create, destroy, rebuild lifecycle을 구현했습니다. Index는 Markdown source of truth가 아니며 삭제하거나 schema가 바뀌면 빈 최신 schema로 다시 만들 수 있습니다.

C01은 schema lifecycle만 담당합니다. Vault Markdown을 읽어 document projection을 채우는 full population은 C02 parser와 C03 incremental sync에서 연결합니다.

## Storage 경계

```text
KNOWLEDGEOS_KNOWLEDGE_ROOT=/data/knowledge
KNOWLEDGEOS_STATE_ROOT=/data/state

/data/knowledge/         Markdown source of truth
/data/state/index.sqlite rebuildable search projection
```

기본 개발 경로는 `../.knowledgeos/index.sqlite`입니다. Production Docker는 host `/data/AppData/knowledgeos/state`를 container `/data/state`로 별도 mount합니다. SilverBullet과 공유 중인 `/data/AppData/obsidian`에는 KnowledgeOS index를 생성하지 않습니다.

## 선택 이유

- Vault와 application metadata의 backup, migration과 권한 경계를 분리합니다.
- SQLite는 개인 서버에서 별도 search daemon 없이 단일 파일로 운영할 수 있습니다.
- FTS5는 title, body와 tag full-text projection을 지원합니다.
- `rusqlite 0.34` bundled SQLite를 사용해 runtime system SQLite version과 FTS5 build option 차이를 제거합니다.
- Rust 1.85 dependency resolution을 유지합니다.

## Schema Version 1

SQLite `PRAGMA user_version = 1`을 schema version source로 사용합니다.

```text
documents
├─ path TEXT PRIMARY KEY
├─ title TEXT
├─ body TEXT
├─ content_hash TEXT
├─ modified_at TEXT
├─ indexed_at TEXT
└─ frontmatter_json TEXT NULL

search_documents USING fts5
├─ path UNINDEXED
├─ title
├─ body
└─ tags
```

FTS tokenizer는 `unicode61 remove_diacritics 2`를 사용합니다. Korean 형태소 분석이나 language-specific ranking은 C04 search 품질 단계의 별도 결정입니다.

## Lifecycle

### Create

- `KNOWLEDGEOS_STATE_ROOT`가 없으면 directory를 생성합니다.
- 설정 root 자체의 symlink는 canonical directory로 고정할 수 있습니다.
- `index.sqlite` 또는 sidecar가 symlink면 거부합니다.
- WAL, foreign keys, 2초 busy timeout과 normal synchronous policy를 적용합니다.

### Open

- schema version 1과 필수 `documents`, FTS5 table을 검증합니다.
- version이 다르거나 같은 version의 schema가 불완전하면 기존 projection을 삭제하고 재생성합니다.
- database corruption은 application startup을 막지 않고 search degraded mode로 기록합니다.

### Destroy

- `index.sqlite`, `index.sqlite-wal`, `index.sqlite-shm`만 제거합니다.
- Markdown Vault에는 접근하지 않습니다.
- 삭제 대상 symlink는 제거하지 않고 typed error를 반환합니다.

### Rebuild

- 기존 DB와 sidecar를 제거합니다.
- 최신 빈 schema를 transaction으로 생성합니다.
- DB가 삭제됐거나 손상된 bytes로 교체된 상태에서도 schema를 복원합니다.
- C02/C03 이전에는 Markdown document population을 수행하지 않습니다.

## 장애 정책

`AppState`는 `Option<SearchIndex>`를 보관합니다. Index 초기화가 실패하면 structured error log를 남기고 Markdown read, create, update와 tree API를 계속 제공합니다. Vault 자체 초기화 실패는 기존처럼 startup을 중단합니다.

이 분리는 “검색 장애가 원본 파일 작업을 막지 않는다”는 filesystem-first 원칙을 유지합니다.

## 장점

- 별도 Typesense, Meilisearch service와 network 운영 비용이 없습니다.
- DB 삭제와 schema upgrade가 사용자 Markdown을 변경하지 않습니다.
- bundled SQLite로 container와 개발 환경의 FTS5 차이를 줄입니다.
- lifecycle lock으로 같은 process 내부 destroy와 rebuild를 직렬화합니다.

## 단점

- Bundled SQLite compile로 backend build 시간이 늘어납니다.
- WAL sidecar를 포함한 state directory 쓰기 권한이 필요합니다.
- Process가 여러 개면 Rust mutex를 공유하지 않으므로 SQLite locking에 의존합니다.
- C01만으로는 실제 문서 검색 결과가 생성되지 않습니다.

## 대안

- System SQLite: binary는 작지만 배포 host마다 SQLite와 FTS5 build option이 달라질 수 있습니다.
- Tantivy: Rust-native ranking과 확장성은 좋지만 MVP schema와 index lifecycle 복잡도가 큽니다.
- Meilisearch/Typesense: 검색 기능은 풍부하지만 개인 서버에 별도 service와 backup 정책이 추가됩니다.
- `ripgrep`: source 직접 검색에는 단순하지만 ranking, tag, backlink projection 확장이 어렵습니다.

## 실제 보장 범위

- 한 backend process의 lifecycle operation은 mutex로 직렬화합니다.
- SQLite WAL과 busy timeout은 일반적인 lock 경합을 처리하지만 다중 process coordination을 완전히 보장하지 않습니다.
- Symlink 방어는 startup과 lifecycle 호출 시점 검사이며 적대적인 local process의 모든 TOCTOU를 방어하지 않습니다.
- Index는 삭제 가능한 cache이므로 database file 자체를 backup source로 사용하지 않습니다.

## 자동화 검증

- state directory와 schema version 1 생성
- Unicode content FTS5 match
- schema version mismatch 폐기·재생성
- 현재 version의 불완전 schema 폐기·재생성
- DB 삭제 후 rebuild
- 손상된 DB bytes 제거 후 rebuild
- file state root와 descendant DB symlink 거부
- index state 장애 시 application degraded startup
- 정상 state root에서 AppState index 제공
- 기존 backend unit와 API contract 회귀

## 운영 시 고려사항

- Docker deployment는 `/data/state` writable bind mount가 반드시 필요합니다.
- State directory는 Vault backup 필수 대상이 아니지만 ownership과 free space monitoring은 필요합니다.
- Rebuild 중 search는 unavailable 상태가 되며 C04에서 API status와 retry 정책을 추가합니다.
- Schema 변경은 `SCHEMA_VERSION` 증가와 destructive projection rebuild를 함께 수행합니다.

## 다음 단계

C02에서 Markdown title, body, frontmatter tags, links와 content hash를 추출하는 tolerant projection parser를 구현합니다. Malformed frontmatter는 index metadata 품질만 낮추고 file CRUD와 raw body indexing을 막지 않아야 합니다.
