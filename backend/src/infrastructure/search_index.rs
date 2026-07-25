use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use rusqlite::{Connection, OptionalExtension, Transaction};
use thiserror::Error;

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
}
