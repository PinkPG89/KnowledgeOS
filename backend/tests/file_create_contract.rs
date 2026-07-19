use std::fs;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tower::ServiceExt;

use knowledgeos_backend::{build_router, config::AppConfig};

/// JSON 형식의 Value를 입력받아 직렬화한 후, `raw_request`를 통해 파일 생성 API를 호출하는 헬퍼 함수입니다.
async fn create_request(vault: &TempDir, payload: Value, max_bytes: u64) -> (StatusCode, Value) {
    raw_request(
        vault,
        serde_json::to_vec(&payload).expect("JSON should encode"),
        max_bytes,
    )
    .await
}

/// 가상의 HTTP POST 요청을 생성하여 파일 생성 API 엔드포인트 `/api/files`를 호출하고 응답을 반환받는 비동기 헬퍼 함수입니다.
///
/// * `vault`: 테스트용 임시 저장소 디렉터리 핸들
/// * `body`: 요청 본문으로 보낼 직렬화된 바이트 배열
/// * `max_bytes`: 테스트에 적용할 단일 마크다운 파일의 최대 허용 바이트 크기
async fn raw_request(vault: &TempDir, body: Vec<u8>, max_bytes: u64) -> (StatusCode, Value) {
    // 1. 테스트용 Mock 설정을 준비하고 허용 용량 한계를 설정합니다.
    let mut config = AppConfig::for_test(vault.path());
    config.max_markdown_bytes = max_bytes;

    // 2. HTTP 라우터를 구동 준비 상태로 빌드합니다.
    let response = build_router(config)
        .expect("test Vault should initialize")
        // 3. tower::ServiceExt::oneshot을 사용해 실제 포트를 할당하지 않고,
        //    가상의 HTTP POST /api/files 요청을 라우터에 주입하여 실행합니다.
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/files")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("test request should be valid"),
        )
        .await
        .expect("create-file API should respond");

    // 4. 응답으로부터 HTTP 상태 코드를 획득합니다.
    let status = response.status();

    // 5. 비동기 응답 스트림의 데이터를 모아 바이트 배열로 받습니다.
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body should be readable")
        .to_bytes();

    // 6. 응답 바디를 JSON으로 역직렬화하여 반환합니다.
    let payload = serde_json::from_slice(&body).expect("response should be JSON");
    (status, payload)
}

/// 중첩된 한글 경로와 공백이 포함된 파일명으로 신규 마크다운 파일 생성을 요청했을 때,
/// 파일이 디스크에 성공적으로 쓰이고, 201 Created 상태 코드와 규격에 맞는 메타데이터가 JSON 응답으로 반환되는지 검증합니다.
#[tokio::test]
async fn creates_nested_unicode_markdown_and_returns_a_document() {
    // 1. 테스트를 위한 임시 격리 Vault 디렉터리를 생성합니다.
    let vault = TempDir::new().expect("temporary Vault should be created");
    // 2. 부모 디렉터리인 "프로젝트" 폴더를 실존하게 미리 만들어둡니다. (생성 규칙 상 부모 디렉터리는 실존해야 함)
    fs::create_dir(vault.path().join("프로젝트")).expect("parent should be created");

    // 3. 가상 HTTP POST 요청으로 파일 생성을 시도합니다.
    let (status, payload) = create_request(
        &vault,
        json!({
            "path": "프로젝트/지식 노트.md",
            "content": "# 지식\n"
        }),
        5 * 1024 * 1024,
    )
    .await;

    // 4. 반환된 상태 코드 및 JSON 명세(Contract)의 유효성을 검증 단언합니다.
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(payload["path"], "프로젝트/지식 노트.md");
    assert_eq!(payload["content"], "# 지식\n");
    assert_eq!(payload["size"], "# 지식\n".len() as u64);
    assert_eq!(
        payload["hash"],
        "sha256:ffd30acf622b825a0aaa5746605037fc761033187d2e8abeb5cba8982e06cc28"
    );

    // ISO8601/RFC3339 규격의 날짜 포맷 검증
    let modified_at = payload["modified_at"]
        .as_str()
        .expect("modified_at should be a string");
    OffsetDateTime::parse(modified_at, &Rfc3339).expect("modified_at should be RFC3339");

    // 실제로 디스크 물리 파일이 작성되었고 내용이 동일한지 파일 시스템 수준에서 더블 체크합니다.
    assert_eq!(
        fs::read_to_string(vault.path().join("프로젝트/지식 노트.md"))
            .expect("created file should be readable"),
        "# 지식\n"
    );
}

