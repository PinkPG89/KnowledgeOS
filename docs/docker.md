# Docker Runtime

- 상태: Implemented
- 최종 갱신: 2026-07-23

## 목적

Docker 구성은 Rust backend binary, Vue production PWA, Linux filesystem 정책, non-root 권한과 Vault bind mount를 검증합니다. Markdown Vault는 image에 복사하지 않고 host directory를 `/data/knowledge`로 mount하므로 Source of Truth 원칙을 유지합니다.

개발용 repository Compose와 개인 서버 운영 Compose를 구분합니다.

- 개발용 backend-only Compose: `/data/Ai-Workspace/KnowledgeOS/compose.yaml`
- 개인 서버 운영 Compose: `/data/docker-stacks/apps/knowledgeos/compose.yaml`
- 개인 서버 shared Vault: `/data/AppData/obsidian`
- 운영 UI: `http://127.0.0.1:3030`
- Nginx Proxy Manager upstream: `knowledgeos:8080`

## 실행

개인 서버에서는 `/data/docker-stacks`의 Compose를 기준으로 frontend와 backend를 함께 실행합니다.

```bash
docker compose -f /data/docker-stacks/apps/knowledgeos/compose.yaml up --build --detach
docker compose -f /data/docker-stacks/apps/knowledgeos/compose.yaml ps
```

다른 localhost UI port가 필요하면 `KNOWLEDGEOS_PORT`를 지정합니다.

```bash
KNOWLEDGEOS_PORT=3035 docker compose -f /data/docker-stacks/apps/knowledgeos/compose.yaml up --build --detach
```

## Smoke Test

container가 healthy 상태가 된 뒤 UI, same-origin API와 Tree API를 확인합니다.

```bash
curl --fail http://127.0.0.1:3030/healthz
curl --fail http://127.0.0.1:3030/api/health
curl --fail http://127.0.0.1:3030/api/tree
```

기존 `./docker/smoke.sh`는 repository 개발용 backend Compose의 Create, Read, Atomic Update와 bind mount 동작을 검증할 때 사용합니다.

## 종료

```bash
docker compose -f /data/docker-stacks/apps/knowledgeos/compose.yaml down
```

## 운영 고려사항

- runtime image는 non-root 사용자로 실행하고 Linux capability를 모두 제거합니다.
- container root filesystem은 read-only이며 Vault bind mount와 제한된 `/tmp`만 쓸 수 있습니다.
- Rust backend는 internal network에만 연결하며 host port를 노출하지 않습니다.
- frontend만 external `frontend` network에 연결해 Nginx Proxy Manager가 접근합니다.
- Many Notes와 SilverBullet이 같은 Vault를 사용하므로 external watcher와 conflict UX 완성 전에는 동시 편집을 피합니다.
- TLS와 외부 인증은 Nginx Proxy Manager에서 별도로 구성합니다.
