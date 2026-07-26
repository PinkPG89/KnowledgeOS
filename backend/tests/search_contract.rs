use std::fs;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

use knowledgeos_backend::{build_router, config::AppConfig};

fn encode(value: &str) -> String {
    utf8_percent_encode(value, NON_ALPHANUMERIC).to_string()
}

async fn request(config: AppConfig, query: &str) -> (StatusCode, Value) {
    let response = build_router(config)
        .expect("test Vault should initialize")
        .oneshot(
            Request::builder()
                .uri(format!("/api/search?{query}"))
                .body(Body::empty())
                .expect("test request should be valid"),
        )
        .await
        .expect("search API should respond");
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
async fn searches_title_body_and_tags_with_plain_snippets_and_scores() {
    let vault = TempDir::new().expect("temporary Vault should be created");
    fs::write(
        vault.path().join("title.md"),
        "# Architecture\nunrelated body",
    )
    .expect("title fixture should be written");
    fs::write(
        vault.path().join("body.md"),
        "# Other\nfilesystem architecture for KnowledgeOS",
    )
    .expect("body fixture should be written");
    fs::write(
        vault.path().join("tag.md"),
        "---\ntags: [architecture]\n---\n# Tagged\nmetadata result",
    )
    .expect("tag fixture should be written");

    let (status, payload) = request(
        AppConfig::for_test(vault.path()),
        &format!("q={}", encode("architecture")),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(payload["query"], "architecture");
    let results = payload["results"]
        .as_array()
        .expect("results should be an array");
    assert_eq!(results.len(), 3);
    assert_eq!(results[0]["path"], "title.md");
    assert_eq!(results[0]["title"], "Architecture");
    assert!(
        results
            .iter()
            .all(|result| result["score"].as_f64().is_some_and(|score| score > 0.0))
    );
    assert!(results.iter().all(|result| {
        !result["snippet"]
            .as_str()
            .expect("snippet should be a string")
            .contains('<')
    }));
}

#[tokio::test]
async fn applies_canonical_path_prefix_limit_and_stable_ordering() {
    let vault = TempDir::new().expect("temporary Vault should be created");
    fs::create_dir_all(vault.path().join("projects/nested"))
        .expect("nested directory should be created");
    fs::write(vault.path().join("outside.md"), "# Match\nshared")
        .expect("outside fixture should be written");
    fs::write(vault.path().join("projects/b.md"), "# Match\nshared")
        .expect("project fixture should be written");
    fs::write(vault.path().join("projects/a.md"), "# Match\nshared")
        .expect("project fixture should be written");
    fs::write(vault.path().join("projects/nested/c.md"), "# Match\nshared")
        .expect("nested fixture should be written");

    let query = format!(
        "q={}&path_prefix={}&limit=2",
        encode("shared"),
        encode("projects")
    );
    let (status, payload) = request(AppConfig::for_test(vault.path()), &query).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(payload["path_prefix"], "projects");
    assert_eq!(payload["limit"], 2);
    assert_eq!(
        payload["results"]
            .as_array()
            .expect("results should be an array")
            .iter()
            .map(|result| result["path"].as_str().expect("path should be a string"))
            .collect::<Vec<_>>(),
        ["projects/a.md", "projects/b.md"]
    );
}

#[tokio::test]
async fn treats_fts_operators_quotes_and_punctuation_as_literal_input() {
    let vault = TempDir::new().expect("temporary Vault should be created");
    fs::write(
        vault.path().join("literal.md"),
        "# Literal\nOR unterminated hello",
    )
    .expect("literal fixture should be written");

    for query in [
        "OR",
        "\"unterminated",
        "hello OR",
        "hello -OR*",
        "(hello)",
        "*",
        "\"",
        "()",
        "-",
    ] {
        let (status, payload) = request(
            AppConfig::for_test(vault.path()),
            &format!("q={}", encode(query)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "query should be escaped: {query}");
        assert_eq!(payload["query"], query);
    }
}

#[tokio::test]
async fn validates_query_prefix_and_limit_with_json_errors() {
    let vault = TempDir::new().expect("temporary Vault should be created");
    let cases = [
        ("", "invalid_search_query"),
        ("q=%20%20", "invalid_search_query"),
        ("q=test&limit=0", "invalid_search_limit"),
        ("q=test&limit=101", "invalid_search_limit"),
        ("q=test&limit=many", "invalid_search_limit"),
        ("q=test&path_prefix=..", "invalid_path"),
    ];

    for (query, expected_code) in cases {
        let (status, payload) = request(AppConfig::for_test(vault.path()), query).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "query: {query}");
        assert_eq!(payload["error"]["code"], expected_code, "query: {query}");
    }

    let oversized = "가".repeat(180);
    let (status, payload) = request(
        AppConfig::for_test(vault.path()),
        &format!("q={}", encode(&oversized)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(payload["error"]["code"], "invalid_search_query");
}

#[tokio::test]
async fn reports_degraded_index_as_service_unavailable() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let vault = directory.path().join("vault");
    fs::create_dir(&vault).expect("Vault should be created");
    let state_file = directory.path().join("state-file");
    fs::write(&state_file, "not a directory").expect("state fixture should be written");
    let mut config = AppConfig::for_test(&vault);
    config.state_root = state_file;

    let (status, payload) = request(config, "q=test").await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        payload,
        json!({
            "error": {
                "code": "search_unavailable",
                "message": "Search index is unavailable"
            }
        })
    );
}
