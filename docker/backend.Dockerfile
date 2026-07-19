# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.85

FROM rust:${RUST_VERSION}-bookworm AS builder

WORKDIR /build

COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src

RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime

ARG APP_UID=1000
ARG APP_GID=1000

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid "${APP_GID}" knowledgeos \
    && useradd --uid "${APP_UID}" --gid "${APP_GID}" --no-create-home --shell /usr/sbin/nologin knowledgeos \
    && mkdir -p /data/knowledge \
    && chown knowledgeos:knowledgeos /data/knowledge

COPY --from=builder /build/target/release/knowledgeos-backend /usr/local/bin/knowledgeos-backend

USER knowledgeos
WORKDIR /app

ENV KNOWLEDGEOS_BIND_ADDRESS=0.0.0.0:3000 \
    KNOWLEDGEOS_KNOWLEDGE_ROOT=/data/knowledge \
    KNOWLEDGEOS_LOG=knowledgeos_backend=info

EXPOSE 3000

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD ["curl", "--fail", "--silent", "--show-error", "http://127.0.0.1:3000/api/health"]

ENTRYPOINT ["/usr/local/bin/knowledgeos-backend"]
