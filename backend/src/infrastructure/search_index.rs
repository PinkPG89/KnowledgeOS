use std::{
    collections::{BTreeMap, HashMap},
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use rusqlite::{Connection, OptionalExtension, Transaction};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::domain::{path::MarkdownPath, projection::MarkdownProjection};

const DATABASE_FILE_NAME: &str = "index.sqlite";
const SCHEMA_VERSION: i32 = 1;
const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(2);

const CREATE_SCHEMA_SQL: &str = r"
    CREATE TABLE documents (
        path TEXT PRIMARY KEY NOT NULL,
        title TEXT NOT NULL,
        body TEXT NOT NULL,
        content_hash TEXT NOT NULL,
        modified_at TEXT NOT NULL,
        indexed_at TEXT NOT NULL,
        frontmatter_json TEXT
    ) STRICT;

    CREATE VIRTUAL TABLE search_documents USING fts5(
        path UNINDEXED,
        title,
        body,
        tags,
        tokenize = 'unicode61 remove_diacritics 2'
    );
";

#[derive(Clone, Debug)]
pub struct SearchIndex {
    state_root: PathBuf,
    database_path: PathBuf,
    lifecycle_lock: Arc<Mutex<()>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchIndexStatus {
    pub schema_version: i32,
    pub document_count: u64,
}

/// `SQLite`에 저장할 parser projection과 원본 파일 수정 시각입니다.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexDocument {
    pub projection: MarkdownProjection,
    pub modified_at: SystemTime,
}

/// 단일 incremental projection 갱신 결과입니다.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexMutation {
    Inserted,
    Updated,
    Unchanged,
}

/// 전체 Vault reconciliation에서 적용한 변경 집계입니다.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IndexReconciliation {
    pub inserted: u64,
    pub updated: u64,
    pub unchanged: u64,
    pub deleted: u64,
}

impl SearchIndex {
    /// 별도 application state directory에서 검색 projection을 열거나 생성합니다.
    ///
    /// # Errors
    ///
    /// State directory를 생성·검증할 수 없거나 SQLite/FTS5 schema를 준비할 수 없으면
    /// [`SearchIndexError`]를 반환합니다.
    pub fn open(state_root: impl AsRef<Path>) -> Result<Self, SearchIndexError> {
        let state_root = prepare_state_root(state_root.as_ref())?;
        let database_path = state_root.join(DATABASE_FILE_NAME);
        reject_symlink_if_present(&database_path)?;

        let index = Self {
            state_root,
            database_path,
            lifecycle_lock: Arc::new(Mutex::new(())),
        };
        index.initialize()?;
        Ok(index)
    }

    #[must_use]
    pub fn state_root(&self) -> &Path {
        &self.state_root
    }

    #[must_use]
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    /// 현재 schema version과 indexed document 수를 조회합니다.
    ///
    /// # Errors
    ///
    /// `SQLite` database를 열거나 metadata를 읽지 못하면 [`SearchIndexError`]를 반환합니다.
    pub fn status(&self) -> Result<SearchIndexStatus, SearchIndexError> {
        let _guard = self.lock_lifecycle()?;
        let connection = self.open_connection()?;
        read_status(&connection, &self.database_path)
    }

    /// DB와 `SQLite` sidecar를 삭제합니다. Markdown 원본에는 접근하지 않습니다.
    ///
    /// # Errors
    ///
    /// Lifecycle lock을 얻지 못하거나 DB 파일을 안전하게 제거할 수 없으면
    /// [`SearchIndexError`]를 반환합니다.
    pub fn destroy(&self) -> Result<(), SearchIndexError> {
        let _guard = self.lock_lifecycle()?;
        remove_database_files(&self.database_path)
    }

