use std::fs;

use rusqlite::{Connection, OptionalExtension};
use tempfile::TempDir;

use knowledgeos_backend::{
    config::AppConfig,
    domain::path::MarkdownPath,
    infrastructure::{
        index_sync::SearchIndexSynchronizer,
        markdown::MarkdownReader,
        search_index::{IndexMutation, IndexReconciliation, SearchIndex},
        vault::VaultRoot,
    },
    state::AppState,
};

const MAX_MARKDOWN_BYTES: u64 = 5 * 1024 * 1024;

#[test]
fn create_and_update_write_the_latest_search_projection() {
    let fixture = Fixture::new();
    let state = AppState::initialize(fixture.config()).expect("application should initialize");
    let path = markdown_path("projects/search.md");
    fs::create_dir(fixture.vault_path().join("projects"))
        .expect("project directory should be created");

    let created = state
        .markdown_writer
        .create(
            &path,
            "---\ntags: [Rust]\n---\n# First title\nold body".to_owned(),
        )
        .expect("Markdown create should succeed");
    assert_eq!(
        indexed_title(
            state
                .search_index
                .as_ref()
                .expect("index should be available"),
            path.as_str()
        )
        .as_deref(),
        Some("First title")
    );
    assert_eq!(
        matched_path(
            state
                .search_index
                .as_ref()
                .expect("index should be available"),
            "rust"
        )
        .as_deref(),
        Some(path.as_str())
    );

    state
        .markdown_writer
        .update(
            &path,
            "# Updated title\nnew searchable body".to_owned(),
            &created.hash,
        )
        .expect("Markdown update should succeed");

    let index = state
        .search_index
        .as_ref()
        .expect("index should be available");
    assert_eq!(
        indexed_title(index, path.as_str()).as_deref(),
        Some("Updated title")
    );
    assert_eq!(
        matched_path(index, "searchable").as_deref(),
        Some(path.as_str())
    );
    assert_eq!(matched_path(index, "old"), None);
}

#[test]
fn reconciliation_repairs_external_create_update_delete_and_ignores_trash() {
    let fixture = Fixture::new();
    fs::write(fixture.vault_path().join("changed.md"), "# Old\nold")
        .expect("initial Markdown should be written");
    fs::write(fixture.vault_path().join("deleted.md"), "# Deleted")
        .expect("initial Markdown should be written");
    fs::write(fixture.vault_path().join("stable.md"), "# Stable\nintact")
        .expect("initial Markdown should be written");
    let index = SearchIndex::open(fixture.state_path()).expect("index should open");
    let sync = fixture.synchronizer(index.clone());

    assert_eq!(
        sync.reconcile()
            .expect("initial sync should succeed")
            .changes,
        IndexReconciliation {
            inserted: 3,
            updated: 0,
            unchanged: 0,
            deleted: 0,
        }
    );
    let connection = Connection::open(index.database_path()).expect("database should open");
    connection
        .execute(
            "UPDATE documents SET title = 'Corrupted' WHERE path = 'stable.md'",
            [],
        )
        .expect("document drift fixture should be written");
    connection
        .execute("DELETE FROM search_documents WHERE path = 'stable.md'", [])
        .expect("FTS drift fixture should be written");
    drop(connection);

    fs::write(
        fixture.vault_path().join("changed.md"),
        "# Changed\nnew body",
    )
    .expect("Markdown should be externally updated");
    fs::remove_file(fixture.vault_path().join("deleted.md"))
        .expect("Markdown should be externally deleted");
    fs::write(
        fixture.vault_path().join("created.md"),
        "---\ntags: [New]\n---\n# Created",
    )
    .expect("Markdown should be externally created");
    fs::write(fixture.vault_path().join("invalid.md"), [0xff, 0xfe])
        .expect("invalid UTF-8 fixture should be written");
    fs::create_dir(fixture.vault_path().join("_trash")).expect("trash should be created");
    fs::write(
        fixture.vault_path().join("_trash/removed.md"),
        "# Must not be indexed",
    )
    .expect("trash fixture should be written");

    let report = sync
        .reconcile()
        .expect("drift reconciliation should succeed");

    assert_eq!(report.discovered, 4);
    assert_eq!(report.skipped, 1);
    assert_eq!(
        report.changes,
        IndexReconciliation {
            inserted: 1,
            updated: 2,
            unchanged: 0,
            deleted: 1,
        }
    );
    assert_eq!(
        index
            .status()
            .expect("status should be readable")
            .document_count,
        3
    );
    assert_eq!(
        indexed_title(&index, "changed.md").as_deref(),
        Some("Changed")
    );
    assert_eq!(
        matched_path(&index, "created").as_deref(),
        Some("created.md")
    );
    assert_eq!(indexed_title(&index, "deleted.md"), None);
    assert_eq!(
        indexed_title(&index, "stable.md").as_deref(),
        Some("Stable")
    );
    assert_eq!(matched_path(&index, "intact").as_deref(), Some("stable.md"));
    assert_eq!(indexed_title(&index, "_trash/removed.md"), None);
    assert_eq!(indexed_title(&index, "invalid.md"), None);
}

