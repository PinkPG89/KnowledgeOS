# Path Policy

- 상태: Accepted
- 최종 갱신: 2026-07-12
- 적용 단계: A02 Canonical Path Policy

## 결론

KnowledgeOS API와 domain layer는 사용자 입력 경로를 운영체제 경로로 즉시 변환하지 않습니다. 먼저 `/` 구분자를 사용하는 UTF-8 상대 경로로 검증하고 `CanonicalPath` value object로 변환합니다.

여기서 canonical은 `std::fs::canonicalize`를 호출했다는 뜻이 아닙니다. 아직 filesystem에 접근하지 않은 상태에서 하나의 허용된 lexical 표현만 유지한다는 뜻입니다. 실제 root containment와 symlink 검사는 A03에서 수행합니다.

## API 표현

중첩 경로는 Axum wildcard parameter를 사용합니다.

```http
GET /api/files/{*path}
PUT /api/files/{*path}
DELETE /api/files/{*path}
```

Axum이 URL을 한 번 percent-decode한 문자열을 domain parser에 전달합니다. 애플리케이션은 path를 다시 decode하지 않습니다. 이중 decode의 모호성을 막기 위해 decode 후에도 `%`가 남은 path는 거부합니다.

## 허용 규칙

- `knowledge/` 기준 상대 경로만 사용합니다.
- directory separator는 `/`만 사용합니다.
- Unicode와 한글, ASCII space, `_`와 `-`를 허용합니다.
- Unicode normalization은 수행하지 않고 입력 code point를 보존합니다.
- `_trash`, `_attachments`, `_templates` 같은 underscore system directory를 허용합니다.
- Markdown file path는 마지막 segment가 소문자 `.md`로 끝나야 합니다.
- 전체 path는 UTF-8 기준 1,024 bytes 이하입니다.
- 각 segment는 UTF-8 기준 255 bytes 이하입니다.

예시:

```text
projects/knowledgeos/architecture.md
프로젝트/지식 운영체제.md
daily/2026-07-12.md
_trash/2026-07-12/deleted-note.md
```

## 거부 규칙

- 빈 path와 빈 segment
- `/etc/passwd` 같은 Unix absolute path
- `C:/notes/a.md` 같은 Windows drive path
- `.` 또는 `..` segment
- `\` Windows separator
- `.`으로 시작하는 hidden segment
- NUL과 control character
- `%`가 남아 있는 double-encoded 또는 모호한 path
- 1,024 bytes를 넘는 전체 path
- 255 bytes를 넘는 segment
- Markdown file API에서 `.md`가 아닌 extension

위험한 입력을 자동으로 정규화하거나 수정하지 않습니다. 예를 들어 `projects//note.md`, `projects/./note.md`, `projects\note.md`는 유효한 경로로 바꾸지 않고 오류를 반환합니다.

## 선택 이유

- 외부 AI, Git, CLI, UI가 동일한 Linux filesystem identity를 공유해야 합니다.
- 위험한 입력을 조용히 수정하면 사용자가 의도하지 않은 파일을 조작할 수 있습니다.
- validated value object를 사용하면 모든 CRUD use case가 같은 path invariant를 공유합니다.
- OS path 처리와 HTTP decoding을 domain 규칙에서 분리할 수 있습니다.

## 장점

- path traversal과 separator 혼용을 filesystem 접근 전에 거부합니다.
- API handler마다 validation을 반복하지 않습니다.
- 한글 파일명과 nested directory를 보존합니다.
- A03 root containment와 symlink 검사의 입력 조건이 단순해집니다.

## 단점

- Windows client가 보낸 `\`를 자동 변환하지 않습니다.
- `%`가 포함된 실제 Linux 파일명은 API에서 사용할 수 없습니다.
- Unicode normalization 형태가 다른 두 파일을 동일한 이름으로 간주하지 않습니다.
- 기존 vault import 시 거부된 파일명을 별도로 보고하고 변환해야 할 수 있습니다.

## 대안

- query parameter: 구현은 단순하지만 file resource URL의 가독성이 낮습니다.
- 자동 normalization: 사용성은 좋아지지만 입력 오류와 공격을 숨길 수 있습니다.
- `PathBuf` 직접 사용: host OS semantics가 domain contract에 유입됩니다.
- 모든 extension 허용: attachment와 Markdown API의 보안 경계가 흐려집니다.

## 운영 시 고려사항

- reverse proxy와 Axum 사이 URL decode 횟수를 통합 테스트해야 합니다.
- filesystem 접근 직전에는 A03 root containment와 symlink 검사를 추가로 수행합니다.
- 외부 AI가 API를 우회하므로 Git audit와 filesystem permission은 별도로 적용합니다.
- `_attachments/` binary API는 Markdown file API와 분리합니다.
- case sensitivity는 Linux filesystem 동작을 유지하며 대소문자를 자동 병합하지 않습니다.