    /// 기존 projection을 삭제하고 빈 최신 schema로 다시 만듭니다.
    ///
    /// C02와 C03에서 filesystem projection population을 이 lifecycle 뒤에 연결합니다.
    ///
    /// # Errors
    ///
    /// 기존 DB 제거 또는 최신 schema 생성에 실패하면 [`SearchIndexError`]를 반환합니다.
    pub fn rebuild(&self) -> Result<SearchIndexStatus, SearchIndexError> {
        let _guard = self.lock_lifecycle()?;
        remove_database_files(&self.database_path)?;
        initialize_database(&self.database_path)?;
        let connection = self.open_connection()?;
        read_status(&connection, &self.database_path)
    }

    /// 단일 Markdown projection을 hash 기준으로 insert 또는 update합니다.
    ///
    /// # Errors
    ///
    /// Lifecycle lock, timestamp 직렬화 또는 `SQLite` transaction이 실패하면
    /// [`SearchIndexError`]를 반환합니다.
    pub fn upsert(&self, document: &IndexDocument) -> Result<IndexMutation, SearchIndexError> {
        let _guard = self.lock_lifecycle()?;
        let mut connection = self.open_connection()?;
        let transaction =
            connection
                .transaction()
                .map_err(|source| SearchIndexError::Database {
                    path: self.database_path.clone(),
                    source,
                })?;
        let mutation = upsert_in_transaction(&transaction, document, &self.database_path, false)?;
        transaction
            .commit()
            .map_err(|source| SearchIndexError::Database {
                path: self.database_path.clone(),
                source,
            })?;
        Ok(mutation)
    }

    /// 단일 Markdown 경로의 document와 FTS projection을 원자적으로 제거합니다.
    ///
    /// # Errors
    ///
    /// Lifecycle lock 또는 `SQLite` transaction이 실패하면 [`SearchIndexError`]를 반환합니다.
    pub fn delete(&self, path: &MarkdownPath) -> Result<bool, SearchIndexError> {
        let _guard = self.lock_lifecycle()?;
        let mut connection = self.open_connection()?;
        let transaction =
            connection
                .transaction()
                .map_err(|source| SearchIndexError::Database {
                    path: self.database_path.clone(),
                    source,
                })?;
        let deleted = delete_in_transaction(&transaction, path.as_str(), &self.database_path)?;
        transaction
            .commit()
            .map_err(|source| SearchIndexError::Database {
                path: self.database_path.clone(),
                source,
            })?;
        Ok(deleted)
    }

    /// 기존 경로를 제거하고 이동된 document projection을 한 transaction에서 저장합니다.
    ///
    /// # Errors
    ///
    /// Lifecycle lock, timestamp 직렬화 또는 `SQLite` transaction이 실패하면
    /// [`SearchIndexError`]를 반환합니다.
    pub fn move_document(
        &self,
        source: &MarkdownPath,
        destination: &IndexDocument,
    ) -> Result<IndexMutation, SearchIndexError> {
        let _guard = self.lock_lifecycle()?;
        let mut connection = self.open_connection()?;
        let transaction =
            connection
                .transaction()
                .map_err(|source| SearchIndexError::Database {
                    path: self.database_path.clone(),
                    source,
                })?;
        if source != &destination.projection.path {
            delete_in_transaction(&transaction, source.as_str(), &self.database_path)?;
        }
        let mutation = upsert_in_transaction(&transaction, destination, &self.database_path, true)?;
        transaction
            .commit()
            .map_err(|source| SearchIndexError::Database {
                path: self.database_path.clone(),
                source,
            })?;
        Ok(mutation)
    }