/// 파일의 내용 크기가 아예 없는 빈 파일(0바이트)이거나, 설정한 최대 바이트 제한과 정확히 일치하는 경계선 크기일 때
/// API가 정상적으로 파일 생성을 승인하고 201 Created를 리턴하는지 한계치(Boundary Condition) 테스트를 진행합니다.
#[tokio::test]
async fn accepts_empty_and_exact_limit_content() {
    let vault = TempDir::new().expect("temporary Vault should be created");
    let maximum = 5 * 1024 * 1024; // 5MiB 제한

    // Case A: 빈 내용 파일 생성 시도 -> 승인되어야 함
    let (empty_status, _) = create_request(
        &vault,
        json!({ "path": "empty.md", "content": "" }),
        maximum,
    )
    .await;

    // Case B: 정확히 5MiB 크기의 파일 생성 시도 -> 승인되어야 함
    let (exact_status, exact_payload) = create_request(
        &vault,
        json!({
            "path": "exact.md",
            "content": "a".repeat(usize::try_from(maximum).expect("test limit should fit usize"))
        }),
        maximum,
    )
    .await;

    assert_eq!(empty_status, StatusCode::CREATED);
    assert_eq!(exact_status, StatusCode::CREATED);
    assert_eq!(exact_payload["size"], maximum);
}

/// 신규 파일 생성 시 발생하는 다양한 오류 유스케이스들(이미 존재하는 파일 덮어쓰기 금지, 부모 경로 부재, 잘못된 파일 확장자, 상위 경로 탈출 위협 등)이
/// 알맞은 HTTP 상태 코드 및 미리 약속된 에러 식별 키("error.code")로 매핑되는지 검증합니다.
#[tokio::test]
async fn maps_create_validation_and_conflict_errors() {
    // 1. 예외 테스트 환경을 구성하기 위해 임시 디렉터리 내에 물리 구조를 사전에 마련합니다.
    let vault = TempDir::new().expect("temporary Vault should be created");
    // (a) 이미 존재하는 파일 생성 테스트용
    fs::write(vault.path().join("existing.md"), "original").expect("file should be written");
    // (b) 이미 동명의 폴더가 존재하는 곳에 파일 쓰기 테스트용
    fs::create_dir(vault.path().join("directory.md")).expect("directory should be created");
    // (c) 부모 노드가 디렉터리가 아닌 일반 파일인 상황 테스트용
    fs::write(vault.path().join("parent-file"), "not a directory")
        .expect("parent file should be written");

    // 2. 테스트 케이스 정의 테이블: [생성 시도 경로, 기대하는 HTTP 상태, 기대하는 에러 코드 명칭]
    let cases = [
        // 이미 파일이 존재하여 덮어쓰기 충돌이 난 경우 -> 409 Conflict & "file_already_exists"
        ("existing.md", StatusCode::CONFLICT, "file_already_exists"),
        // 이미 폴더가 존재하여 충돌이 난 경우 -> 409 Conflict & "file_already_exists"
        ("directory.md", StatusCode::CONFLICT, "file_already_exists"),
        // 부모 디렉터리 자체가 존재하지 않는 경우 -> 404 Not Found & "parent_not_found"
        ("missing/note.md", StatusCode::NOT_FOUND, "parent_not_found"),
        // 부모 경로 지점에 디렉터리가 아니라 일반 파일이 버티고 있는 경우 -> 422 Unprocessable Entity & "parent_not_directory"
        (
            "parent-file/note.md",
            StatusCode::UNPROCESSABLE_ENTITY,
            "parent_not_directory",
        ),
        // 상위 경로 지시자(..)를 넣어 Vault 영역 탈출을 꾀한 경우 -> 400 Bad Request & "invalid_path"
        ("../secret.md", StatusCode::BAD_REQUEST, "invalid_path"),
        // 숨김 속성 파일 생성을 시도한 경우 -> 400 Bad Request & "invalid_path"
        (".private.md", StatusCode::BAD_REQUEST, "invalid_path"),
        // 확장자가 대문자 마크다운인 경우 거부 -> 422 Unprocessable Entity & "not_a_markdown_file"
        (
            "README.MD", // "README.MD"
            StatusCode::UNPROCESSABLE_ENTITY,
            "not_a_markdown_file",
        ),
        // 마크다운 확장자가 아닌 경우 거부 -> 422 Unprocessable Entity & "not_a_markdown_file"
        (
            "note.txt",
            StatusCode::UNPROCESSABLE_ENTITY,
            "not_a_markdown_file",
        ),
    ];

    // 3. 루프를 돌며 각 예외 케이스별 대응 코드가 정상 작동하는지 일괄 검증합니다.
    for (path, expected_status, expected_code) in cases {
        let (status, payload) =
            create_request(&vault, json!({ "path": path, "content": "content" }), 1024).await;
        assert_eq!(status, expected_status, "unexpected status for {path}");
        assert_eq!(
            payload["error"]["code"], expected_code,
            "unexpected code for {path}"
        );
    }

    // 덮어쓰기 거부 테스트 이후, 기존의 원본 파일이 손상 없이 보존되었는지 최종 점검합니다.
    assert_eq!(
        fs::read_to_string(vault.path().join("existing.md"))
            .expect("existing file should remain readable"),
        "original"
    );
}

