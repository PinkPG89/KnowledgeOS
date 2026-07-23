# syntax=docker/dockerfile:1

FROM node:20.19.4-alpine AS builder

WORKDIR /build

# dependency layer를 source code와 분리해 package lock이 같으면 재사용합니다.
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci

COPY frontend ./
RUN npm run build

FROM nginxinc/nginx-unprivileged:1.28.0-alpine AS runtime

COPY --chown=101:101 --chmod=0444 docker/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder --chown=101:101 /build/dist /usr/share/nginx/html

EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD ["wget", "--quiet", "--output-document=/dev/null", "http://127.0.0.1:8080/healthz"]
