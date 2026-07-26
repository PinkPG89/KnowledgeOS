# B09 Document Create UI Plan

## 상태

- 진행 상태: Completed
- 선행 조건: A05 Create file, A10 Lazy tree endpoint, B05 Open file flow
- 완료 기준: validation/creating/error/success와 tree refresh·document open 자동화 검증

## 목표

현재 문서 편집 문맥에서 안전하게 새 Markdown 문서를 생성하고, 생성된 문서를 즉시 tree와
editor에 연결합니다.

## UI 계약

- workspace topbar의 새 문서 버튼으로 반응형 panel을 엽니다.
- mobile에서는 navigation/search/inspector drawer와 상호 배타적으로 동작합니다.
- desktop에서는 현재 editor를 유지하는 overlay panel로 동작합니다.
- 사용자는 Vault 기준 relative Markdown path와 optional title을 입력합니다.
- path는 canonical lowercase `.md` 경로여야 하며 parent directory를 자동 생성하지 않습니다.
- title이 있으면 초기 content를 `# {title}\n`으로 생성하고, 없으면 빈 문서를 생성합니다.
- 생성 중에는 중복 submit을 차단합니다.
- API validation, duplicate conflict, missing parent와 network failure message를 panel에서
  표시합니다.
- 성공하면 생성된 parent directory를 강제 refresh한 뒤 `/files/{path}` route로 이동합니다.
- 생성된 문서는 기존 editor save와 browser draft flow를 그대로 사용합니다.

## API 경계

- 기존 `MarkdownClient`에 `createFile(path, content, signal)`을 추가합니다.
- 요청은 `POST /api/files`와 `{ path, content }` JSON을 사용합니다.
- 성공은 `201 Created`와 요청 path/content에 일치하는 document snapshot이어야 합니다.
- 기존 runtime snapshot validation, structured API error와 abort/network normalization을
  재사용합니다.

## 선택 이유

- A05의 exclusive create 정책을 그대로 사용해 UI에서 사전 존재 확인을 하지 않습니다.
- 전체 relative path 입력은 아직 A07 directory create UI가 없는 상태에서도 생성 위치를
  명시할 수 있습니다.
- 생성 후 route 기반 open flow를 재사용하면 별도 editor 초기화 경로가 생기지 않습니다.
- parent tree의 force refresh는 이미 loaded된 directory cache에 새 파일이 누락되는 문제를
  방지합니다.

## 장점

- backend 보호 파일과 API contract를 변경하지 않습니다.
- duplicate concurrent create에서도 기존 문서를 덮어쓰지 않습니다.
- desktop/mobile이 같은 form, error와 open flow를 공유합니다.

## 단점

- 사용자가 존재하는 parent path를 직접 알아야 합니다.
- 생성 전에 directory picker나 directory 자동 생성은 제공하지 않습니다.
- title 외의 frontmatter/template 선택은 제공하지 않습니다.

## 대안

- Tree context menu: 생성 위치가 명확하지만 touch interaction과 focused directory state가 먼저
  필요합니다.
- Directory picker: 사용성은 좋지만 lazy tree의 unloaded subtree 탐색 UI가 추가됩니다.
- Client-side duplicate check: 응답은 빨라질 수 있지만 race를 막지 못하므로 적용하지 않습니다.

## 운영 고려사항

- `file_already_exists`는 이름 변경을 유도하고 overwrite action을 제공하지 않습니다.
- 생성 API 성공 후 route 이동이 취소되더라도 파일은 이미 안전하게 생성된 상태입니다.
- 생성 후 tree refresh 실패는 문서 생성 자체를 rollback하지 않습니다.
- A07 완료 전에는 존재하지 않는 parent를 자동 생성하지 않습니다.

## 검증

- Create request JSON과 `201` snapshot validation
- invalid path, structured conflict, network/abort error
- empty title content와 H1 title content
- submit 중 중복 차단, error 표시와 retry
- desktop/mobile panel 상호 배타성과 close
- 생성 후 parent tree refresh, route open과 editor rendering

## 구현 결과

- 기존 `MarkdownClient`와 분리된 create contract를 추가해 기존 read/update test double의
  호환성을 유지했습니다.
- create response의 `201`, 요청 path/content와 document snapshot을 runtime validation합니다.
- desktop overlay와 mobile 상호 배타 drawer에 새 문서 form을 연결했습니다.
- validation/creating/error/retry와 duplicate submit 차단을 구현했습니다.
- 생성 성공 후 parent tree를 force refresh하고 기존 route/editor flow로 문서를 엽니다.
