# Git Versioning and Backup Policy

- 상태: Accepted
- 최종 갱신: 2026-07-19

## 결론

KnowledgeOS 운영에 Git hosting service는 필수가 아닙니다. 다만 Git commit만으로는 같은 disk 장애를 복구할 수 없으므로 다음 세 계층을 분리합니다.

1. Vault의 자동 Git version snapshot
2. 물리적으로 분리된 bare repository로 복제
3. 선택적인 encrypted offsite backup

## Repository 경계

### Application repository

- 대상: KnowledgeOS backend, frontend, Docker와 설계 문서
- 예시: `/srv/knowledgeos/app`
- 배포와 source code 변경 이력을 관리합니다.
- 사용자 Markdown 데이터는 포함하지 않습니다.

### Vault repository

- 대상: 사용자의 Markdown Vault와 `_trash/`
- 예시: `/srv/knowledgeos/vault`
- KnowledgeOS가 자동 version snapshot을 생성할 repository입니다.
- application repository와 commit 주기, 보존 정책, 접근 권한을 분리합니다.

## 운영 흐름

```text
KnowledgeOS / AI / editor writes Markdown
  -> local Vault Git commit
  -> push to bare repository on another disk, NAS, or server
  -> optional encrypted offsite snapshot
```

- local commit은 변경 추적, diff, rollback을 제공합니다.
- 별도 장치의 bare repository는 server disk 장애에 대비한 복제본입니다.
- offsite backup은 화재, 도난, ransomware처럼 동일 장소의 장애에 대비합니다.
- push 실패가 Markdown 저장 자체를 실패시키면 안 됩니다. 실패를 기록하고 재시도 가능한 운영 상태로 노출합니다.

## 포함과 제외

기본 포함 대상:

- lowercase `.md` 파일
- 사용자가 보존하려는 attachment
- `_trash/`
- Vault에 함께 두기로 결정한 사용자 설정

기본 제외 대상:

- `.git/`
- 임시 write 파일
- lock 파일
- `.knowledgeos/index.sqlite` 같은 재생성 가능한 index와 cache
- runtime log

실제 ignore 규칙은 Vault 생성 기능을 구현할 때 별도 template으로 확정합니다.

## Git Service 선택 기준

bare repository는 SSH와 filesystem 권한만으로도 운영할 수 있으므로 개인 server MVP에는 충분합니다.

Forgejo 또는 Gitea는 다음 요구가 생길 때만 도입합니다.

- browser 기반 diff와 history 확인
- 여러 사용자와 repository 권한 관리
- issue, pull request, webhook
- SSH key와 repository lifecycle을 UI에서 관리

Git service를 추가하면 UI 편의성은 높아지지만 database, upgrade, backup, 인증과 보안 패치 대상이 늘어납니다.

## 복구 검증

- 정기적으로 bare repository에서 새 directory로 clone하여 Markdown을 읽을 수 있는지 확인합니다.
- encrypted offsite backup을 사용하면 restore test와 key 보관 절차를 함께 운영합니다.
- Git repository의 무결성 검사만으로 실제 복구 가능성이 증명되지는 않으므로 파일 열기까지 검증합니다.

## 단계별 구현

1. MVP에서는 수동 version snapshot을 제공합니다.
2. 안정화 단계에서 변경 묶음별 자동 commit과 실패 재시도를 추가합니다.
3. 별도 bare repository push와 상태 관측을 추가합니다.
4. 필요하면 restic 같은 도구로 encrypted offsite backup을 구성합니다.
5. 다중 사용자 운영 요구가 생길 때만 Forgejo 또는 Gitea를 검토합니다.
