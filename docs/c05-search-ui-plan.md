# C05 Search UI Plan

## 상태

- 진행 상태: Completed
- 선행 조건: C03 Incremental index sync, C04 Search API
- 완료 기준: empty/loading/error/result 상태와 keyboard navigation 자동화 검증

## 목표

현재 workspace를 벗어나지 않고 Vault 전체를 검색하고, 키보드와 터치로 결과 문서를 열 수
있는 반응형 검색 패널을 제공합니다.

## UI 계약

- topbar의 검색 버튼으로 패널을 열고 `Escape` 또는 닫기 버튼으로 닫습니다.
- 모바일에서는 기존 navigation/inspector와 상호 배타적인 drawer로 동작합니다.
- 데스크톱에서는 editor 문맥을 유지하는 overlay panel로 동작합니다.
- 검색어는 submit 시 전송하며 앞뒤 공백을 제거합니다.
- 빈 검색어는 API를 호출하지 않고 안내 상태를 유지합니다.
- 요청 중에는 이전 결과를 숨기고 loading 상태를 표시합니다.
- 성공 시 제목, canonical path와 plain-text snippet을 표시합니다.
- 결과가 없으면 검색어를 포함한 empty 상태를 표시합니다.
- 실패 시 API message와 재시도 동작을 제공합니다.
- 입력창의 `ArrowDown`은 첫 결과로 이동합니다.
- 결과 목록은 `ArrowUp`, `ArrowDown`, `Home`, `End`로 이동합니다.
- 결과에서 `Enter`를 누르거나 결과를 누르면 해당 Markdown 문서를 엽니다.

## API 경계

- `SearchClient`가 `/api/search` 요청과 runtime response validation을 담당합니다.
- UI component는 HTTP schema가 아닌 domain model만 사용합니다.
- 새 검색은 이전 요청을 `AbortController`로 취소합니다.
- abort는 사용자 오류로 표시하지 않습니다.
- API error, network error와 invalid response를 구분된 code로 정규화합니다.

## 선택 이유

- 기존 workspace shell과 route 기반 document open flow를 그대로 재사용할 수 있습니다.
- 명시적 submit은 모바일 입력 중 요청 폭증을 방지하고 동작을 예측 가능하게 만듭니다.
- component-local 검색 상태는 다른 화면에서 공유되지 않는 일시적 UI 상태에 적합합니다.
- runtime validation은 backend contract drift가 잘못된 document route로 전파되는 것을 막습니다.

## 장점

- desktop과 mobile에서 같은 검색 결과 component와 keyboard contract를 사용합니다.
- 검색 실패가 editor와 file tree 상태에 영향을 주지 않습니다.
- 별도 dependency 없이 현재 Vue/Pinia 구조와 일관성을 유지합니다.

## 단점

- submit 전에는 live result가 갱신되지 않습니다.
- C05에서는 path prefix 선택 UI를 제공하지 않습니다.
- 검색어와 결과는 panel을 닫으면 유지되지 않습니다.

## 대안

- Debounced live search: 탐색 속도는 빠르지만 cancellation, IME composition과 요청 부하 제어가
  추가로 필요하므로 후속 개선으로 둡니다.
- 독립 search route: deep link에는 유리하지만 문서 편집 문맥이 끊기고 mobile navigation이
  복잡해집니다.
- 전역 search store: 여러 화면에서 상태를 공유할 때 유용하지만 현재 범위에는 불필요한
  lifecycle을 추가합니다.

## 운영 고려사항

- 검색 패널은 backend의 `503 search_unavailable` message를 그대로 노출하고 재시도를
  제공합니다.
- snippet은 backend가 제공하는 plain text로만 렌더링하며 HTML injection을 사용하지 않습니다.
- 결과 score는 정렬에만 사용하고 UI에 절대 relevance 값으로 표시하지 않습니다.
- API 기본 limit 20을 사용해 작은 화면에서 과도한 DOM과 network payload를 피합니다.

## 검증

- Search client query encoding, response validation, API/network error
- initial, loading, empty, error, retry와 result rendering
- input에서 첫 결과로 focus 이동
- Arrow/Home/End navigation과 Enter/click open
- desktop overlay와 mobile drawer open/close
- search result open 후 route 이동과 mobile drawer close

## 구현 결과

- `SearchClient`와 domain model로 HTTP boundary와 UI 상태를 분리했습니다.
- 반응형 SearchPanel을 workspace topbar와 기존 mobile drawer state에 연결했습니다.
- 새 요청에서 이전 요청을 취소하고 stale response를 generation으로 차단합니다.
- initial/loading/empty/error/result, retry와 plain-text snippet을 구현했습니다.
- keyboard navigation과 route 기반 document open flow를 자동화 테스트로 검증했습니다.
