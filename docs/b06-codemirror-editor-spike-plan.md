# B06 CodeMirror Markdown Editor Spike

- 상태: Implementation Complete · Device Validation Pending
- 시작일: 2026-07-23

## Summary

CodeMirror 6 기반 Markdown 편집 surface와 모바일 toolbar를 구현했습니다. 문서는 기본적으로 rendered preview로 열리며 사용자가 `편집` mode로 전환할 수 있습니다. 현재 draft는 component local state에만 존재하며 backend 저장 API와 연결하지 않습니다. Korean IME 조합 중 외부 model replacement를 지연해 조합 문자열이 중간에 교체되지 않도록 했습니다.

B06의 최종 완료 기준에는 iPhone Safari와 Android Chrome 실기기 검증이 포함됩니다. 자동화 테스트와 desktop browser 검증만으로 이 조건을 대체하지 않으므로 CodeMirror 채택 상태는 `Provisional`입니다.

## 선택 이유

- CodeMirror 6는 viewport 중심 rendering으로 큰 문서에서 전체 DOM 생성을 피합니다.
- extension 조합 방식이므로 mobile UX에 필요하지 않은 기능을 제외할 수 있습니다.
- Markdown language package, 접근성 속성, composition 상태와 transaction API를 직접 제어할 수 있습니다.
- WYSIWYG보다 filesystem-first Markdown 원문을 정확하게 유지하기 쉽습니다.

## 채택 계약

- `codemirror` `6.0.2`의 `minimalSetup`을 기본 구성으로 사용합니다.
- `@codemirror/lang-markdown` `6.5.1`로 Markdown syntax language support를 추가합니다.
- line wrapping을 활성화하고 editor content는 UTF-8 원문 문자열로 취급합니다.
- Vue `modelValue`가 IME composition 중 변경되면 즉시 덮어쓰지 않고 `compositionend` 뒤 적용합니다.
- toolbar pointer interaction은 현재 selection을 유지하며 44px 이상의 touch target을 제공합니다.
- `markdown-it`은 raw HTML을 비활성화하고 DOMPurify로 생성 HTML을 다시 sanitize합니다.
- 문서는 기본적으로 preview mode로 열고 source editing은 명시적 전환 후 제공합니다.
- B06 draft는 저장되지 않으며 화면에 이를 명시합니다.

## 장점

- Vue store와 CodeMirror document model의 경계를 작은 component로 제한합니다.
- B07 save state machine이 editor 구현 세부사항과 분리된 draft contract를 사용할 수 있습니다.
- mobile toolbar를 extension에 종속시키지 않아 이후 명령과 접근성 정책을 독립적으로 변경할 수 있습니다.
- 큰 문서의 viewport rendering을 component test로 회귀 검증할 수 있습니다.

## 단점

- CodeMirror document와 Vue 문자열 model 사이에 큰 문자열 복사가 발생할 수 있습니다.
- jsdom composition test는 browser event ordering을 완전히 재현하지 못합니다.
- `minimalSetup`은 search, history shortcut와 bracket behavior 등 필요한 extension을 이후 명시적으로 선택해야 합니다.
- B07 전까지 편집 결과를 저장하거나 복구할 수 없습니다.

## 대안

- Native `textarea`: dependency와 integration은 단순하지만 큰 문서, syntax highlighting와 selection command 확장이 제한됩니다.
- Monaco Editor: 기능은 풍부하지만 mobile bundle과 touch UX 비용이 큽니다.
- TipTap/ProseMirror: rich text UX에는 유리하지만 Markdown 원문 round-trip과 source editing 요구에 추가 변환 계층이 필요합니다.
- Toast UI Editor: 완성형 UI를 빠르게 제공하지만 KnowledgeOS의 작은 extension surface와 mobile composition 제어에는 과도합니다.

## 구현 범위

- Markdown syntax language support
- line wrapping과 accessible textbox label
- heading, bold, italic, list, task와 link toolbar prototype
- IME composition 상태 표시
- composition 중 external replacement 지연
- large document viewport rendering 자동화 검증
- heading, emphasis, list, table, code와 link Markdown preview
- preview와 source editor mode 전환

## 비범위

- backend `PUT` 연결과 autosave
- clean, dirty, saving, conflict와 error 상태
- browser draft recovery
- split view
- Obsidian wikilink, embed와 attachment resolution
- attachment upload와 paste handling

## 자동화 검증

- 한글 Markdown 초기 rendering
- 접근 가능한 textbox와 toolbar label
- toolbar transaction과 전체 draft emit
- composition 중 external replacement 지연
- 500 KiB 이상, 30,000 line 문서의 viewport rendering
- preview rendering, draft 반영과 mode 전환
- raw HTML, event handler와 unsafe link protocol 차단
- 기존 deep link, tree selection, retry와 mobile drawer 회귀

## 실기기 검증 체크리스트

### iPhone Safari

- 두벌식 한글 연속 입력에서 자모 분리나 중복 문자가 발생하지 않는다.
- 후보 선택, backspace와 문장 중간 cursor 이동이 정상 동작한다.
- software keyboard가 열린 상태에서 toolbar 버튼을 누르면 selection이 유지된다.
- 5 MiB에 가까운 Markdown에서 scroll과 입력이 사용 가능한 수준이다.
- 세로/가로 회전 후 editor와 toolbar가 viewport를 벗어나지 않는다.

### Android Chrome

- Gboard 한글 조합, 후보 선택과 backspace가 정상 동작한다.
- toolbar touch target과 selection이 유지된다.
- 5 MiB에 가까운 Markdown에서 scroll과 입력이 사용 가능한 수준이다.
- browser back과 tree drawer 동작이 draft를 의도치 않게 저장하지 않는다.

## 운영 시 고려사항

- 실제 저장 기능을 연결하기 전까지 이 editor는 production write surface로 취급하지 않습니다.
- B07에서는 full-string emit 빈도와 save debounce를 별도로 측정해야 합니다.
- API response 또는 watcher update가 composition과 충돌할 때 사용자 draft를 자동 교체하면 안 됩니다.
- dependency update 시 CodeMirror composition 관련 release note와 mobile regression을 다시 검증합니다.
- preview는 standard Markdown 범위이며 Obsidian-specific syntax는 migration review 후 별도 extension으로 추가합니다.
- PWA asset generator의 dev-only `sharp` 취약점은 npm override `0.35.3`으로 해소했으며 asset generation을 회귀 검증합니다.

## 다음 단계

핑크님이 iPhone Safari와 Android Chrome 체크리스트를 수행하고 결과를 기록하면 B06을 `Completed`로 전환합니다. 실패 항목이 있으면 B07로 넘어가기 전에 editor integration을 수정합니다.
