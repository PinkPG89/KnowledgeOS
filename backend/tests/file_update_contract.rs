use std::fs;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tower::ServiceExt;

use knowledgeos_backend::{build_router, config::AppConfig};

/// 입력받은 문자열 콘텐츠의 SHA-256 해시 체크섬을 계산하여 "sha256:<hex>" 형식의 문자열로 반환하는 헬퍼 함수입니다.
fn content_hash(content: &str) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(content.as_bytes())))
}

/// 경로의 각 세그먼트(폴더명, 파일명 등)를 개별적으로 퍼센트 인코딩(Percent Encoding)한 뒤 다시 연결합니다.
/// URI 상에 한글이나 공백 등의 문자를 깨짐 없이 안전하게 전송하기 위한 필수 처리입니다.
fn encoded_path(path: &str) -> String {
    path.split('/')
        .map(|segment| utf8_percent_encode(segment, NON_ALPHANUMERIC).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

/// 테스트에 적합하도록 최대 파일 크기 제한을 적용한 가상 Axum HTTP 라우터 인스턴스를 빌드해 반환합니다.
fn router(vault: &TempDir, max_bytes: u64) -> Router {
    let mut config = AppConfig::for_test(vault.path());
    config.max_markdown_bytes = max_bytes;
    build_router(config).expect("test Vault should initialize")
}

/// 가상의 HTTP PUT 요청을 생성하여 특정 경로의 파일 수정을 요청하고 응답 결과를 파싱하여 반환받는 비동기 헬퍼 함수입니다.
///
/// * `router`: 테스트 대상 Axum 라우터 서비스 인스턴스
/// * `path`: 수정하고자 하는 파일의 상대 경로
/// * `payload`: 수정될 본문 내용 및 optimistic lock 검증을 위한 `base_hash`를 담은 JSON 구조체
async fn update_request(router: Router, path: &str, payload: Value) -> (StatusCode, Value) {
    // 1. tower::ServiceExt::oneshot을 경유해 실제 TCP 연결 없이 메모리상에서 PUT 요청을 흘려보냅니다.
    let response = router
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/files/{}", encoded_path(path)))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&payload).expect("JSON should encode"),
                ))
                .expect("test request should be valid"),
        )
        .await
        .expect("update-file API should respond");

    // 2. HTTP 상태 코드를 확보합니다.
    let status = response.status();

    // 3. 비동기식 스트림 응답 본문 데이터를 병합하여 바이트 배열로 가져옵니다.
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body should be readable")
        .to_bytes();

    // 4. 응답 본문을 JSON으로 파싱해 리턴합니다.
    let payload = serde_json::from_slice(&body).expect("response should be JSON");
    (status, payload)
}

/// 중첩된 한글 경로 하위의 마크다운 파일을 타겟으로 삼아 정상적인 `base_hash`를 동반해 수정을 요청했을 때,
/// 파일이 원활하게 수정(Atomic Replace)되며 200 OK 및 갱신된 파일 정보(해시, 크기, 시간 등)가 리턴되는지 검증합니다.
#[tokio::test]
async fn updates_nested_unicode_markdown_and_returns_the_new_document() {
    // 1. 임시 격리 Vault 디렉터리를 마련합니다.
    let vault = TempDir::new().expect("temporary Vault should be created");
    // 2. 부모 디렉터리 "프로젝트" 폴더를 사전에 생성해 둡니다.
    fs::create_dir(vault.path().join("프로젝트")).expect("parent should be created");
    // 3. 수정 대상이 될 최초의 "지식 노트.md" 파일을 작성해 놓습니다.
    fs::write(vault.path().join("프로젝트/지식 노트.md"), "original")
        .expect("original Markdown should be written");

    // 4. 가상 HTTP PUT 요청을 보내 업데이트를 시도합니다.
    let (status, payload) = update_request(
        router(&vault, 5 * 1024 * 1024),
        "프로젝트/지식 노트.md",
        json!({
            "content": "# 수정\n",
            // 최초 원본 내용("original")의 해시를 base_hash로 전달하여 낙관적 락 정합성을 맞춥니다.
            "base_hash": content_hash("original")
        }),
    )
    .await;

    // 5. API 스펙 계약 조건을 검증 단언합니다.
    assert_eq!(status, StatusCode::OK);
    assert_eq!(payload["path"], "프로젝트/지식 노트.md");
    assert_eq!(payload["content"], "# 수정\n");
    assert_eq!(payload["hash"], content_hash("# 수정\n"));
    assert_eq!(payload["size"], "# 수정\n".len() as u64);

    // RFC3339 규격 시간대 파싱 검사
    OffsetDateTime::parse(
        payload["modified_at"]
            .as_str()
            .expect("modified_at should be a string"),
        &Rfc3339,
    )
    .expect("modified_at should be RFC3339");

    // 실제로 디스크 상의 파일 내용이 온전하게 변경되었는지 최종 검사합니다.
    assert_eq!(
        fs::read_to_string(vault.path().join("프로젝트/지식 노트.md"))
            .expect("updated Markdown should be readable"),
        "# 수정\n"
    );
}

