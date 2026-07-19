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

fn encoded_query_path(path: &str) -> String {
    utf8_percent_encode(path, NON_ALPHANUMERIC).to_string()
}

async fn request(vault: &TempDir, path: Option<&str>) -> (StatusCode, Value) {
    let uri = path.map_or_else(
        || "/api/tree".to_owned(),
        |path| format!("/api/tree?path={}", encoded_query_path(path)),
    );
    let response = build_router(AppConfig::for_test(vault.path()))
        .expect("test Vault should initialize")
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("test request should be valid"),
        )
        .await
        .expect("tree API should respond");
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

fn assert_timestamp(value: &Value) {
    let timestamp = value.as_str().expect("modified_at should be a string");
    OffsetDateTime::parse(timestamp, &Rfc3339).expect("modified_at should be RFC3339");
    let milliseconds = timestamp
        .split_once('.')
        .and_then(|(_, fraction)| fraction.strip_suffix('Z'))
        .expect("modified_at should contain UTC fractional seconds");
    assert_eq!(milliseconds.len(), 3);
}

#[tokio::test]
async fn lists_root_depth_one_with_directory_first_stable_order() {
    let vault = TempDir::new().expect("temporary Vault should be created");
    fs::create_dir(vault.path().join("한글 폴더")).expect("directory should be created");
    fs::create_dir(vault.path().join("zeta")).expect("directory should be created");
    fs::create_dir(vault.path().join("_trash")).expect("directory should be created");
    fs::write(vault.path().join("b.md"), "12345").expect("Markdown should be written");
    fs::write(vault.path().join("A.md"), "abc").expect("Markdown should be written");
    fs::write(vault.path().join("zeta/nested.md"), "nested")
        .expect("nested Markdown should be written");

    let (status, payload) = request(&vault, None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(payload["path"], "");
    let entries = payload["entries"]
        .as_array()
        .expect("entries should be an array");
    let names = entries
        .iter()
        .map(|entry| entry["name"].as_str().expect("name should be a string"))
        .collect::<Vec<_>>();
    assert_eq!(names, ["_trash", "zeta", "한글 폴더", "A.md", "b.md"]);
    assert_eq!(entries[0]["type"], "directory");
    assert!(entries[0].get("size").is_none());
    assert_eq!(entries[3]["type"], "file");
    assert_eq!(entries[3]["size"], 3);
    assert_eq!(entries[4]["size"], 5);
    assert!(
        entries
            .iter()
            .all(|entry| entry["path"] != "zeta/nested.md")
    );
    for entry in entries {
        assert_timestamp(&entry["modified_at"]);
    }
}

#[tokio::test]
async fn treats_absent_and_empty_path_as_root_and_lists_encoded_nested_path() {
    let vault = TempDir::new().expect("temporary Vault should be created");
    fs::create_dir(vault.path().join("프로젝트 공간")).expect("directory should be created");
    fs::write(vault.path().join("프로젝트 공간/지식 노트.md"), "한글")
        .expect("Markdown should be written");

    let (absent_status, absent) = request(&vault, None).await;
    let (empty_status, empty) = request(&vault, Some("")).await;
    let (nested_status, nested) = request(&vault, Some("프로젝트 공간")).await;

    assert_eq!(absent_status, StatusCode::OK);
    assert_eq!(empty_status, StatusCode::OK);
    assert_eq!(absent, empty);
    assert_eq!(nested_status, StatusCode::OK);
    assert_eq!(nested["path"], "프로젝트 공간");
    assert_eq!(nested["entries"][0]["name"], "지식 노트.md");
    assert_eq!(nested["entries"][0]["path"], "프로젝트 공간/지식 노트.md");
    assert_eq!(nested["entries"][0]["size"], "한글".len() as u64);
}

#[cfg(unix)]
#[tokio::test]
async fn excludes_hidden_non_markdown_symlink_and_special_children() {
    use std::{os::unix::fs::symlink, process::Command};

    let vault = TempDir::new().expect("temporary Vault should be created");
    let outside = TempDir::new().expect("outside directory should be created");
    fs::create_dir(vault.path().join("visible")).expect("directory should be created");
    fs::create_dir(vault.path().join(".hidden")).expect("hidden directory should be created");
    fs::write(vault.path().join("note.md"), "note").expect("Markdown should be written");
    fs::write(vault.path().join(".private.md"), "private")
        .expect("hidden Markdown should be written");
    fs::write(vault.path().join("note.txt"), "text").expect("text file should be written");
    fs::write(vault.path().join("README.MD"), "upper")
        .expect("uppercase extension should be written");
    fs::write(outside.path().join("outside.md"), "outside")
        .expect("outside Markdown should be written");
    symlink(outside.path(), vault.path().join("linked-directory"))
        .expect("directory symlink should be created");
    symlink(
        outside.path().join("outside.md"),
        vault.path().join("linked.md"),
    )
    .expect("file symlink should be created");
    let fifo = vault.path().join("tree.pipe");
    let status = Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo should run");
    assert!(status.success(), "FIFO should be created");

    let (status, payload) = request(&vault, None).await;

    assert_eq!(status, StatusCode::OK);
    let names = payload["entries"]
        .as_array()
        .expect("entries should be an array")
        .iter()
        .map(|entry| entry["name"].as_str().expect("name should be a string"))
        .collect::<Vec<_>>();
    assert_eq!(names, ["visible", "note.md"]);
}

#[cfg(unix)]
#[tokio::test]
async fn maps_invalid_missing_file_and_symlink_targets_to_contract_errors() {
    use std::os::unix::fs::symlink;

    let vault = TempDir::new().expect("temporary Vault should be created");
    let outside = TempDir::new().expect("outside directory should be created");
    fs::write(vault.path().join("note.md"), "note").expect("Markdown should be written");
    symlink(outside.path(), vault.path().join("linked"))
        .expect("directory symlink should be created");

    let cases = [
        ("missing", StatusCode::NOT_FOUND, "directory_not_found"),
        (
            "note.md",
            StatusCode::UNPROCESSABLE_ENTITY,
            "not_a_directory",
        ),
        (
            "projects/../secret",
            StatusCode::BAD_REQUEST,
            "invalid_path",
        ),
        (".hidden", StatusCode::BAD_REQUEST, "invalid_path"),
        ("linked", StatusCode::FORBIDDEN, "path_not_allowed"),
    ];

    for (path, expected_status, expected_code) in cases {
        let (status, payload) = request(&vault, Some(path)).await;
        assert_eq!(status, expected_status, "unexpected status for {path}");
        assert_eq!(
            payload["error"]["code"], expected_code,
            "unexpected error code for {path}"
        );
    }
}
