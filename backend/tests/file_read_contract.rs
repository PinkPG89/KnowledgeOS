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

fn encoded_path(path: &str) -> String {
    path.split('/')
        .map(|segment| utf8_percent_encode(segment, NON_ALPHANUMERIC).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

async fn request(vault: &TempDir, path: &str, max_bytes: u64) -> (StatusCode, Value) {
    let mut config = AppConfig::for_test(vault.path());
    config.max_markdown_bytes = max_bytes;
    let response = build_router(config)
        .expect("test Vault should initialize")
        .oneshot(
            Request::builder()
                .uri(format!("/api/files/{}", encoded_path(path)))
                .body(Body::empty())
                .expect("test request should be valid"),
        )
        .await
        .expect("file API should respond");
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body should be readable")
        .to_bytes();
    let payload = serde_json::from_slice(&body).expect("response should be JSON");
    (status, payload)
}

#[tokio::test]
async fn reads_nested_percent_encoded_unicode_markdown() {
    let vault = TempDir::new().expect("temporary Vault should be created");
    let directory = vault.path().join("프로젝트");
    fs::create_dir(&directory).expect("nested directory should be created");
    fs::write(directory.join("지식 노트.md"), "# 지식\n").expect("Markdown should be written");

    let (status, payload) = request(&vault, "프로젝트/지식 노트.md", 5 * 1024 * 1024).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(payload["path"], "프로젝트/지식 노트.md");
    assert_eq!(payload["content"], "# 지식\n");
    assert_eq!(payload["size"], "# 지식\n".len() as u64);
    let hash = payload["hash"].as_str().expect("hash should be a string");
    assert!(hash.starts_with("sha256:"));
    assert_eq!(hash.len(), "sha256:".len() + 64);
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

#[tokio::test]
async fn maps_read_failures_to_public_error_codes() {
    let vault = TempDir::new().expect("temporary Vault should be created");
    fs::create_dir(vault.path().join("directory.md")).expect("directory should be created");
    fs::write(vault.path().join("invalid.md"), [0xff, 0xfe])
        .expect("invalid UTF-8 should be written");
    fs::write(vault.path().join("large.md"), "123456789")
        .expect("large Markdown should be written");

    let cases = [
        ("missing.md", 1024, StatusCode::NOT_FOUND, "file_not_found"),
        (
            "directory.md",
            1024,
            StatusCode::UNPROCESSABLE_ENTITY,
            "not_a_regular_file",
        ),
        (
            "invalid.md",
            1024,
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_utf8",
        ),
        (
            "large.md",
            8,
            StatusCode::PAYLOAD_TOO_LARGE,
            "file_too_large",
        ),
        (
            "note.txt",
            1024,
            StatusCode::UNPROCESSABLE_ENTITY,
            "not_a_markdown_file",
        ),
        (
            "README.MD",
            1024,
            StatusCode::UNPROCESSABLE_ENTITY,
            "not_a_markdown_file",
        ),
        (".private.md", 1024, StatusCode::BAD_REQUEST, "invalid_path"),
        (
            "projects/../secret.md",
            1024,
            StatusCode::BAD_REQUEST,
            "invalid_path",
        ),
    ];

    for (path, maximum, expected_status, expected_code) in cases {
        let (status, payload) = request(&vault, path, maximum).await;
        assert_eq!(status, expected_status, "unexpected status for {path}");
        assert_eq!(
            payload["error"]["code"], expected_code,
            "unexpected code for {path}"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn rejects_descendant_symlinks_without_leaking_absolute_paths() {
    use std::os::unix::fs::symlink;

    let vault = TempDir::new().expect("temporary Vault should be created");
    let outside = TempDir::new().expect("outside directory should be created");
    let outside_file = outside.path().join("secret.md");
    fs::write(&outside_file, "secret").expect("outside file should be written");
    symlink(&outside_file, vault.path().join("linked.md")).expect("symlink should be created");

    let (status, payload) = request(&vault, "linked.md", 1024).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(payload["error"]["code"], "path_not_allowed");
    assert!(
        !payload
            .to_string()
            .contains(&outside.path().display().to_string())
    );
}