/// 클라이언트가 읽어간 시점 이후에 다른 사람에 의해 파일 내용이 이미 바뀌어 해시 정합성이 깨진 경우(낙관적 동시성 락 충돌),
/// 수정을 안전하게 거부(409 Conflict)하고 원본 데이터를 오염시키지 않는지 검증합니다.
#[tokio::test]
async fn stale_hash_returns_conflict_and_preserves_the_original() {
    // 1. 임시 Vault를 열고 현재 시점의 내용으로 "current"를 작성합니다.
    let vault = TempDir::new().expect("temporary Vault should be created");
    fs::write(vault.path().join("note.md"), "current").expect("current Markdown should be written");

    // 2. 이미 만료된(stale) 이전 해시값("stale" 해시)을 base_hash로 조작해 PUT 요청을 전송해 봅니다.
    let (status, payload) = update_request(
        router(&vault, 1024),
        "note.md",
        json!({ "content": "replacement", "base_hash": content_hash("stale") }),
    )
    .await;

    // 3. 동시성 충돌 방어 로직이 제대로 통했는지 검증합니다.
    // - 409 Conflict 상태 코드가 돌아왔는가?
    assert_eq!(status, StatusCode::CONFLICT);
    // - 에러 식별 코드가 "write_conflict"인가?
    assert_eq!(payload["error"]["code"], "write_conflict");
    // - 현재 디바이스의 실제 최신 해시 정보를 JSON details를 통해 돌려주어 클라이언트가 재수정 분기를 밟을 수 있게 돕는가?
    assert_eq!(
        payload["error"]["details"]["current_hash"],
        content_hash("current")
    );
    // - 가장 핵심적으로, 디스크 상의 파일이 replacement로 덮어씌워지지 않고 여전히 "current"로 완벽 보존되고 있는가?
    assert_eq!(
        fs::read_to_string(vault.path().join("note.md"))
            .expect("current Markdown should remain readable"),
        "current"
    );
}

