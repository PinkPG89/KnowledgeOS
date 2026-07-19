use std::fs;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::Value;
use tempfile::TempDir;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tower::ServiceExt;

use knowledgeos_backend::{build_router, config::AppConfig};

/// 경로의 각 세그먼트(폴더명, 파일명 등)를 개별적으로 퍼센트 인코딩(Percent Encoding)한 뒤 다시 연결합니다.
///
/// ## 퍼센트 인코딩이 필요한 이유
/// HTTP URI 경로 상에서 한글이나 공백 같은 비-ASCII 문자를 안전하게 전송하기 위해 퍼센트 인코딩이 적용되어야 합니다.
/// 단, 경로 구분자인 슬래시('/')까지 함께 인코딩해버리면 Axum 라우터가 경로 세그먼트를 제대로 구분하지 못하므로,
/// 슬래시('/')를 기준으로 문자열을 분할(split)하여 개별 세그먼트만 인코딩한 후 다시 슬래시로 병합(join)합니다.
fn encoded_path(path: &str) -> String {
    path.split('/')
        .map(|segment| utf8_percent_encode(segment, NON_ALPHANUMERIC).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

/// 가상의 HTTP GET 요청을 생성하여 메모리 내에서 파일 읽기 API 엔드포인트를 호출하고 응답을 반환받는 비동기 헬퍼 함수입니다.
///
/// * `vault`: 테스트용으로 사용할 임시 저장소 디렉터리 핸들
/// * `path`: 조회하려는 파일의 상대 경로
/// * `max_bytes`: 테스트에 적용할 단일 마크다운 파일의 최대 허용 바이트 크기
async fn request(vault: &TempDir, path: &str, max_bytes: u64) -> (StatusCode, Value) {
    // 1. 테스트를 위한 모의(Mock) 설정을 준비하고 허용 크기를 세팅합니다.
    let mut config = AppConfig::for_test(vault.path());
    config.max_markdown_bytes = max_bytes;

    // 2. build_router를 통해 HTTP 라우터를 구동 준비 상태로 빌드합니다.
    let response = build_router(config)
        .expect("test Vault should initialize")
        // 3. tower::ServiceExt::oneshot을 사용해 실제 TCP 포트를 열지 않고,
        //    가상의 HTTP GET 요청을 라우터에 주입하여 단발성으로 처리한 후 결과를 받습니다.
        .oneshot(
            Request::builder()
                .uri(format!("/api/files/{}", encoded_path(path)))
                .body(Body::empty())
                .expect("test request should be valid"),
        )
        .await
        .expect("file API should respond");

    // 4. 응답 객체로부터 HTTP 상태 코드를 획득합니다.
    let status = response.status();

    // 5. 비동기 스트림 바디의 모든 데이터를 모아서 바이트 배열로 변환합니다.
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body should be readable")
        .to_bytes();

    // 6. 획득한 응답 바디를 JSON 파서(serde_json)로 역직렬화하여 결과 객체를 반환합니다.
    let payload = serde_json::from_slice(&body).expect("response should be JSON");
    (status, payload)
}

/// 한글 경로명 및 공백이 포함된 마크다운 파일을 퍼센트 인코딩 상태로 호출했을 때,
/// 파일이 성공적으로 읽히고 문서 메타데이터(크기, 해시, 수정 시각 등)가 공개 계약 규격에 맞춰 JSON 응답으로 오는지 검증합니다.
#[tokio::test]
async fn reads_nested_percent_encoded_unicode_markdown() {
    // 1. 테스트용 임시 디렉터리(Vault)를 격리 공간에 생성합니다.
    let vault = TempDir::new().expect("temporary Vault should be created");

    // 2. 한글 디렉터리 "프로젝트"를 생성하고 그 안에 "지식 노트.md" 파일을 작성합니다.
    let directory = vault.path().join("프로젝트");
    fs::create_dir(&directory).expect("nested directory should be created");
    fs::write(directory.join("지식 노트.md"), "# 지식\n").expect("Markdown should be written");

    // 3. 가상 HTTP 요청을 날려 200 OK 응답과 JSON 바디를 받아옵니다.
    let (status, payload) = request(&vault, "프로젝트/지식 노트.md", 5 * 1024 * 1024).await;

    // 4. 계약(Contract)이 준수되었는지 검증 단언(Assert)을 진행합니다.
    assert_eq!(status, StatusCode::OK);
    assert_eq!(payload["path"], "프로젝트/지식 노트.md");
    assert_eq!(payload["content"], "# 지식\n");
    assert_eq!(payload["size"], "# 지식\n".len() as u64);

    // 해시 포맷 검증 ("sha256:" 접두사 및 64글자 16진수 문자열 조합 확인)
    let hash = payload["hash"].as_str().expect("hash should be a string");
    assert!(hash.starts_with("sha256:"));
    assert_eq!(hash.len(), "sha256:".len() + 64);

    // 수정 시각 포맷 검증 (RFC3339 표준 준수 여부 및 밀리초 단위 3자리 정밀도 여부 체크)
    let modified_at = payload["modified_at"]
        .as_str()
        .expect("modified_at should be a string");
    OffsetDateTime::parse(modified_at, &Rfc3339).expect("modified_at should be RFC3339");
    let milliseconds = modified_at
        .split_once('.')
        .and_then(|(_, fraction)| fraction.strip_suffix('Z'))
        .expect("modified_at should contain UTC fractional seconds");
    assert_eq!(milliseconds.len(), 3);
}

/// 다양한 비정상 요청 상황(존재하지 않는 파일, 폴더 접근 시도, 깨진 인코딩, 용량 초과, 잘못된 확장자, 상위 경로 탈출 공격 등)에서
/// 외부 시스템 계약으로 사전에 정의된 적절한 HTTP 상태 코드와 에러 코드 메시지(error.code)로 정확하게 매핑되는지 검증합니다.
#[tokio::test]
async fn maps_read_failures_to_public_error_codes() {
    // 1. 예외 테스트 환경을 구성하기 위해 다양한 성격의 파일/디렉터리를 임시 Vault 내에 세팅합니다.
    let vault = TempDir::new().expect("temporary Vault should be created");
    // (a) 폴더인데 .md 확장자를 가진 노드 생성 (Not a Regular File 테스트용)
    fs::create_dir(vault.path().join("directory.md")).expect("directory should be created");
    // (b) 유효하지 않은 UTF-8 바이트 시퀀스를 담은 텍스트 생성
    fs::write(vault.path().join("invalid.md"), [0xff, 0xfe])
        .expect("invalid UTF-8 should be written");
    // (c) 설정한 한계치보다 큰 파일 생성
    fs::write(vault.path().join("large.md"), "123456789")
        .expect("large Markdown should be written");

    // 2. 테스트 케이스 정의 테이블: [요청 경로, 최대 허용 용량, 기대하는 HTTP 상태, 기대하는 에러 코드 명칭]
    let cases = [
        // 존재하지 않는 파일 조회 시 -> 404 Not Found & "file_not_found"
        ("missing.md", 1024, StatusCode::NOT_FOUND, "file_not_found"),
        // 폴더를 파일로써 조회 시 -> 422 Unprocessable Entity & "not_a_regular_file"
        (
            "directory.md",
            1024,
            StatusCode::UNPROCESSABLE_ENTITY,
            "not_a_regular_file",
        ),
        // UTF-8 파싱 불가능한 바이너리 조회 시 -> 422 Unprocessable Entity & "invalid_utf8"
        (
            "invalid.md",
            1024,
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_utf8",
        ),
        // 허용 바이트 크기 한도를 넘는 대용량 파일 조회 시 -> 413 Payload Too Large & "file_too_large"
        (
            "large.md",
            8,
            StatusCode::PAYLOAD_TOO_LARGE,
            "file_too_large",
        ),
        // 마크다운(.md)이 아닌 텍스트(.txt) 파일 조회 시 -> 422 Unprocessable Entity & "not_a_markdown_file"
        (
            "note.txt",
            1024,
            StatusCode::UNPROCESSABLE_ENTITY,
            "not_a_markdown_file",
        ),
        // 확장자가 대문자인 경우 규격 위반 거부 -> 422 Unprocessable Entity & "not_a_markdown_file"
        (
            "README.MD",
            1024,
            StatusCode::UNPROCESSABLE_ENTITY,
            "not_a_markdown_file",
        ),
        // 점(.)으로 시작하는 숨겨진 파일 접근 시도 차단 -> 400 Bad Request & "invalid_path"
        (".private.md", 1024, StatusCode::BAD_REQUEST, "invalid_path"),
        // 상위 경로 지시자(..)를 통한 디렉터리 탈출 공격(Directory Traversal) 차단 -> 400 Bad Request & "invalid_path"
        (
            "projects/../secret.md",
            1024,
            StatusCode::BAD_REQUEST,
            "invalid_path",
        ),
    ];

    // 3. 루프를 돌며 각 예외 케이스별 대응 코드가 정상 작동하는지 일괄 검증합니다.
    for (path, maximum, expected_status, expected_code) in cases {
        let (status, payload) = request(&vault, path, maximum).await;
        assert_eq!(status, expected_status, "unexpected status for {path}");
        assert_eq!(
            payload["error"]["code"], expected_code,
            "unexpected code for {path}"
        );
    }
}

/// [Unix/Linux 전용] Vault 영역 내에 심볼릭 링크를 설치하고 이를 경유해 외부 파일 조회를 시도했을 때,
/// Vault 보안 컨테이너 가드가 작동하여 접근을 금지(403 Forbidden)하고, 에러 응답에 외부 물리 절대 경로 정보가 노출되지 않는지 검증합니다.
#[cfg(unix)]
#[tokio::test]
async fn rejects_descendant_symlinks_without_leaking_absolute_paths() {
    use std::os::unix::fs::symlink;

    // 1. 임시 Vault 및 영역 바깥의 별도 임시 격리 디렉터리(outside)를 생성합니다.
    let vault = TempDir::new().expect("temporary Vault should be created");
    let outside = TempDir::new().expect("outside directory should be created");

    // 2. Vault 바깥 공간에 중요 비밀번호 등 민감 정보를 암시하는 파일을 작성합니다.
    let outside_file = outside.path().join("secret.md");
    fs::write(&outside_file, "secret").expect("outside file should be written");

    // 3. Vault 내부에 바깥의 민감 파일을 가리키는 심볼릭 링크 "linked.md"를 인위적으로 생성합니다.
    symlink(&outside_file, vault.path().join("linked.md")).expect("symlink should be created");

    // 4. 해당 심볼릭 링크 파일 조회를 가상 HTTP로 시도합니다.
    let (status, payload) = request(&vault, "linked.md", 1024).await;

    // 5. 보안 격리가 의도대로 작동했는지 단언(Assert)합니다.
    // - 403 Forbidden 상태 코드로 안전하게 거부되었는가?
    assert_eq!(status, StatusCode::FORBIDDEN);
    // - 에러 식별 코드가 "path_not_allowed"인가?
    assert_eq!(payload["error"]["code"], "path_not_allowed");
    // - 사용자 응답 JSON 내에 바깥의 실제 물리 절대 경로(예: /tmp/...)가 섞여서 누출(Information Disclosure)되지 않았는가?
    assert!(
        !payload
            .to_string()
            .contains(&outside.path().display().to_string())
    );
}
