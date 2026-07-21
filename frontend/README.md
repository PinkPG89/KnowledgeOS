# KnowledgeOS Frontend

Vue 3, TypeScript, Vue Router와 Pinia로 구성한 모바일 우선 PWA입니다.

## Requirements

- Node.js `^20.19.0 || >=22.12.0`
- npm lockfile 기반 설치

## Commands

```sh
npm ci
npm run dev
npm run validate
```

- `npm run dev`: Vite development server
- `npm run test:unit`: Vitest component와 store test
- `npm run build`: type check와 production PWA build
- `npm run verify:pwa`: manifest, service worker와 install icon 확인
- `npm run validate`: lint, format check, type check, test와 build 전체 검증

VS Code에서는 Vue - Official 확장을 사용합니다.
