# B07 Editor Save State Machine

- 상태: Implementation Complete
- 완료일: 2026-07-24

## Summary

CodeMirror draft를 Pinia document store의 단일 상태로 이동하고 backend `PUT /api/files/{path}`와 연결했습니다. 저장 상태는 `clean`, `dirty`, `saving`, `conflict`, `error`로 관리합니다. B07은 명시적 저장 버튼과 `Ctrl/Cmd+S`만 제공하며 autosave는 B08 browser draft recovery 이후로 보류합니다.

## 선택 이유

- 저장 중 추가 입력과 API response를 분리해 사용자가 입력한 최신 draft를 잃지 않습니다.
- backend가 반환한 document hash를 다음 update의 `base_hash`로 사용합니다.
- `409 write_conflict`에서 local draft를 유지하고 자동 overwrite를 금지합니다.
- IME 조합과 mobile background 전환이 충분히 검증되기 전에 autosave를 활성화하지 않습니다.

## 상태 계약

```text
clean -> dirty -> saving -> clean
                    |
                    +-> dirty
                    +-> conflict
                    +-> error -> saving
```

- `clean`: draft와 마지막으로 승인된 server snapshot content가 같습니다.
- `dirty`: draft가 server snapshot과 다릅니다.
- `saving`: 하나의 update request만 실행 중입니다.
- `conflict`: server hash가 달라 자동 저장을 중단하고 local draft를 유지합니다.
- `error`: network 또는 server 오류가 발생했으며 retry 가능 여부를 함께 저장합니다.

저장 중 사용자가 계속 입력하면 request에 포함된 snapshot만 server document가 됩니다. response 수신 시 현재 draft가 snapshot과 다르면 상태를 다시 `dirty`로 전환합니다.

## API 계약

```http
PUT /api/files/projects/note.md
Content-Type: application/json

{
  "content": "# Updated\n",
  "base_hash": "sha256:<64 lowercase hex>"
}
```

성공 response는 `MarkdownDocument` 전체 snapshot입니다. `write_conflict`의 `details.current_hash`는 진단 정보로 보존하지만 client가 자동 overwrite에 사용하지 않습니다.

## 사용자 보호

- 저장 중 두 번째 저장 요청은 새 HTTP request를 만들지 않습니다.
- IME composition 중 저장 버튼과 keyboard save를 비활성화합니다.
- browser reload와 tab close에는 native unsaved-change 경고를 요청합니다.
- 내부 route 이동은 확인 전까지 차단합니다.
- conflict 해결의 destructive action은 확인 후 server version을 다시 읽습니다.

## 장점

- editor 구현과 persistence 상태를 분리해 B08 draft recovery를 store 기준으로 확장할 수 있습니다.
- stale base hash를 사용한 무조건 overwrite를 차단합니다.
- 저장 response 도착 전에 발생한 추가 입력을 보존합니다.

## 단점

- browser native confirmation 문구는 browser 정책에 따라 사용자 정의가 제한됩니다.
- network 오류 직전에 server write가 완료됐지만 response만 유실되면 retry가 conflict로 바뀔 수 있습니다.
- B08 전에는 사용자가 명시적으로 discard한 draft를 복구할 수 없습니다.
- 현재 저장은 manual이며 debounce autosave를 제공하지 않습니다.

## 대안

- 즉시 autosave: 입력 손실 가능성은 줄지만 IME partial composition, mobile background 중단과 draft recovery가 먼저 필요합니다.
- last-write-wins: UI는 단순하지만 외부 editor와 공유하는 filesystem-first Vault에서 데이터 손실 위험이 큽니다.
- server-side edit session: 강한 coordination이 가능하지만 개인 서버 MVP에는 상태와 운영 복잡도가 과도합니다.

## 실제 보장 범위

- KnowledgeOS request 간에는 backend hash 비교와 atomic replace가 stale update를 감지합니다.
- 이 보장은 trusted local process 환경의 optimistic concurrency control입니다.
- 적대적인 local process가 검사와 replace 사이 filesystem을 변경하는 모든 TOCTOU 상황을 방어한다고 표현하지 않습니다.
- 외부 editor 변경 감지와 browser draft recovery는 후속 단계에서 강화합니다.

## 자동화 검증

- update request path encoding, body와 base hash
- conflict response의 current hash parsing
- 저장 중 중복 request 차단
- 저장 중 추가 입력 보존과 `dirty` 복귀
- retryable error 재시도
- conflict local draft 유지 component UI
- unsaved internal navigation 차단
- 기존 read, preview, tree와 mobile flow 회귀

## 운영 시 고려사항

- production Vault를 SilverBullet과 동시에 쓰는 동안 conflict 가능성이 있으므로 overwrite 없는 정책을 유지합니다.
- B08 완료 전에는 save debounce를 활성화하지 않습니다.
- conflict와 retry 빈도는 향후 structured logging과 metrics 대상으로 추가합니다.
- server version reload는 local draft를 버리는 명시적 사용자 동작으로만 실행합니다.

## 다음 단계

B08에서 `path + base_hash`를 key로 browser draft를 저장하고 reload recovery, resume/discard UI와 remote-change conflict 검증을 구현합니다.
