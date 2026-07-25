use std::fs;

use rusqlite::{Connection, params};
use tempfile::TempDir;

use knowledgeos_backend::infrastructure::search_index::{
    SearchIndex, SearchIndexError, SearchIndexStatus,
};

#[test]
fn creates_versioned_schema_with_working_fts5() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let state_root = directory.path().join("state");
    let index = SearchIndex::open(&state_root).expect("search index should initialize");

    assert_eq!(
        index.status().expect("status should be readable"),
        SearchIndexStatus {
            schema_version: 1,
            document_count: 0,
        }
    );
    assert_eq!(
        index.database_path(),
        fs::canonicalize(&state_root)
            .expect("state root should canonicalize")
            .join("index.sqlite")
    );

    let connection =
        Connection::open(index.database_path()).expect("database should be directly inspectable");
    connection
        .execute(
            "INSERT INTO documents (
                path, title, body, content_hash, modified_at, indexed_at, frontmatter_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
            params![
                "projects/agent.md",
                "Agent 설계",
                "한글 지식 검색",
                format!("sha256:{}", "a".repeat(64)),
                "2026-07-24T01:02:03.004Z",
                "2026-07-24T01:02:04.004Z",
            ],
        )
        .expect("document projection should be insertable");
    connection
        .execute(
            "INSERT INTO search_documents (path, title, body, tags)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                "projects/agent.md",
                "Agent 설계",
                "한글 지식 검색",
                "knowledge rust"
            ],
        )
        .expect("FTS projection should be insertable");

    let matched_path: String = connection
        .query_row(
            "SELECT path FROM search_documents
             WHERE search_documents MATCH '지식'",
            [],
            |row| row.get(0),
        )
        .expect("FTS5 should match Unicode content");
    assert_eq!(matched_path, "projects/agent.md");
    assert_eq!(
        index.status().expect("status should count documents"),
        SearchIndexStatus {
            schema_version: 1,
            document_count: 1,
        }
    );
}

#[test]
fn replaces_an_index_with_an_incompatible_schema_version() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let index = SearchIndex::open(directory.path()).expect("search index should initialize");
    let connection =
        Connection::open(index.database_path()).expect("database should be directly inspectable");
    connection
        .execute(
            "INSERT INTO documents (
                path, title, body, content_hash, modified_at, indexed_at, frontmatter_json
             ) VALUES ('old.md', 'Old', 'Old', 'sha256:old', 'old', 'old', NULL)",
            [],
        )
        .expect("old projection should be insertable");
    connection
        .pragma_update(None, "user_version", 99)
        .expect("schema version should be replaceable");
    drop(connection);

    let reopened =
        SearchIndex::open(directory.path()).expect("incompatible projection should be recreated");

    assert_eq!(
        reopened.status().expect("new status should be readable"),
        SearchIndexStatus {
            schema_version: 1,
            document_count: 0,
        }
    );
}

#[test]
fn replaces_an_incomplete_schema_at_the_current_version() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let database_path = directory.path().join("index.sqlite");
    let connection =
        Connection::open(&database_path).expect("incomplete database should be created");
    connection
        .execute("CREATE TABLE unrelated (value TEXT)", [])
        .expect("incomplete schema fixture should be created");
    connection
        .pragma_update(None, "user_version", 1)
        .expect("current schema version fixture should be set");
    drop(connection);

    let index = SearchIndex::open(directory.path()).expect("incomplete schema should be recreated");

    assert_eq!(
        index.status().expect("new status should be readable"),
        SearchIndexStatus {
            schema_version: 1,
            document_count: 0,
        }
    );
}

#[test]
fn rebuilds_after_database_deletion_and_corruption() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let index = SearchIndex::open(directory.path()).expect("search index should initialize");

    index.destroy().expect("projection should be removable");
    assert!(!index.database_path().exists());
    assert_eq!(
        index.rebuild().expect("deleted database should rebuild"),
        SearchIndexStatus {
            schema_version: 1,
            document_count: 0,
        }
    );

    index
        .destroy()
        .expect("projection should be removable again");
    fs::write(index.database_path(), b"not a sqlite database")
        .expect("corrupt database fixture should be written");
    assert_eq!(
        index.rebuild().expect("corrupt database should rebuild"),
        SearchIndexStatus {
            schema_version: 1,
            document_count: 0,
        }
    );
}

#[test]
fn rejects_a_file_as_the_state_root() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let state_file = directory.path().join("state");
    fs::write(&state_file, "not a directory").expect("state file fixture should be written");

    assert!(matches!(
        SearchIndex::open(&state_file),
        Err(SearchIndexError::StateRootNotDirectory(path)) if path == state_file
    ));
}

#[cfg(unix)]
#[test]
fn rejects_a_symlink_database_file() {
    use std::os::unix::fs::symlink;

    let directory = TempDir::new().expect("temporary directory should be created");
    let state_root = directory.path().join("state");
    fs::create_dir(&state_root).expect("state root should be created");
    let outside = directory.path().join("outside.sqlite");
    fs::write(&outside, "outside").expect("outside fixture should be written");
    symlink(&outside, state_root.join("index.sqlite")).expect("symlink fixture should be created");

    assert!(matches!(
        SearchIndex::open(&state_root),
        Err(SearchIndexError::SymlinkNotAllowed(path))
            if path == fs::canonicalize(&state_root)
                .expect("state root should canonicalize")
                .join("index.sqlite")
    ));
}