    /// 현재 filesystem snapshot을 기준으로 누락·변경·삭제 projection을 원자적으로 조정합니다.
    ///
    /// # Errors
    ///
    /// 중복 path, lifecycle lock, timestamp 직렬화 또는 `SQLite` transaction이 실패하면
    /// [`SearchIndexError`]를 반환합니다.
    pub fn reconcile(
        &self,
        documents: Vec<IndexDocument>,
    ) -> Result<IndexReconciliation, SearchIndexError> {
        let mut desired = BTreeMap::new();
        for document in documents {
            let path = document.projection.path.as_str().to_owned();
            if desired.insert(path.clone(), document).is_some() {
                return Err(SearchIndexError::DuplicateProjectionPath(path));
            }
        }

        let _guard = self.lock_lifecycle()?;
        let mut connection = self.open_connection()?;
        let transaction =
            connection
                .transaction()
                .map_err(|source| SearchIndexError::Database {
                    path: self.database_path.clone(),
                    source,
                })?;
        let indexed = indexed_documents(&transaction, &self.database_path)?;
        let mut result = IndexReconciliation::default();

        for document in desired.values() {
            match indexed.get(document.projection.path.as_str()) {
                Some(stored) if stored.matches(document)? => {
                    result.unchanged += 1;
                }
                Some(_) => {
                    upsert_in_transaction(&transaction, document, &self.database_path, true)?;
                    result.updated += 1;
                }
                None => {
                    upsert_in_transaction(&transaction, document, &self.database_path, true)?;
                    result.inserted += 1;
                }
            }
        }

        for indexed_path in indexed.keys() {
            if !desired.contains_key(indexed_path)
                && delete_in_transaction(&transaction, indexed_path, &self.database_path)?
            {
                result.deleted += 1;
            }
        }

        transaction
            .commit()
            .map_err(|source| SearchIndexError::Database {
                path: self.database_path.clone(),
                source,
            })?;
        Ok(result)
    }

    fn initialize(&self) -> Result<(), SearchIndexError> {
        let _guard = self.lock_lifecycle()?;
        initialize_database(&self.database_path)
    }

    fn open_connection(&self) -> Result<Connection, SearchIndexError> {
        open_connection(&self.database_path)
    }

    fn lock_lifecycle(&self) -> Result<std::sync::MutexGuard<'_, ()>, SearchIndexError> {
        self.lifecycle_lock
            .lock()
            .map_err(|_| SearchIndexError::LifecycleLockPoisoned)
    }
}

fn upsert_in_transaction(
    transaction: &Transaction<'_>,
    document: &IndexDocument,
    database_path: &Path,
    force: bool,
) -> Result<IndexMutation, SearchIndexError> {
    let path = document.projection.path.as_str();
    let current_hash = transaction
        .query_row(
            "SELECT content_hash FROM documents WHERE path = ?1",
            [path],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;
    if !force
        && current_hash
            .as_deref()
            .is_some_and(|hash| hash == document.projection.content_hash)
    {
        return Ok(IndexMutation::Unchanged);
    }

    let modified_at = format_timestamp(document.modified_at)?;
    let indexed_at = format_timestamp(SystemTime::now())?;
    transaction
        .execute(
            "INSERT INTO documents (
                path, title, body, content_hash, modified_at, indexed_at, frontmatter_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(path) DO UPDATE SET
                title = excluded.title,
                body = excluded.body,
                content_hash = excluded.content_hash,
                modified_at = excluded.modified_at,
                indexed_at = excluded.indexed_at,
                frontmatter_json = excluded.frontmatter_json",
            rusqlite::params![
                path,
                document.projection.title,
                document.projection.body,
                document.projection.content_hash,
                modified_at,
                indexed_at,
                document.projection.frontmatter_json,
            ],
        )
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;
    transaction
        .execute("DELETE FROM search_documents WHERE path = ?1", [path])
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;
    transaction
        .execute(
            "INSERT INTO search_documents (path, title, body, tags)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                path,
                document.projection.title,
                document.projection.body,
                document.projection.tags.join(" "),
            ],
        )
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;

    Ok(if current_hash.is_some() {
        IndexMutation::Updated
    } else {
        IndexMutation::Inserted
    })
}

fn delete_in_transaction(
    transaction: &Transaction<'_>,
    path: &str,
    database_path: &Path,
) -> Result<bool, SearchIndexError> {
    transaction
        .execute("DELETE FROM search_documents WHERE path = ?1", [path])
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;
    let deleted = transaction
        .execute("DELETE FROM documents WHERE path = ?1", [path])
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;
    Ok(deleted > 0)
}

#[derive(Default)]
struct StoredIndexDocument {
    title: String,
    body: String,
    content_hash: String,
    modified_at: String,
    frontmatter_json: Option<String>,
    fts_rows: Vec<StoredFtsDocument>,
}

