# C03 Incremental Index Sync

- 상태: Completed
- 완료일: 2026-07-26

## Summary

Markdown 원본 쓰기와 SQLite FTS5 projection 갱신을 연결하고, application startup마다
Vault 전체를 source of truth로 reconciliation합니다. Create와 update는 성공한 원본
snapshot을 즉시 upsert하며 delete와 move는 후속 filesystem use case가 호출할 수 있는
원자적 index contract를 제공합니다.

검색 index는 계속 재생성 가능한 cache입니다. Index upsert가 실패해도 이미 성공한 Markdown
create/update 응답을 실패로 바꾸거나 원본 파일을 rollback하지 않습니다. 해당 drift는 다음
startup reconciliation 또는 명시적 reconciliation으로 복구합니다.

## 선택 이유

- 기존 `MarkdownWriter`를 수정하지 않고 application service에서 원본 쓰기와 projection
  갱신을 조립했습니다. 원본 filesystem adapter와 SQLite adapter 사이의 직접 결합을 피하고
  기존 파일 안전성 test를 그대로 유지하기 위해서입니다.
- API handler가 사용하는 writer contract는 유지했습니다. 사용자 작성 중인 API 파일과
  충돌하지 않으면서 create/update 경로에 incremental sync를 적용할 수 있습니다.
- Startup full reconciliation을 baseline으로 선택했습니다. D01 filesystem watcher가
  구현되기 전에도 AI, Git과 외부 editor가 만든 변경을 backend restart 시 확실히 복구할
  수 있기 때문입니다.
- Documents row와 FTS row의 실제 title, body, tags, hash, modified time과 frontmatter를
  비교합니다. Hash만 비교하면 손상되거나 누락된 FTS row를 복구하지 못합니다.

## Sync Contract

```text
Markdown create/update
  → durable source write
  → tolerant parser
  → documents + FTS transaction
  → index 실패 시 error log, source write 성공 유지

Application startup
  → Vault recursive scan
  → stable Markdown snapshot read
  → tolerant parser
  → source/index exact comparison
  → insert/update/delete를 단일 transaction으로 반영
```

- `upsert_document`: path가 없으면 insert, hash가 바뀌면 update, 동일 projection이면 no-op
- `delete_path`: documents와 FTS row를 한 transaction에서 제거
- `move_document`: source 삭제와 destination upsert를 한 transaction에서 수행
- `reconcile`: filesystem에 없는 stale row 삭제, 누락 row insert, 내용 또는 FTS drift update
- `_trash/`: 검색 대상에서 제외하고 기존 projection도 제거
- hidden path, symlink, non-Markdown file: scan하지 않음
- invalid UTF-8, oversized 또는 읽기 충돌 문서: warning과 skipped 집계 후 stale projection 제거
- malformed frontmatter: C02 tolerant parser 결과를 정상 index하고 진단 수만 집계

## 장점

- 외부 filesystem client가 만든 index drift를 backend restart만으로 복구합니다.
- documents와 FTS 변경이 transaction 단위로 함께 반영되어 부분 projection을 줄입니다.
- 검색 장애와 source Markdown 내구성을 분리해 index가 파일 CRUD의 단일 장애점이 되지
  않습니다.
- `SearchIndexSynchronizer`가 HTTP와 독립적이어서 A08 move, A09 trash와 D01 watcher에서
  같은 contract를 재사용할 수 있습니다.

## 단점

- Startup마다 Vault 전체 파일을 읽고 parse하므로 Vault 크기에 비례해 시작 시간이
  증가합니다.
- C01 schema에는 parser version이 없습니다. 같은 content hash에 parser 규칙만 바뀌는
  release에서는 schema version을 올리거나 명시적 rebuild가 필요합니다.
- Invalid UTF-8 또는 size limit 초과 문서는 검색 결과에서 제거되며 파일 API의 개별 오류는
  그대로 유지됩니다.
- Link projection은 C02 model에 남아 있지만 C01 search schema에는 link table이 없어
  C03에서 영속화하지 않습니다. Backlink 기능을 추가할 때 schema migration이 필요합니다.

## 대안

- 파일 쓰기 transaction과 SQLite transaction을 하나의 강한 transaction처럼 취급:
  서로 다른 storage engine이라 원자성을 보장할 수 없고 index 장애가 원본 저장을 막으므로
  제외했습니다.
- Startup reconciliation 없이 incremental hook만 사용: 외부 AI와 editor의 직접
  filesystem 변경을 놓치므로 filesystem-first 원칙에 맞지 않습니다.
- 매 startup full rebuild: 구현은 단순하지만 unchanged 문서까지 다시 쓰고 FTS를 비우는
  시간이 길어 incremental 비교 방식을 선택했습니다.
- D01 watcher를 C03에 포함: 실시간성은 좋아지지만 event debounce와 atomic replace
  normalization이라는 별도 위험을 섞으므로 후속 단위로 유지했습니다.

## 자동화 검증

- Create 후 title, body와 tags FTS projection insert
- Update 후 이전 FTS term 제거와 최신 projection 반영
- 외부 create, update와 delete drift reconciliation
- Documents metadata 손상과 누락된 FTS row reconciliation
- `_trash/` 제외와 invalid UTF-8 skipped 처리
- Move source 삭제와 destination projection insert
- Delete projection 제거
- SQLite index 장애가 성공한 Markdown create와 원본 content를 rollback하지 않음
- 전체 backend test와 strict Clippy 통과

## 운영 시 고려사항

- Startup log의 `discovered`, `skipped`, `malformed_frontmatter`, `inserted`, `updated`,
  `unchanged`, `deleted`를 관찰해야 합니다.
- `skipped`가 증가하면 invalid UTF-8, file size, 권한 또는 read race를 원본 Vault에서
  조사해야 합니다.
- 대형 Vault에서 startup scan 시간이 병목이 되면 manifest 기반 빠른 비교를 검토할 수
  있지만 manifest도 재생성 가능한 cache로 유지해야 합니다.
- 여러 backend process가 같은 state root를 공유하는 구성은 지원하지 않습니다. SQLite
  busy timeout은 일시적 경합만 완화하며 process 단위 single-writer 운영이 기본입니다.

## 다음 단계

C04에서 FTS query escaping, path prefix, limit, snippet과 score를 갖는 Search API를
추가합니다. D01에서는 외부 변경을 restart 없이 같은 synchronizer contract로 전달합니다.
