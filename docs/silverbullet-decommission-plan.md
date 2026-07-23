# SilverBullet Conditional Decommission Plan

- 상태: Planned · KnowledgeOS Success Gate 대기
- 작성일: 2026-07-23
- 원칙: SilverBullet container 제거와 공유 Markdown Vault 삭제는 서로 다른 작업입니다.

## Summary

KnowledgeOS가 개인 서버의 기본 지식 작업공간으로 안정화되면 SilverBullet을 단계적으로 퇴역합니다. 즉시 container와 데이터를 삭제하지 않고, 기능 충족 확인, 전환 rehearsal, write freeze, rollback 기간, container 제거 순서로 진행합니다.

SilverBullet과 KnowledgeOS는 `/data/AppData/obsidian`을 공유합니다. SilverBullet 전용 `/data/AppData/silverbullet`에는 현재 작은 SQLite index DB가 있으며 공유 Vault와 분리되어 있습니다.

## 제거 시작 게이트

아래 조건을 모두 충족하기 전에는 SilverBullet 중지 또는 제거를 시작하지 않습니다.

### Product Gate

- B06 iPhone Safari와 Android Chrome 실기기 검증 완료
- B07 save state machine 완료
- B08 browser draft recovery 완료
- A07 directory create, A08 move/rename, A09 trash/restore 완료
- C01–C05 search projection과 search UI 완료
- D01–D03 external change 감지, UI invalidation과 conflict review 완료
- SilverBullet에서 사용하던 핵심 Markdown 문서와 link가 KnowledgeOS에서 정상적으로 열림

### Safety Gate

- 공유 Vault 전체 backup과 restore rehearsal 완료
- Git snapshot 또는 동등한 version recovery 경로 검증
- KnowledgeOS write·conflict·external edit 시나리오 통합 테스트 통과
- SilverBullet 전용 syntax와 plugin 의존 문서 audit 완료
- rollback 시 SilverBullet이 동일 Vault를 다시 열 수 있음을 검증

### Operations Gate

- Nginx Proxy Manager를 통한 KnowledgeOS TLS와 접근 제어 검증
- container restart와 host reboot 후 자동 복구 검증
- 최소 14일 동안 KnowledgeOS를 주 사용 도구로 운영
- soak 기간에 데이터 손실, unresolved conflict와 blocker 등급 장애가 없음

## Phase 1: Inventory And Rehearsal

1. SilverBullet의 public domain, NPM Proxy Host와 인증 정책을 기록합니다.
2. SilverBullet-specific syntax, templates, scripts와 plugin 사용 여부를 조사합니다.
3. `/data/AppData/obsidian` snapshot을 생성하고 별도 위치에서 restore를 검증합니다.
4. `/data/AppData/silverbullet`을 별도 archive로 보관합니다.
5. 복구된 Vault를 대상으로 KnowledgeOS read, search와 link 동작을 검증합니다.

이 단계에서는 실행 중인 SilverBullet을 변경하지 않습니다.

## Phase 2: Cutover

1. 사용자와 AI의 SilverBullet write를 중지합니다.
2. 마지막 Vault snapshot을 생성합니다.
3. SilverBullet을 중지하되 Compose 파일과 전용 DB는 유지합니다.
4. NPM Proxy Host upstream을 KnowledgeOS `knowledgeos:8080`으로 전환합니다.
5. 모바일·데스크톱에서 read, write, search, conflict와 restore smoke test를 수행합니다.

SilverBullet localhost port `3010`과 Compose 정의는 rollback 기간 동안 유지할 수 있지만 외부 domain에서는 노출하지 않습니다.

## Phase 3: Rollback Window

- 최소 14일 동안 SilverBullet container를 stopped 상태로 유지합니다.
- `/data/AppData/obsidian`과 `/data/AppData/silverbullet`을 삭제하거나 이동하지 않습니다.
- blocker가 발생하면 KnowledgeOS write를 중단하고 snapshot을 생성한 뒤 SilverBullet을 다시 시작합니다.
- rollback 후 두 앱의 동시 write는 허용하지 않습니다.

## Phase 4: Container Decommission

rollback window가 문제없이 종료된 후 다음 순서로 수행합니다.

1. SilverBullet container가 중지됐는지 확인합니다.
2. NPM에 남은 SilverBullet Proxy Host와 certificate 참조를 확인합니다.
3. `/data/docker-stacks/apps/silverbullet/compose.yaml`로 `docker compose down`을 실행합니다.
4. SilverBullet image 제거 여부는 disk 정책에 따라 별도로 결정합니다.
5. Compose 정의는 운영 이력 archive로 보존합니다.
6. `/data/AppData/silverbullet`은 최소 90일 보존합니다.

## 데이터 삭제 정책

- `/data/AppData/obsidian`: SilverBullet 퇴역 작업에서 절대 삭제하지 않습니다.
- `/data/AppData/silverbullet`: 90일 보존 이후에도 핑크님의 명시적 승인 없이는 삭제하지 않습니다.
- backup archive: restore 검증과 보존 정책 확인 없이 삭제하지 않습니다.
- Docker image 삭제는 persistent data 삭제와 무관하며 별도 작업으로 취급합니다.

## Rollback

```bash
docker compose -f /data/docker-stacks/apps/silverbullet/compose.yaml up --detach
docker compose -f /data/docker-stacks/apps/silverbullet/compose.yaml ps
```

NPM upstream을 SilverBullet alias와 port로 되돌린 뒤, Vault 최근 변경과 SQLite index 상태를 확인합니다. KnowledgeOS와 SilverBullet을 동시에 write 가능 상태로 운영하지 않습니다.

## 현재 결론

현재 KnowledgeOS는 B06 실기기 검증과 B07 이후 작업이 남아 있으므로 SilverBullet 제거 게이트를 통과하지 못했습니다. SilverBullet은 계속 실행하며, 이 문서는 향후 제거 작업의 승인 체크리스트로 사용합니다.