/// 설정 한도를 넘는 크기 제한 테스트, JSON 형태가 망가진 비정상 바디 요청(Bad Request),
/// 그리고 과도한 크기의 요청 바디 버퍼링 시도를 수신 단계에서 즉시 차단(Payload Too Large)하는지 검증합니다.
#[tokio::test]
async fn rejects_large_content_malformed_json_and_oversized_body() {
    let vault = TempDir::new().expect("temporary Vault should be created");

    // Case A: 허용치(8바이트)보다 큰 9바이트 내용을 담아 마크다운을 전송했을 때 -> 413 Payload Too Large
    let (content_status, content_payload) = create_request(
        &vault,
        json!({ "path": "large.md", "content": "123456789" }),
        8,
    )
    .await;

    // Case B: JSON 포맷 자체를 깨뜨려 전송했을 때 -> 400 Bad Request
    let (json_status, json_payload) = raw_request(&vault, b"{invalid".to_vec(), 8).await;

    // Case C: JSON 바디 크기 자체가 시스템 기본 버퍼 임계값을 아득히 초과하여 전송했을 때 -> 413 Payload Too Large
    let (body_status, body_payload) = raw_request(&vault, vec![b'a'; 70_000], 8).await;

    assert_eq!(content_status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(content_payload["error"]["code"], "file_too_large");
    assert_eq!(json_status, StatusCode::BAD_REQUEST);
    assert_eq!(json_payload["error"]["code"], "invalid_request");
    assert_eq!(body_status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(body_payload["error"]["code"], "file_too_large");
}

/// [Unix/Linux 전용] Vault 영역 내에 설치된 폴더형 심볼릭 링크를 경유해 외부 디렉터리에 신규 마크다운 파일 생성을 꾀할 때,
/// 보안 검증 필터에 차단(403 Forbidden)되며, 에러 메시지 상에 서버의 물리 절대 경로가 유출되지 않는지 검증합니다.
#[cfg(unix)]
#[tokio::test]
async fn rejects_descendant_symlinks_without_leaking_absolute_paths() {
    use std::os::unix::fs::symlink;

    // 1. 임시 Vault 및 영역 바깥의 별도 임시 격리 디렉터리(outside)를 준비합니다.
    let vault = TempDir::new().expect("temporary Vault should be created");
    let outside = TempDir::new().expect("outside directory should be created");

    // 2. Vault 내부에 외부 디렉터리를 가리키는 폴더 성격의 심볼릭 링크 "linked"를 인위적으로 생성합니다.
    symlink(outside.path(), vault.path().join("linked")).expect("symlink should be created");

    // 3. 해당 심볼릭 링크 폴더 하위에 "note.md"를 생성해달라는 HTTP 요청을 날립니다.
    let (status, payload) = create_request(
        &vault,
        json!({ "path": "linked/note.md", "content": "secret" }),
        1024,
    )
    .await;

    // 4. 보안 가드가 침투를 막았는지 단언합니다.
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(payload["error"]["code"], "path_not_allowed");

    // 에러 응답에 외부 임시 물리 경로(/tmp/...)가 섞여서 포함되지 않았는지 단언합니다.
    assert!(
        !payload
            .to_string()
            .contains(&outside.path().display().to_string())
    );
}
