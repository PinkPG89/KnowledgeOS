# B08 Browser Draft Recovery

- 상태: Implementation Complete
- 완료일: 2026-07-24

## Summary

저장하지 않은 Markdown draft를 browser IndexedDB에 보관하고 reload 후 resume 또는 discard할 수 있도록 구현했습니다. Draft는 `path`, `baseHash`, `content`, `updatedAt`을 함께 저장합니다. Server document hash가 draft의 base hash와 다르면 자동 적용하지 않고 recovery conflict로 격리합니다.

## 선택 이유

- Backend가 허용하는 최대 5 MiB Markdown은 일반적인 `localStorage` quota에 근접하거나 초과할 수 있습니다.
- IndexedDB는 큰 문자열과 비동기 transaction을 지원해 editor main thread blocking을 줄입니다.
- Draft repository interface를 browser API와 Pinia store 사이에 두어 테스트와 향후 storage migration이 가능합니다.
- Browser draft는 Markdown 원본을 대체하지 않는 임시 recovery projection으로 취급합니다.

## 데이터 계약

```text
BrowserDraft
├─ path: canonical Markdown path
├─ baseHash: sha256 server snapshot hash
├─ content: UTF-8 draft string
└─ updatedAt: RFC3339 millisecond timestamp
```

IndexedDB:

```text
database: knowledgeos
version: 1
object store: drafts
keyPath: path
record version: 1
```

한 path에는 최신 browser draft 하나만 보관합니다. `baseHash`는 해당 draft가 시작된 server snapshot을 나타냅니다.

## Recovery 상태

```text
none
available  baseHash == server hash
conflict   baseHash != server hash
```

- `available`: server가 바뀌지 않았으므로 사용자가 선택하면 `dirty` draft로 복구합니다.
- `conflict`: server 최신 content와 browser draft를 분리 보존하고 자동 적용하지 않습니다.
- conflict draft를 열면 editor는 local content를 표시하지만 save state는 `conflict`로 유지해 자동 overwrite를 차단합니다.
- discard가 IndexedDB에서 실패하면 recovery UI와 draft reference를 유지합니다.

## Persistence 동작

- Editor 변경 후 300 ms 동안 추가 변경을 모아 최신 draft만 기록합니다.
- document route 전환 전 pending write를 flush합니다.
- tab이 background로 이동하거나 unload가 시작되면 best-effort flush를 요청합니다.
- server save 성공 후 browser draft를 제거합니다.
- 저장 중 추가 입력이 있으면 server가 반환한 새 hash를 base hash로 다시 기록합니다.
- browser storage 실패는 server document load를 막지 않고 UI에 backup error로 표시합니다.

## 장점

- reload, PWA update와 browser crash 이후 저장하지 않은 작업을 복구할 수 있습니다.
- 외부 editor가 server file을 변경한 경우 stale draft를 자동 overwrite하지 않습니다.
- IndexedDB 구현을 store에서 분리해 memory repository 기반 결정적 테스트가 가능합니다.

## 단점

- Private mode, quota 부족과 browser storage 정책에 따라 IndexedDB가 실패할 수 있습니다.
- Browser가 process를 즉시 종료하면 마지막 debounce 구간의 draft가 기록되지 않을 수 있습니다.
- Browser profile 삭제, site data 삭제와 origin 변경 시 draft도 사라집니다.
- Conflict 자동 merge는 제공하지 않으므로 복사·비교 후 사용자가 정리해야 합니다.

## 대안

- `localStorage`: 구현은 단순하지만 5 MiB 문서와 synchronous write에 부적합합니다.
- Server-side draft: 장치 간 복구는 가능하지만 filesystem source of truth 외에 영속 원본이 추가됩니다.
- Service Worker Cache API: response cache에는 적합하지만 path/hash 기반 mutable draft repository로는 계약이 불명확합니다.
- OPFS: 큰 파일에 유리하지만 browser 지원·backup semantics와 migration 복잡도가 MVP에 과도합니다.

## 실제 보장 범위

- Draft recovery는 같은 browser origin과 profile의 IndexedDB가 유지되는 범위에서만 동작합니다.
- `visibilitychange`와 `beforeunload` flush는 best-effort이며 process 강제 종료 전 완료를 보장하지 않습니다.
- `baseHash` 비교는 recovery 시점의 server response와 draft 기준 snapshot 불일치를 감지합니다.
- 적대적인 local process의 filesystem TOCTOU나 다중 장치 실시간 merge를 보장하지 않습니다.

## 자동화 검증

- path와 base hash를 포함한 draft persistence
- store 재생성 후 reload recovery
- server hash 변경 시 recovery conflict 격리
- conflict UI에서 server content 우선 표시
- 사용자가 conflict draft를 연 뒤 save 차단
- server save 성공 후 browser draft 제거
- IndexedDB read·delete 실패 시 안전한 fallback
- B07 중복 저장, retry, conflict와 기존 frontend 회귀

## 운영 시 고려사항

- Reverse proxy hostname 또는 scheme을 바꾸면 browser origin이 달라져 기존 draft에 접근할 수 없습니다.
- Browser storage 사용량과 quota 실패는 향후 diagnostics 화면에 노출해야 합니다.
- Browser draft는 backup이 아니며 Vault와 Git/offsite backup 정책을 대체하지 않습니다.
- Autosave는 B08 완료만으로 자동 활성화하지 않고 Korean IME와 mobile background 성능을 별도로 검증한 후 결정합니다.

## 다음 단계

Frontend Track B의 구현 단계가 완료됐습니다. 다음 roadmap 단계는 C01 SQLite FTS5 index schema와 rebuild lifecycle입니다. B06 iPhone Safari와 Android Chrome 실기기 검증은 별도 완료 조건으로 계속 추적합니다.
