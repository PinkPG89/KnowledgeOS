# Vault Policy

- 상태: Accepted
- 최종 갱신: 2026-07-12
- 적용 단계: A03 Root Containment and Symlink Policy

## 결론

KnowledgeOS MVP는 process 하나당 하나의 활성 Vault만 사용합니다. 사용자는 `KNOWLEDGEOS_KNOWLEDGE_ROOT`로 원하는 Markdown directory를 선택하고, 변경한 설정은 process 재시작 후 적용됩니다.

설정된 root 자체는 symlink일 수 있습니다. backend는 startup에서 root를 한 번 canonicalize해 실제 절대 directory로 고정합니다. Vault 내부 descendant symlink는 대상이 Vault 안에 있더라도 전부 거부합니다.

## 시작 계약

backend는 network socket을 열기 전에 다음 조건을 검증합니다.

- configured root가 존재합니다.
- root가 regular directory입니다.
- root를 canonicalize할 수 있습니다.
- root directory를 열어 목록을 읽을 수 있습니다.

검증에 실패하면 HTTP 요청을 받지 않고 typed startup error로 종료합니다.

## 경로 해석 계약

- `resolve_existing`: 존재하는 file 또는 directory의 모든 segment를 검사합니다.
- `resolve_parent_for_create`: 생성 대상의 기존 parent chain과 이미 존재하는 target을 검사합니다.
- 각 descendant는 `symlink_metadata`로 검사해 symlink follow 전에 거부합니다.
- 중간 segment는 실제 directory여야 합니다.
- 최종 canonical path와 create parent는 활성 Vault root 내부여야 합니다.

## 선택 이유

- Obsidian처럼 사용자가 지식 directory를 선택할 수 있어야 합니다.
- 단일 활성 Vault는 watcher, index, Git backup의 경계를 명확하게 유지합니다.
- root symlink를 한 번 허용하면 mount와 운영 경로를 유연하게 구성할 수 있습니다.
- descendant symlink를 모두 거부하면 read와 create operation의 정책이 단순하고 일관됩니다.

## 장점

- 잘못된 Vault 설정을 첫 요청 전에 발견합니다.
- API가 `knowledge/` 밖의 파일을 실수로 조작하지 않습니다.
- symlink가 내부를 가리키는지 외부를 가리키는지에 따른 예외가 없습니다.
- 실행 중 root가 암묵적으로 바뀌지 않습니다.

## 단점

- Vault 변경 시 backend를 재시작해야 합니다.
- 기존 Vault 내부 symlink는 UI에서 사용할 수 없습니다.
- 다중 Vault 통합 검색은 지원하지 않습니다.
- 검사와 실제 open 사이에서 적대적 local process가 filesystem을 교체하는 TOCTOU 가능성은 남습니다.

## 대안

- 실행 중 Vault 전환: 사용자 경험은 좋지만 watcher, index, 열린 editor 상태 전환이 복잡합니다.
- 다중 Vault 동시 활성화: 통합 검색은 가능하지만 API와 cache key에 Vault 식별자가 필요합니다.
- descendant symlink 허용: 유연하지만 containment와 race condition 방어가 어려워집니다.
- root symlink도 거부: 가장 엄격하지만 container mount와 운영 경로 유연성이 낮습니다.

## 운영 시 고려사항

- Vault path 변경은 configuration 변경과 process 재시작으로 수행합니다.
- startup log에는 configured path와 canonical path를 구분해 기록합니다.
- index와 watcher는 활성 Vault canonical root에만 연결합니다.
- local AI는 운영체제 권한으로 Vault에 직접 접근하므로 backend 정책을 우회할 수 있습니다.
- 적대적 local process까지 방어해야 하면 Linux `openat2` 또는 capability directory handle 기반 접근을 별도 보안 단계로 도입합니다.
