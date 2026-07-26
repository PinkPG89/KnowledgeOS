# B09 Document Create UI Plan

## 상태

- 진행 상태: Completed
- 선행 조건: A05 Create file, A10 Lazy tree endpoint, B05 Open file flow
- 완료 기준: validation/creating/error/success와 tree refresh·document open 자동화 검증

## 목표

현재 문서 편집 문맥에서 안전하게 새 Markdown 문서를 생성하고, 생성된 문서를 즉시 tree와
editor에 연결합니다.

## UI 계약

- 파일 트리 toolbar의 새 문서 버튼으로 tree 내부 inline form을 엽니다.
- focus된 node가 directory면 해당 directory를 생성 위치로 사용합니다.
- focus된 node가 file이면 parent directory를 생성 위치로 사용합니다.
- focus와 selected node가 없으면 Vault root를 생성 위치로 사용합니다.
- 사용자는 전체 path가 아닌 파일명만 입력합니다.
- `.md`는 자동으로 추가하고 대문자 `.MD`는 lowercase `.md`로 정규화합니다.
- `/`, `\`, hidden name과 canonical path 위반은 API 호출 전에 거부합니다.
- 새 문서는 빈 content로 생성하고 filename title fallback을 사용합니다.
- 생성 중에는 중복 submit을 차단합니다.
- API validation, duplicate conflict, missing parent와 network failure message를 inline form에서
  표시합니다.
- `Escape`와 취소 버튼으로 inline form을 닫습니다.
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
- tree focus가 생성 위치를 결정하므로 사용자가 전체 relative path를 기억할 필요가 없습니다.
- 생성 후 route 기반 open flow를 재사용하면 별도 editor 초기화 경로가 생기지 않습니다.
- parent tree의 force refresh는 이미 loaded된 directory cache에 새 파일이 누락되는 문제를
  방지합니다.

## 장점

- backend 보호 파일과 API contract를 변경하지 않습니다.
- duplicate concurrent create에서도 기존 문서를 덮어쓰지 않습니다.
- desktop/mobile file tree가 같은 inline form, error와 open flow를 공유합니다.
- filename 입력만 필요하므로 path 입력 오류와 interaction 단계가 줄어듭니다.

## 단점

- 생성하려는 directory가 tree에 보이도록 먼저 탐색해야 합니다.
- directory 자동 생성과 template 선택은 제공하지 않습니다.
- 새 문서는 빈 content이므로 생성 직후 editor에서 내용을 작성해야 합니다.

## 대안

- 별도 overlay path form: 구현은 단순하지만 tree 문맥을 버리고 전체 경로 입력을 요구해
  폐기했습니다.
- Directory별 context menu: 위치는 더 직접적이지만 mobile long-press와 menu 접근성이
  추가됩니다.
- Directory picker: 별도 화면이 필요하고 현재 lazy tree와 기능이 중복됩니다.
- Client-side duplicate check: 응답은 빨라질 수 있지만 race를 막지 못하므로 적용하지 않습니다.

## 운영 고려사항

- `file_already_exists`는 이름 변경을 유도하고 overwrite action을 제공하지 않습니다.
- 생성 API 성공 후 route 이동이 취소되더라도 파일은 이미 안전하게 생성된 상태입니다.
- 생성 후 tree refresh 실패는 문서 생성 자체를 rollback하지 않습니다.
- A07 완료 전에는 존재하지 않는 parent를 자동 생성하지 않습니다.

## 검증

- Create request JSON과 `201` snapshot validation
- invalid path, structured conflict, network/abort error
- root/directory/file focus에 따른 create parent 선택
- `.md` 자동 추가, `.MD` 정규화와 invalid filename
- submit 중 중복 차단, error 표시와 retry
- Escape/cancel과 mobile tree drawer 흐름
- 생성 후 parent tree refresh, route open과 editor rendering

## 구현 결과

- 기존 `MarkdownClient`와 분리된 create contract를 추가해 기존 read/update test double의
  호환성을 유지했습니다.
- create response의 `201`, 요청 path/content와 document snapshot을 runtime validation합니다.
- tree toolbar와 focus/selected node 문맥에 inline create form을 연결했습니다.
- filename을 canonical Markdown path로 정규화하고 parent path를 자동 조합합니다.
- validation/creating/error/retry와 duplicate submit 차단을 구현했습니다.
- 생성 성공 후 parent tree를 force refresh하고 기존 route/editor flow로 문서를 엽니다.
