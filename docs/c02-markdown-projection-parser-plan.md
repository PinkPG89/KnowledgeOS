# C02 Markdown Projection Parser

- 상태: Completed
- 완료일: 2026-07-26

## Summary

UTF-8 `MarkdownDocument` snapshot을 SQLite 검색 index에서 사용할 파생 데이터로 변환하는
tolerant parser를 구현했습니다. Parser는 원본 Markdown을 변경하지 않고 filesystem과
SQLite에도 접근하지 않습니다. Frontmatter metadata가 손상돼도 전체 원문 body와 fallback
title을 계속 반환하므로 검색 projection 장애가 파일 CRUD를 차단하지 않습니다.

## 선택 이유

- `pulldown-cmark 0.13.4`는 CommonMark event와 source offset을 제공해 heading, link,
  image와 code 범위를 문자열 정규식보다 안정적으로 구분합니다.
- `yaml-rust2 0.11.0`은 pure Rust YAML 1.2 parser이며 Rust 1.85 기준선과 호환됩니다.
- Parser와 projection model을 분리해 C03 index sync가 Markdown 문법 세부사항에
  의존하지 않도록 했습니다.
- 검색 DB는 재생성 가능한 cache이므로 parser 결과가 원본 파일을 수정하거나 저장하지
  않습니다.

## Projection Contract

```text
MarkdownProjection
├─ path
├─ title
├─ body
├─ tags[]
├─ links[]
├─ content_hash
├─ frontmatter_json?
└─ frontmatter_status
```

Title은 다음 우선순위로 결정합니다.

1. valid frontmatter의 비어 있지 않은 string `title`
2. Markdown body의 첫 번째 H1 text
3. `.md`를 제거한 filename stem

Valid frontmatter는 body에서 제거하고 canonical JSON으로 보존합니다. YAML parse 실패,
mapping이 아닌 root, JSON object로 안전하게 변환할 수 없는 key 또는 닫히지 않은
delimiter는 `Malformed`로 기록하고 전체 원문을 body로 유지합니다.

## Tags And Links

- Frontmatter `tags`는 string 또는 string array를 허용합니다.
- Inline hashtag는 Unicode 문자, 숫자, `_`, `-`, `/`를 허용합니다.
- Tag는 lowercase로 정규화하고 최초 출현 순서를 유지하며 중복을 제거합니다.
- Inline/fenced code와 Markdown link destination의 hashtag는 tag로 해석하지 않습니다.
- Standard Markdown link, Markdown image, `[[wiki]]`, `![[embed]]`를 구분해 보존합니다.
- Wiki alias는 제거하고 heading fragment를 포함한 target reference는 유지합니다.
- Code block, inline code와 raw HTML 안의 wiki syntax는 link로 해석하지 않습니다.

## 장점

- Malformed metadata가 검색 품질만 낮추고 파일 접근성을 훼손하지 않습니다.
- Frontmatter와 본문을 분리해 FTS body에 YAML key noise가 들어가는 것을 줄입니다.
- Link syntax 종류를 보존해 후속 backlink와 attachment resolution에 재사용할 수 있습니다.
- Pure parser unit test로 filesystem과 DB 없이 빠르게 회귀 검증할 수 있습니다.

## 단점

- YAML의 non-string mapping key는 JSON projection에서 지원하지 않습니다.
- Inline tag는 형태소 분석 없이 문법 문자 경계만 검사합니다.
- Wiki target resolution과 broken link 판정은 수행하지 않습니다.
- H1 title은 rendered plain text만 보존하며 원래 Markdown formatting은 제거됩니다.

## 대안

- 정규식만 사용: dependency는 적지만 nested Markdown, code와 reference link 경계가
  불안정해 제외했습니다.
- `serde_yaml`: API는 편리하지만 유지보수 중단 상태이므로 신규 채택하지 않았습니다.
- `serde_yml`: 최신 release도 deprecated 상태이므로 제외했습니다.
- Frontmatter 필수화: metadata 품질은 높지만 외부 AI와 editor의 직접 Markdown 접근성을
  저해하므로 KnowledgeOS 원칙과 맞지 않습니다.

## 자동화 검증

- YAML frontmatter title, tags, JSON과 body 분리
- title의 H1 및 filename fallback
- malformed, unclosed, empty와 CRLF frontmatter
- frontmatter와 Unicode inline tag 병합·중복 제거
- inline/fenced code와 URL fragment tag 제외
- Markdown/wiki/embed/image link 추출과 중복 제거
- code와 raw HTML 내부 wiki syntax 제외
- non-mapping과 JSON 비호환 YAML metadata 격리

## 운영 시 고려사항

- C02는 runtime startup이나 API contract를 변경하지 않습니다.
- 큰 파일은 A04의 5 MiB read limit 안에서만 parser에 전달됩니다.
- Parser dependency update 시 Rust 1.85 MSRV와 CommonMark source offset 동작을 다시
  검증해야 합니다.
- Malformed frontmatter 수는 C03/C04에서 logging 또는 search diagnostics로 관찰해야
  합니다.

## 다음 단계

C03에서 create, update, move, trash와 full reconciliation 흐름에 projection upsert/delete를
연결합니다. 원본과 index의 hash drift를 검사하고 검색 장애가 파일 CRUD 응답을 실패시키지
않는 degraded policy를 유지합니다.
