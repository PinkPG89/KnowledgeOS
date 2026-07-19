# Docker Development Runtime

- 상태: Implemented
- 최종 갱신: 2026-07-19

## 목적

Docker 구성은 Rust backend binary, Linux filesystem 정책, non-root 권한, Vault bind mount를 개발 환경에서 검증합니다. `knowledge/`는 image에 복사하지 않고 host directory를 `/data/knowledge`로 mount하므로 Markdown Source of Truth 원칙을 유지합니다.

## 실행

repository root에서 host 사용자 UID/GID를 build argument로 전달합니다.

```bash
HOST_UID=$(id -u) HOST_GID=$(id -g) docker compose up --build --detach
docker compose ps
```

기본 API 주소는 `http://127.0.0.1:3000`입니다. 다른 host port가 필요하면 `KNOWLEDGEOS_PORT`를 지정합니다.

```bash
KNOWLEDGEOS_PORT=8080 HOST_UID=$(id -u) HOST_GID=$(id -g) docker compose up --build --detach
```

## Smoke Test

container가 healthy 상태가 된 뒤 실행합니다.

```bash
./docker/smoke.sh
```

smoke test는 Health, Markdown Create, Read, Atomic Update, host bind mount 반영을 순서대로 검증하며 임시 Markdown 파일을 종료 시 정리합니다.

## 종료

```bash
docker compose down
```

## 운영 고려사항

- runtime image는 non-root 사용자로 실행하고 Linux capability를 모두 제거합니다.
- container root filesystem은 read-only이며 Vault bind mount와 제한된 `/tmp`만 쓸 수 있습니다.
- host Vault가 다른 UID/GID 소유라면 build 시 `HOST_UID`, `HOST_GID`를 해당 소유자와 맞춰야 합니다.
- 현재 Compose는 local development용이며 TLS, 인증, reverse proxy, backup 정책은 포함하지 않습니다.