#[test]
fn move_delete_and_index_failure_follow_the_projection_cache_policy() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.vault_path().join("archive")).expect("archive should be created");
    fs::write(fixture.vault_path().join("source.md"), "# Source")
        .expect("source should be written");
    let index = SearchIndex::open(fixture.state_path()).expect("index should open");
    let sync = fixture.synchronizer(index.clone());
    let reader = fixture.reader();
    let source = markdown_path("source.md");
    let source_document = reader.read(&source).expect("source should be read");
    assert_eq!(
        sync.upsert_document(&source_document)
            .expect("source should be indexed"),
        IndexMutation::Inserted
    );

    fs::rename(
        fixture.vault_path().join("source.md"),
        fixture.vault_path().join("archive/destination.md"),
    )
    .expect("source should move");
    let destination = markdown_path("archive/destination.md");
    let destination_document = reader
        .read(&destination)
        .expect("destination should be read");
    assert_eq!(
        sync.move_document(&source, &destination_document)
            .expect("move projection should sync"),
        IndexMutation::Inserted
    );
    assert_eq!(indexed_title(&index, source.as_str()), None);
    assert_eq!(
        indexed_title(&index, destination.as_str()).as_deref(),
        Some("Source")
    );
    assert!(
        sync.delete_path(&destination)
            .expect("delete projection should sync")
    );
    assert_eq!(indexed_title(&index, destination.as_str()), None);

    let state = AppState::initialize(fixture.config()).expect("application should initialize");
    state
        .search_index
        .as_ref()
        .expect("index should be available")
        .destroy()
        .expect("index failure fixture should remove the database");
    let durable_path = markdown_path("durable.md");
    let document = state
        .markdown_writer
        .create(&durable_path, "# Durable source".to_owned())
        .expect("index failure must not fail source Markdown create");
    assert_eq!(document.path, durable_path);
    assert_eq!(
        fs::read_to_string(fixture.vault_path().join("durable.md"))
            .expect("source Markdown should remain readable"),
        "# Durable source"
    );
}

struct Fixture {
    directory: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let directory = TempDir::new().expect("temporary directory should be created");
        fs::create_dir(directory.path().join("vault")).expect("Vault should be created");
        Self { directory }
    }

    fn vault_path(&self) -> std::path::PathBuf {
        self.directory.path().join("vault")
    }

    fn state_path(&self) -> std::path::PathBuf {
        self.directory.path().join("state")
    }

    fn config(&self) -> AppConfig {
        let mut config = AppConfig::for_test(self.vault_path());
        config.state_root = self.state_path();
        config
    }

    fn reader(&self) -> MarkdownReader {
        MarkdownReader::new(
            VaultRoot::open(self.vault_path()).expect("Vault should open"),
            MAX_MARKDOWN_BYTES,
        )
    }

    fn synchronizer(&self, index: SearchIndex) -> SearchIndexSynchronizer {
        SearchIndexSynchronizer::new(
            VaultRoot::open(self.vault_path()).expect("Vault should open"),
            MAX_MARKDOWN_BYTES,
            index,
        )
    }
}

fn markdown_path(value: &str) -> MarkdownPath {
    MarkdownPath::parse(value).expect("test Markdown path should be valid")
}

fn indexed_title(index: &SearchIndex, path: &str) -> Option<String> {
    let connection = Connection::open(index.database_path()).expect("database should open");
    connection
        .query_row(
            "SELECT title FROM documents WHERE path = ?1",
            [path],
            |row| row.get(0),
        )
        .optional()
        .expect("document projection should be queryable")
}

fn matched_path(index: &SearchIndex, query: &str) -> Option<String> {
    let connection = Connection::open(index.database_path()).expect("database should open");
    connection
        .query_row(
            "SELECT path FROM search_documents WHERE search_documents MATCH ?1",
            [query],
            |row| row.get(0),
        )
        .optional()
        .expect("FTS projection should be queryable")
}
