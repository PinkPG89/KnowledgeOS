# Obsidian Vault Migration Plan

- 상태: Initial Copy Completed · Production Cutover Pending
- 작성일: 2026-07-23
- Source: `/data/AppData/obsidian`
- Target: `/data/AppData/knowledgeos/vault`

## 결론

KnowledgeOS application source repository에 사용자 문서를 복사하지 않습니다. 운영 Vault는 `/data/AppData/knowledgeos/vault`로 분리하고 container에서 `/data/knowledge`로 mount합니다.

현재 Obsidian Vault는 9.2 MiB이며 Markdown 47개, 전체 파일 83개입니다. symlink와 world-writable file은 없지만 Obsidian plugin, Smart Environment, Syncthing, SilverBullet 인증 상태와 휴지통이 같은 root에 섞여 있습니다.

## 선택 이유

- application source와 사용자 데이터의 backup·release 주기를 분리합니다.
- hidden application metadata를 KnowledgeOS filesystem API 경계 밖으로 분리합니다.
- 원본 Vault를 그대로 보존해 전환 실패 시 즉시 rollback할 수 있습니다.
- 1차 import에서 상대 경로를 유지해 Markdown link와 embed 손상을 최소화합니다.

## Target Layout

```text
/data/AppData/knowledgeos/
├── import-archive/
└── vault/
    ├── _attachments/
    ├── _templates/
    ├── _trash/
    │   └── obsidian/
    ├── ai/
    ├── daily/
    ├── imports/
    │   └── obsidian/
    ├── inbox/
    ├── projects/
    └── references/
```

## Classification

### Active content

Obsidian의 모든 hidden path segment를 제외한 파일과 디렉터리를 `imports/obsidian/` 아래에 상대 경로 그대로 복제합니다. 기존 한글 디렉터리와 Excalidraw Markdown도 이 범위에 포함합니다.

1차 migration에서 디렉터리명을 임의로 영어 category로 바꾸지 않습니다. 이동 전에 link graph와 사용자 분류 의도를 확인하지 않으면 상대 link와 문맥을 손상할 수 있기 때문입니다.

### Legacy trash

Obsidian `.trash/`는 KnowledgeOS `_trash/obsidian/`으로 분리합니다. 자동 복원하거나 active content로 승격하지 않습니다.

### Application state

다음 항목은 active Vault에 넣지 않습니다.

- `.obsidian/`
- `.smart-env/`
- `.stfolder*`
- `.DS_Store`
- `.silverbullet.auth.json`

필요한 rollback metadata는 `/data/AppData/knowledgeos/import-archive/` 아래 timestamp snapshot으로 보존합니다. 이 archive는 KnowledgeOS API에 노출하지 않습니다.

## Migration Phases

### Phase 1: Initial Copy

SilverBullet을 실행한 상태에서 source를 수정하지 않는 초기 복제를 수행합니다.

```bash
./scripts/migrate-obsidian-vault.sh --dry-run
./scripts/migrate-obsidian-vault.sh --execute
./scripts/migrate-obsidian-vault.sh --verify
```

이 단계에서는 KnowledgeOS Compose mount를 변경하지 않습니다.

### Phase 2: Content Review

- Markdown 문서가 모두 열리는지 확인
- wikilink, embed, standard Markdown link 확인
- Excalidraw Markdown의 보존 여부 확인
- `.base`와 `.canvas`처럼 현재 UI가 표시하지 않는 format의 보존 위치 결정
- `imports/obsidian`에서 projects, references, ai 등으로 옮길 문서의 명시적 mapping 작성

### Phase 3: Write Freeze And Final Sync

1. SilverBullet, Obsidian, Syncthing과 AI writer를 일시 중지합니다.
2. source Vault snapshot을 생성합니다.
3. migration script를 다시 실행하고 checksum 검증합니다.
4. application state archive를 생성합니다.
5. Compose bind mount를 `/data/AppData/knowledgeos/vault`로 전환합니다.

### Phase 4: Cutover Validation

- root Tree API와 nested Unicode path 조회
- Markdown read와 hash 확인
- create, update, move, trash와 restore
- search rebuild와 결과 검증
- external change notification과 conflict flow
- mobile deep link와 editor 동작

## Rollback

KnowledgeOS write를 중지하고 Compose mount를 `/data/AppData/obsidian`으로 되돌립니다. source Vault는 migration 과정에서 수정하지 않으므로 SilverBullet도 기존 Compose로 다시 시작할 수 있습니다.

## Initial Copy Result

- `/data/AppData/knowledgeos/vault` target layout을 생성했습니다.
- active content 43개를 `imports/obsidian/`으로 복제했습니다.
- legacy trash 7개를 `_trash/obsidian/`으로 분리했습니다.
- active Vault의 hidden entry는 0개입니다.
- source와 target의 checksum mirror 검증이 통과했습니다.
- read-only 임시 Rust backend에서 health, root Tree, Unicode nested Tree와 Markdown Read API가 통과했습니다.
- source `/data/AppData/obsidian`은 수정하지 않았습니다.
- production Compose는 아직 source Vault를 사용합니다.

## Cutover Hold

SilverBullet과 KnowledgeOS가 서로 다른 Vault에 write하면 데이터가 분기됩니다. 따라서 다음 조건 전에는 `/data/docker-stacks/apps/knowledgeos/compose.yaml`의 bind mount를 target으로 변경하지 않습니다.

- B07 save state machine과 B08 draft recovery 완료
- D01–D03 external change와 conflict flow 완료
- SilverBullet write freeze 일정 확정
- final sync와 checksum 검증
- application state archive와 source snapshot 완료

## 데이터 안전 규칙

- migration script는 source를 수정하지 않습니다.
- migration script는 target 파일도 삭제하지 않습니다.
- 물리적 move보다 copy, checksum 검증, cutover 순서를 사용합니다.
- 원본 `/data/AppData/obsidian`은 SilverBullet rollback 기간 종료 전까지 유지합니다.
- archive에 인증 파일이 포함될 수 있으므로 권한을 `0700/0600` 수준으로 제한합니다.