impl StoredIndexDocument {
    fn matches(&self, document: &IndexDocument) -> Result<bool, SearchIndexError> {
        let projection = &document.projection;
        Ok(self.title == projection.title
            && self.body == projection.body
            && self.content_hash == projection.content_hash
            && self.modified_at == format_timestamp(document.modified_at)?
            && self.frontmatter_json == projection.frontmatter_json
            && self.fts_rows
                == [StoredFtsDocument {
                    title: projection.title.clone(),
                    body: projection.body.clone(),
                    tags: projection.tags.join(" "),
                }])
    }
}

#[derive(Eq, PartialEq)]
struct StoredFtsDocument {
    title: String,
    body: String,
    tags: String,
}

fn indexed_documents(
    transaction: &Transaction<'_>,
    database_path: &Path,
) -> Result<HashMap<String, StoredIndexDocument>, SearchIndexError> {
    let mut statement = transaction
        .prepare(
            "SELECT path, title, body, content_hash, modified_at, frontmatter_json
             FROM documents",
        )
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                StoredIndexDocument {
                    title: row.get(1)?,
                    body: row.get(2)?,
                    content_hash: row.get(3)?,
                    modified_at: row.get(4)?,
                    frontmatter_json: row.get(5)?,
                    fts_rows: Vec::new(),
                },
            ))
        })
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;
    let mut documents = HashMap::new();
    for row in rows {
        let (path, document) = row.map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;
        documents.insert(path, document);
    }
    drop(statement);

    let mut statement = transaction
        .prepare("SELECT path, title, body, tags FROM search_documents ORDER BY rowid")
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                StoredFtsDocument {
                    title: row.get(1)?,
                    body: row.get(2)?,
                    tags: row.get(3)?,
                },
            ))
        })
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;
    for row in rows {
        let (path, fts) = row.map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;
        if let Some(document) = documents.get_mut(&path) {
            document.fts_rows.push(fts);
        }
    }
    Ok(documents)
}

fn format_timestamp(timestamp: SystemTime) -> Result<String, SearchIndexError> {
    OffsetDateTime::from(timestamp)
        .format(&Rfc3339)
        .map_err(SearchIndexError::TimestampFormat)
}

fn prepare_state_root(configured: &Path) -> Result<PathBuf, SearchIndexError> {
    match fs::metadata(configured) {
        Ok(metadata) if !metadata.is_dir() => {
            return Err(SearchIndexError::StateRootNotDirectory(
                configured.to_path_buf(),
            ));
        }
        Ok(_) => {}
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(configured).map_err(|source| SearchIndexError::Io {
                path: configured.to_path_buf(),
                source,
            })?;
        }
        Err(source) => {
            return Err(SearchIndexError::Io {
                path: configured.to_path_buf(),
                source,
            });
        }
    }
    let canonical = fs::canonicalize(configured).map_err(|source| SearchIndexError::Io {
        path: configured.to_path_buf(),
        source,
    })?;
    let metadata = fs::metadata(&canonical).map_err(|source| SearchIndexError::Io {
        path: canonical.clone(),
        source,
    })?;
    if !metadata.is_dir() {
        return Err(SearchIndexError::StateRootNotDirectory(
            configured.to_path_buf(),
        ));
    }
    fs::read_dir(&canonical).map_err(|source| SearchIndexError::Io {
        path: canonical.clone(),
        source,
    })?;
    Ok(canonical)
}

fn initialize_database(database_path: &Path) -> Result<(), SearchIndexError> {
    reject_symlink_if_present(database_path)?;
    let connection = open_connection(database_path)?;
    let current_version = schema_version(&connection, database_path)?;

    if current_version == SCHEMA_VERSION {
        match verify_schema(&connection, database_path) {
            Ok(()) => return Ok(()),
            Err(SearchIndexError::InvalidSchema) => {}
            Err(error) => return Err(error),
        }
    }

    drop(connection);
    remove_database_files(database_path)?;
    let mut connection = open_connection(database_path)?;
    create_schema(&mut connection, database_path)
}