/// 파일 업데이트 과정에서 일어날 수 있는 입력 유효성 및 대상 객체 접근 예외 상황들이
/// 미리 규격화해 둔 에러 코드와 알맞은 HTTP 상태 코드로 적절하게 매핑되어 반환되는지 확인합니다.
#[tokio::test]
async fn maps_update_validation_and_target_errors() {
    // 1. 테스트 목적에 맞춰 사전 파일 인프라 상태를 구성합니다.
    let vault = TempDir::new().expect("temporary Vault should be created");
    // (a) 정상 타겟 파일 마련
    fs::write(vault.path().join("note.md"), "current").expect("file should be written");
    // (b) 디렉터리 경로 노드
    fs::create_dir(vault.path().join("directory.md")).expect("directory should be created");
    // (c) 유효하지 않은 인코딩 파일
    fs::write(vault.path().join("invalid.md"), [0xff, 0xfe])
        .expect("invalid UTF-8 should be written");

    // "current" 내용물과 호환되는 정상 해시를 기준으로 삼아 예외 필터를 가동합니다.
    let valid_hash = content_hash("current");

    // 2. 테스트 케이스 정의 테이블: [수정 대상 경로, 제공할 base_hash, 기대하는 HTTP 상태, 기대하는 에러 코드 명칭]
    let cases = [
        // 존재하지 않는 파일에 쓰려 할 때 -> 404 Not Found & "file_not_found"
        (
            "missing.md",
            valid_hash.as_str(),
            StatusCode::NOT_FOUND,
            "file_not_found",
        ),
        // 파일이 아닌 디렉터리를 업데이트하려 할 때 -> 422 Unprocessable Entity & "not_a_regular_file"
        (
            "directory.md",
            valid_hash.as_str(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "not_a_regular_file",
        ),
        // UTF-8 포맷이 깨진 바이너리 덤프 대상을 덮어쓰려 할 때 -> 422 Unprocessable Entity & "invalid_utf8"
        (
            "invalid.md",
            valid_hash.as_str(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_utf8",
        ),
        // 디렉터리 트래버설을 통해 임시 Vault 밖의 민감 자리에 업데이트를 시도할 때 -> 400 Bad Request & "invalid_path"
        (
            "../secret.md",
            valid_hash.as_str(),
            StatusCode::BAD_REQUEST,
            "invalid_path",
        ),
        // 올바른 마크다운 확장자명이 아닌 경로에 업데이트를 시도할 때 -> 422 Unprocessable Entity & "not_a_markdown_file"
        (
            "README.MD",
            valid_hash.as_str(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "not_a_markdown_file",
        ),
        // 클라이언트가 이상한 포맷의 해시 형식을 base_hash 인자로 제공했을 때 -> 400 Bad Request & "invalid_base_hash"
        (
            "note.md",
            "SHA256:invalid",
            StatusCode::BAD_REQUEST,
            "invalid_base_hash",
        ),
    ];

    // 3. 각 실패 케이스별 응답 대응 유효성을 단언합니다.
    for (path, base_hash, expected_status, expected_code) in cases {
        let (status, payload) = update_request(
            router(&vault, 1024),
            path,
            json!({ "content": "replacement", "base_hash": base_hash }),
        )
        .await;
        assert_eq!(status, expected_status, "unexpected status for {path}");
        assert_eq!(
            payload["error"]["code"], expected_code,
            "unexpected code for {path}"
        );
    }
}

/// 업데이트 요청에 실린 내용 크기가 서버에서 규정한 단일 파일 최대 용량(8바이트) 한도를 초과하는 경우,
/// 디스크에 새 파일(임시파일)을 쓰고 덮어씌우는 일련의 쓰기 트랜잭션 절차에 진입하기 앞서
/// 조기에 수신 차단(413 Payload Too Large)하고, 원본 파일 데이터를 오염 없이 지켜내는지 검증합니다.
#[tokio::test]
async fn rejects_oversized_content_before_replacing_the_original() {
    let vault = TempDir::new().expect("temporary Vault should be created");
    fs::write(vault.path().join("note.md"), "current").expect("current Markdown should be written");

    // 9바이트짜리 대용량(?) 수정 요청을 최대 8바이트 제한 서버에 날려봅니다.
    let (status, payload) = update_request(
        router(&vault, 8),
        "note.md",
        json!({ "content": "123456789", "base_hash": content_hash("current") }),
    )
    .await;

    // 용량 제한 가드가 정상적으로 파일 오염을 선제 차단했는지 검증합니다.
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(payload["error"]["code"], "file_too_large");
    assert_eq!(
        fs::read_to_string(vault.path().join("note.md"))
            .expect("current Markdown should remain readable"),
        "current"
    );
}

/// 동일한 `base_hash`를 바라보고 있는 두 명의 사용자(또는 AI 에이전트 스레드)가
/// 디바이스 내의 동일 파일에 동시에 비동기 PUT 업데이트 요청을 경합(Race Condition)하듯 밀어 넣었을 때,
/// 트랜잭션의 원자성(Atomicity)이 지켜져 한 명만 수정을 성공(200 OK)하고, 다른 한 명은 정합성 에러(409 Conflict)로 낙담 처리되는지 확인합니다.
#[tokio::test]
async fn concurrent_updates_with_one_base_hash_allow_only_one_success() {
    // 1. 임시 Vault를 구성하고 공통 원본 파일("original")을 작성합니다.
    let vault = TempDir::new().expect("temporary Vault should be created");
    fs::write(vault.path().join("note.md"), "original")
        .expect("original Markdown should be written");

    // 2. 동시 비동기 처리를 테스트하기 위해 라우터와 최초 원본 해시값을 준비합니다.
    let router = router(&vault, 1024);
    let base_hash = content_hash("original");

    // 3. tokio::join! 매크로에 비동기 태스크 2개를 묶어 공급해 동시에 비동기 IO 연산을 경유하도록 실행합니다.
    let first = update_request(
        router.clone(),
        "note.md",
        json!({ "content": "first", "base_hash": base_hash }),
    );
    let second = update_request(
        router,
        "note.md",
        json!({ "content": "second", "base_hash": content_hash("original") }),
    );

    // 두 요청의 처리 완료 응답 결과를 동시에 획득합니다.
    let (first_result, second_result) = tokio::join!(first, second);
    let statuses = [first_result.0, second_result.0];

    // 4. 둘 중의 하나(승리한 트랜잭션)는 무조건 성공(200 OK)해야 합니다.
    assert_eq!(
        statuses
            .iter()
            .filter(|status| **status == StatusCode::OK)
            .count(),
        1
    );
    // 5. 나머지 한 명(패배한 트랜잭션)은 base_hash가 중간에 변경되었음을 포착하고 반드시 실패(409 Conflict)를 돌려받아야 합니다.
    assert_eq!(
        statuses
            .iter()
            .filter(|status| **status == StatusCode::CONFLICT)
            .count(),
        1
    );
}

/// [Unix/Linux 전용] Vault 디렉터리 내에 심볼릭 링크 파일을 조작 배치해놓고 파일 수정을 요청했을 때,
/// 보안 가드가 우회 침투 시도를 적발해 403 Forbidden 상태 코드로 안전하게 거부하고,
/// 에러 디버깅 정보에 Vault 외부의 물리 기기 절대 경로가 기재되어 밖으로 노출되지 않는지 확인합니다.
#[cfg(unix)]
#[tokio::test]
async fn rejects_descendant_symlinks_without_leaking_absolute_paths() {
    use std::os::unix::fs::symlink;

    // 1. 임시 Vault 및 바깥 영역 격리 디렉터리(outside)를 생성합니다.
    let vault = TempDir::new().expect("temporary Vault should be created");
    let outside = TempDir::new().expect("outside directory should be created");

    // 2. Vault 바깥 공간에 중요 비밀번호 등 민감 정보를 암시하는 파일을 미리 작성합니다.
    let outside_file = outside.path().join("secret.md");
    fs::write(&outside_file, "secret").expect("outside file should be written");

    // 3. Vault 내부에 바깥의 민감 파일을 가리키는 심볼릭 링크 "linked.md"를 인위적으로 생성합니다.
    symlink(&outside_file, vault.path().join("linked.md")).expect("symlink should be created");

    // 4. 해당 심볼릭 링크 파일을 타겟으로 수정 PUT 요청을 날립니다.
    let (status, payload) = update_request(
        router(&vault, 1024),
        "linked.md",
        json!({ "content": "replacement", "base_hash": content_hash("secret") }),
    )
    .await;

    // 5. 보안 가드가 의도대로 침투를 차단했는지 단언합니다.
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(payload["error"]["code"], "path_not_allowed");

    // 에러 데이터 내에 바깥의 실제 물리 절대 경로가 유출되지 않았는지 단언합니다.
    assert!(
        !payload
            .to_string()
            .contains(&outside.path().display().to_string())
    );
}
