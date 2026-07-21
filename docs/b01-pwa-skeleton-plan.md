# B01 Vue 3 PWA Skeleton Plan

- 상태: Completed
- 최종 갱신: 2026-07-21

## Summary

- `frontend/`에 Vue 3, TypeScript, Vite 기반 SPA를 구성합니다.
- Vue Router와 Pinia를 애플리케이션 진입점에서 조립합니다.
- `vite-plugin-pwa`의 `generateSW` 전략으로 app shell을 precache합니다.
- B01은 실제 Tree API 연동이나 editor를 포함하지 않고 설치 가능한 shell과 실행 기반만 만듭니다.

## Technology Decisions

### Framework and build

- Vue 3 Composition API와 `<script setup lang="ts">`를 사용합니다.
- Vite를 development server와 production bundler로 사용합니다.
- TypeScript는 strict mode와 project references를 유지합니다.
- 지원 Node.js 범위는 `^20.19.0 || >=22.12.0`으로 고정합니다.

현재 server의 Node.js 20.19.4는 이 범위를 충족합니다.

### Routing and state

- Vue Router의 `createWebHistory`를 사용합니다.
- Pinia는 공유 UI 상태와 이후 tree state의 경계로 사용합니다.
- B01에서는 network online/offline 상태만 작은 store로 구현해 조립 상태를 검증합니다.

### Styling

- B01과 B02는 CSS custom properties와 component-scoped CSS를 사용합니다.
- Tailwind나 UI component framework는 도입하지 않습니다.
- 이유는 모바일 shell의 layout과 접근성 기준을 먼저 검증하고 design dependency를 최소화하기 위해서입니다.

### PWA update policy

- update 전략은 `autoUpdate`가 아니라 사용자 확인 prompt를 사용합니다.
- 자동 reload는 향후 editor의 저장되지 않은 draft를 잃게 할 수 있으므로 금지합니다.
- app shell의 정적 asset만 precache하며 API 응답과 Markdown content는 B01에서 cache하지 않습니다.
- offline은 기존에 불러온 shell을 표시하고, UI 상단 상태에서 API 기능을 사용할 수 없음을 알립니다.

## Directory Structure

```text
frontend/
├── public/
│   └── icons/
├── src/
│   ├── assets/
│   ├── components/
│   ├── router/
│   ├── stores/
│   ├── views/
│   ├── App.vue
│   ├── main.ts
│   └── pwa.ts
├── src/__tests__/
├── index.html
├── package.json
├── tsconfig*.json
└── vite.config.ts
```

## Initial UI Contract

- 첫 route `/`는 KnowledgeOS workspace shell placeholder를 표시합니다.
- 현재 단계와 다음 단계가 화면에 명시되어야 합니다.
- online/offline 상태는 색상만이 아니라 텍스트로 표시합니다.
- service worker가 offline 준비를 마치거나 update를 발견하면 접근 가능한 status panel을 표시합니다.
- 존재하지 않는 route는 shell 안의 Not Found view로 처리합니다.

## Test Plan

- root route가 KnowledgeOS shell을 렌더링
- Pinia network store가 browser online/offline event를 반영
- PWA status panel의 offline-ready와 update prompt 상태
- unknown route가 Not Found view를 렌더링
- TypeScript type check
- ESLint와 Prettier check
- Vitest unit/component test
- production build 후 manifest, service worker와 192/512 icon 생성 확인

## Non-Goals

- Tree API 호출과 lazy node state
- responsive 3-panel layout과 mobile drawer
- Markdown editor와 offline editing
- API response caching
- 인증과 production reverse proxy

## Completion Criteria

- `npm run build`, `npm run type-check`, `npm run lint`, `npm run test:unit`이 통과합니다.
- production build에 Web App Manifest와 service worker가 생성됩니다.
- browser에서 standalone 설치 조건을 만족하는 name, theme color와 icon을 제공합니다.
- B01 완료 후 B02 Responsive App Shell을 진행합니다.

## Implementation Result

- Node.js 20.19.4에서 동작하는 Vue 3.5, Vite 7, Vue Router 4, Pinia 3 조합을 lockfile로 고정했습니다.
- Vue Router 5의 transitive Babel 8이 Node.js 22.18+를 요구해, Node.js 20 계약과 일치하는 Vue Router 4.6으로 조정했습니다.
- 정적 app shell, root와 Not Found route, online/offline store와 PWA 상태 panel을 구현했습니다.
- PWA update는 자동 reload하지 않고 명시적인 사용자 확인을 요구합니다.
- 공식 `@vite-pwa/assets-generator`로 SVG 원본에서 192/512/maskable icon을 재생성할 수 있습니다.
- ESLint, Oxlint, Prettier, TypeScript, Vitest 5개, production build와 PWA artifact 검증이 통과했습니다.
- B01은 production reverse proxy와 frontend container를 포함하지 않으며, 배포 통합은 responsive shell 이후 별도 운영 단위로 진행합니다.
