# B02 Responsive App Shell Plan

- 상태: Completed
- 완료일: 2026-07-22

## Summary

KnowledgeOS frontend를 landing 화면에서 실제 작업공간 구조로 전환했습니다. Desktop에서는 navigation, editor, inspector를 동시에 배치하고, mobile에서는 editor만 기본 표시한 뒤 양쪽 panel을 상호 배타적인 drawer로 엽니다.

## 선택 이유

- Markdown 편집 영역은 mobile viewport 전체를 우선 사용해야 합니다.
- Desktop 사용자는 파일 탐색과 metadata 확인을 위해 세 영역을 동시에 볼 수 있어야 합니다.
- B03 tree state와 B05 file open flow가 layout 세부 구현을 알지 않도록 panel 상태를 Pinia store에 분리했습니다.
- Many Notes의 반응형 동작은 참고하되 component와 상태 모델은 KnowledgeOS 계약에 맞게 독립 구현했습니다.

## Layout Contract

### Desktop

- `64rem` 이상을 초기 desktop breakpoint로 사용합니다.
- navigation, editor, inspector의 3-column layout을 제공합니다.
- navigation과 inspector는 각각 독립적으로 접거나 다시 열 수 있습니다.
- 두 panel의 표시 선호만 `localStorage`에 저장합니다.

### Mobile

- `64rem` 미만에서는 editor가 전체 작업 폭을 사용합니다.
- navigation과 inspector는 화면 양쪽에서 overlay drawer로 엽니다.
- 두 drawer는 동시에 열리지 않습니다.
- backdrop과 panel close button으로 drawer를 닫을 수 있습니다.
- mobile drawer 상태는 일시 상태이며 저장하지 않습니다.

### Breakpoint Transition

- viewport mode 변경 시 열린 mobile drawer를 닫습니다.
- desktop panel 선호는 mobile 전환 중에도 유지합니다.
- `matchMedia` event를 사용해 CSS breakpoint와 JavaScript 상태 경계를 동일하게 유지합니다.

## Accessibility

- panel toggle은 `aria-controls`와 `aria-expanded`를 제공합니다.
- 닫힌 panel에는 `aria-hidden`과 `inert`를 적용해 focus 진입을 막습니다.
- 모든 icon button은 최소 `44px` touch target과 accessible label을 가집니다.
- motion 감소 설정에서는 drawer transition을 제거합니다.
- mobile backdrop은 명시적인 close control로 동작합니다.

## State Boundary

`layout` store는 다음 client-only 상태만 소유합니다.

- 현재 viewport mode
- desktop navigation/inspector preference
- 현재 열린 mobile drawer

Tree node, active file, editor draft, server response는 B02 store에 포함하지 않습니다. 각 데이터는 이후 Track B 단계에서 별도 상태 경계를 갖습니다.

## 장점

- B03과 B04가 고정된 panel slot에 tree 기능을 추가할 수 있습니다.
- mobile과 desktop의 상태 의미를 분리해 resize 과정의 예측 가능성을 높입니다.
- server API 없이도 layout과 접근성을 독립적으로 테스트할 수 있습니다.

## 단점과 대안

- `64rem` breakpoint는 실제 기기 검증 전 초기값입니다. B06 mobile editor spike에서 조정할 수 있습니다.
- resizable desktop panel은 구현하지 않았습니다. 고정 폭은 단순하지만 큰 화면 활용도가 제한됩니다.
- mobile bottom navigation은 tree/editor/search/info 기능이 연결되는 시점까지 연기했습니다. 빈 navigation 항목을 먼저 제공하는 것보다 실제 route와 함께 도입하는 편이 접근성과 정보 구조 검증에 유리합니다.

## 운영 시 고려사항

- `localStorage`가 차단되거나 손상되면 기본 desktop layout으로 복구합니다.
- 저장 실패는 현재 session의 panel 조작을 중단시키지 않습니다.
- CSS media query와 TypeScript의 `DESKTOP_LAYOUT_QUERY` 값을 변경할 때 함께 갱신해야 합니다.
- mobile browser의 dynamic viewport를 고려해 높이는 `100dvh`를 사용합니다.

## Test Result

- desktop 3영역 렌더링
- mobile drawer 상호 배타성 및 backdrop close
- breakpoint 전환 시 mobile overlay 정리
- desktop preference만 persistence
- 손상된 preference fallback
- 기존 route, network, PWA status 회귀 테스트
- `npm run validate` production 검증

## Next Step

B03 Tree State Model에서 canonical relative path를 key로 사용하는 lazy tree client state를 구현합니다.