fn open_connection(database_path: &Path) -> Result<Connection, SearchIndexError> {
    reject_symlink_if_present(database_path)?;
    let connection =
        Connection::open(database_path).map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;
    connection
        .busy_timeout(SQLITE_BUSY_TIMEOUT)
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;
    Ok(connection)
}

fn create_schema(
    connection: &mut Connection,
    database_path: &Path,
) -> Result<(), SearchIndexError> {
    let transaction = connection
        .transaction()
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;
    create_schema_in_transaction(&transaction, database_path)?;
    transaction
        .commit()
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })
}

fn create_schema_in_transaction(
    transaction: &Transaction<'_>,
    database_path: &Path,
) -> Result<(), SearchIndexError> {
    transaction
        .execute_batch(CREATE_SCHEMA_SQL)
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;
    transaction
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })
}

fn verify_schema(connection: &Connection, database_path: &Path) -> Result<(), SearchIndexError> {
    let documents_table = table_sql(connection, "documents", database_path)?;
    let search_table = table_sql(connection, "search_documents", database_path)?;
    if documents_table.is_none()
        || !search_table
            .as_deref()
            .is_some_and(|sql| sql.contains("fts5"))
    {
        return Err(SearchIndexError::InvalidSchema);
    }

    connection
        .query_row(
            "SELECT count(*) FROM search_documents WHERE search_documents MATCH 'schema_probe'",
            [],
            |_| Ok(()),
        )
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })
}

fn table_sql(
    connection: &Connection,
    table_name: &str,
    database_path: &Path,
) -> Result<Option<String>, SearchIndexError> {
    connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
            [table_name],
            |row| row.get(0),
        )
        .optional()
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })
}

fn schema_version(connection: &Connection, database_path: &Path) -> Result<i32, SearchIndexError> {
    connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })
}

fn read_status(
    connection: &Connection,
    database_path: &Path,
) -> Result<SearchIndexStatus, SearchIndexError> {
    verify_schema(connection, database_path)?;
    let document_count = connection
        .query_row("SELECT count(*) FROM documents", [], |row| row.get(0))
        .map_err(|source| SearchIndexError::Database {
            path: database_path.to_path_buf(),
            source,
        })?;
    Ok(SearchIndexStatus {
        schema_version: schema_version(connection, database_path)?,
        document_count,
    })
}

fn remove_database_files(database_path: &Path) -> Result<(), SearchIndexError> {
    for path in database_paths(database_path) {
        reject_symlink_if_present(&path)?;
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(SearchIndexError::Io { path, source }),
        }
    }
    Ok(())
}

fn database_paths(database_path: &Path) -> [PathBuf; 3] {
    [
        database_path.to_path_buf(),
        sidecar_path(database_path, "-wal"),
        sidecar_path(database_path, "-shm"),
    ]
}

fn sidecar_path(database_path: &Path, suffix: &str) -> PathBuf {
    let mut path = OsString::from(database_path.as_os_str());
    path.push(suffix);
    PathBuf::from(path)
}

fn reject_symlink_if_present(path: &Path) -> Result<(), SearchIndexError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(SearchIndexError::SymlinkNotAllowed(path.to_path_buf()))
        }
        Ok(_) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(SearchIndexError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[derive(Debug, Error)]
pub enum SearchIndexError {
    #[error("search index state root is not a directory: {0}")]
    StateRootNotDirectory(PathBuf),
    #[error("search index paths must not be symbolic links: {0}")]
    SymlinkNotAllowed(PathBuf),
    #[error("search index filesystem operation failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("search index database operation failed for {path}: {source}")]
    Database {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },
    #[error("search index schema is incomplete or invalid")]
    InvalidSchema,
    #[error("search index lifecycle lock is poisoned")]
    LifecycleLockPoisoned,
    #[error("duplicate Markdown projection path during reconciliation: {0}")]
    DuplicateProjectionPath(String),
    #[error("search index timestamp formatting failed: {0}")]
    TimestampFormat(#[source] time::error::Format),
}
