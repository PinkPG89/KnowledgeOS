use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::{document::MarkdownDocument, path::MarkdownPath};

use super::{
    markdown::{MarkdownReadError, MarkdownReader},
    vault::{VaultError, VaultRoot},
};

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// 단일 활성 Vault에 UTF-8 Markdown 파일을 배타적으로 생성하는 writer입니다.
///
/// `OpenOptions::create_new(true)`를 사용하므로 경로 검증 직후 다른 요청이 같은
/// 파일을 먼저 생성하더라도 기존 파일을 덮어쓰지 않습니다.
#[derive(Clone, Debug)]
pub struct MarkdownWriter {
    vault: VaultRoot,
    max_bytes: u64,
    write_lock: Arc<Mutex<()>>,
}

impl MarkdownWriter {
    #[must_use]
    pub fn new(vault: VaultRoot, max_bytes: u64) -> Self {
        Self {
            vault,
            max_bytes,
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    /// 새로운 Markdown 파일을 생성하고 디스크 동기화가 끝난 문서 snapshot을 반환합니다.
    ///
    /// # Errors
    ///
    /// 부모 경로가 Vault 정책을 위반하거나, 파일이 이미 존재하거나, content가 크기
    /// 제한을 초과하거나, write·flush·sync·metadata 작업이 실패하면 오류를 반환합니다.
    pub fn create(
        &self,
        path: &MarkdownPath,
        content: String,
    ) -> Result<MarkdownDocument, MarkdownWriteError> {
        self.create_with(path, content, persist_content)
    }

    fn create_with(
        &self,
        path: &MarkdownPath,
        content: String,
        persist: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
    ) -> Result<MarkdownDocument, MarkdownWriteError> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| MarkdownWriteError::LockPoisoned)?;
        let size = u64::try_from(content.len()).unwrap_or(u64::MAX);
        if size > self.max_bytes {
            return Err(MarkdownWriteError::FileTooLarge {
                path: path.to_string(),
                observed: size,
                maximum: self.max_bytes,
            });
        }

        let absolute = self.vault.resolve_parent_for_create(path.as_canonical())?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&absolute)
            .map_err(|source| map_create_error(path, source))?;

        if let Err(source) = persist(&mut file, content.as_bytes()) {
            drop(file);
            remove_incomplete_file(&absolute, path);
            return Err(MarkdownWriteError::Io {
                path: path.to_string(),
                source,
            });
        }

        let metadata = match file.metadata() {
            Ok(metadata) => metadata,
            Err(source) => {
                drop(file);
                remove_incomplete_file(&absolute, path);
                return Err(MarkdownWriteError::Metadata {
                    path: path.to_string(),
                    source,
                });
            }
        };
        let modified_at = match metadata.modified() {
            Ok(modified_at) => modified_at,
            Err(source) => {
                drop(file);
                remove_incomplete_file(&absolute, path);
                return Err(MarkdownWriteError::Metadata {
                    path: path.to_string(),
                    source,
                });
            }
        };

        let hash = format!("sha256:{}", hex::encode(Sha256::digest(content.as_bytes())));
        Ok(MarkdownDocument {
            path: path.clone(),
            content,
            hash,
            size: metadata.len(),
            modified_at,
        })
    }

    /// 현재 hash가 `base_hash`와 일치할 때만 기존 Markdown 파일을 atomic replace합니다.
    ///
    /// temp 파일은 대상과 같은 directory에 생성하므로 Linux의 `rename`이 하나의
    /// filesystem 안에서 atomic하게 기존 파일을 교체합니다.
    ///
    /// # Errors
    ///
    /// 대상이 없거나 regular UTF-8 Markdown이 아니거나, hash가 일치하지 않거나,
    /// temp write·sync·rename·metadata 작업이 실패하면 오류를 반환합니다.
    pub fn update(
        &self,
        path: &MarkdownPath,
        content: String,
        base_hash: &str,
    ) -> Result<MarkdownDocument, MarkdownUpdateError> {
        self.update_with(path, content, base_hash, persist_content)
    }

    fn update_with(
        &self,
        path: &MarkdownPath,
        content: String,
        base_hash: &str,
        persist: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
    ) -> Result<MarkdownDocument, MarkdownUpdateError> {
        let size = u64::try_from(content.len()).unwrap_or(u64::MAX);
        if size > self.max_bytes {
            return Err(MarkdownUpdateError::FileTooLarge {
                path: path.to_string(),
                observed: size,
                maximum: self.max_bytes,
            });
        }

        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| MarkdownUpdateError::LockPoisoned)?;
        let reader = MarkdownReader::new(self.vault.clone(), self.max_bytes);
        let current = reader.read(path).map_err(MarkdownUpdateError::from_read)?;
        enforce_base_hash(path, base_hash, &current.hash)?;

        let target = self.vault.resolve_existing(path.as_canonical())?;
        let temporary = temporary_path(&target)?;
        let mut temporary_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|source| MarkdownUpdateError::Io {
                path: path.to_string(),
                source,
            })?;

        if let Err(source) = persist(&mut temporary_file, content.as_bytes()) {
            drop(temporary_file);
            remove_temporary_file(&temporary, path);
            return Err(MarkdownUpdateError::Io {
                path: path.to_string(),
                source,
            });
        }

        let latest = match reader.read(path) {
            Ok(document) => document,
            Err(error) => {
                drop(temporary_file);
                remove_temporary_file(&temporary, path);
                return Err(MarkdownUpdateError::from_read(error));
            }
        };
        if let Err(error) = enforce_base_hash(path, base_hash, &latest.hash) {
            drop(temporary_file);
            remove_temporary_file(&temporary, path);
            return Err(error);
        }

        let metadata = match temporary_file.metadata() {
            Ok(metadata) => metadata,
            Err(source) => {
                drop(temporary_file);
                remove_temporary_file(&temporary, path);
                return Err(MarkdownUpdateError::Metadata {
                    path: path.to_string(),
                    source,
                });
            }
        };
        let modified_at = match metadata.modified() {
            Ok(modified_at) => modified_at,
            Err(source) => {
                drop(temporary_file);
                remove_temporary_file(&temporary, path);
                return Err(MarkdownUpdateError::Metadata {
                    path: path.to_string(),
                    source,
                });
            }
        };

        if let Err(source) = fs::rename(&temporary, &target) {
            drop(temporary_file);
            remove_temporary_file(&temporary, path);
            return Err(MarkdownUpdateError::Io {
                path: path.to_string(),
                source,
            });
        }
        let hash = format!("sha256:{}", hex::encode(Sha256::digest(content.as_bytes())));

        Ok(MarkdownDocument {
            path: path.clone(),
            content,
            hash,
            size: metadata.len(),
            modified_at,
        })
    }
}

fn persist_content(file: &mut File, content: &[u8]) -> io::Result<()> {
    file.write_all(content)?;
    file.flush()?;
    file.sync_all()
}

fn map_create_error(path: &MarkdownPath, source: io::Error) -> MarkdownWriteError {
    if source.kind() == io::ErrorKind::AlreadyExists {
        MarkdownWriteError::AlreadyExists(path.to_string())
    } else {
        MarkdownWriteError::Io {
            path: path.to_string(),
            source,
        }
    }
}

fn remove_incomplete_file(absolute: &Path, path: &MarkdownPath) {
    if let Err(cleanup_error) = fs::remove_file(absolute) {
        tracing::error!(
            relative_path = %path,
            %cleanup_error,
            "failed to remove incomplete Markdown file"
        );
    }
}

fn enforce_base_hash(
    path: &MarkdownPath,
    expected: &str,
    actual: &str,
) -> Result<(), MarkdownUpdateError> {
    if expected != actual {
        return Err(MarkdownUpdateError::HashConflict {
            path: path.to_string(),
            expected: expected.to_owned(),
            actual: actual.to_owned(),
        });
    }
    Ok(())
}

fn temporary_path(target: &Path) -> Result<std::path::PathBuf, MarkdownUpdateError> {
    let parent = target.parent().ok_or(MarkdownUpdateError::InvalidTarget)?;
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(
        ".knowledgeos-{}-{sequence}.tmp",
        std::process::id()
    )))
}

fn remove_temporary_file(temporary: &Path, path: &MarkdownPath) {
    if let Err(cleanup_error) = fs::remove_file(temporary) {
        if cleanup_error.kind() != io::ErrorKind::NotFound {
            tracing::error!(
                relative_path = %path,
                %cleanup_error,
                "failed to remove temporary Markdown file"
            );
        }
    }
}

#[derive(Debug, Error)]
pub enum MarkdownWriteError {
    #[error(transparent)]
    Vault(#[from] VaultError),

    #[error("Markdown file already exists: {0}")]
    AlreadyExists(String),

    #[error("Markdown file is too large: {observed} bytes; maximum is {maximum} bytes: {path}")]
    FileTooLarge {
        path: String,
        observed: u64,
        maximum: u64,
    },

    #[error("filesystem operation failed for {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("file metadata is unavailable for {path}: {source}")]
    Metadata {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("Markdown write lock is poisoned")]
    LockPoisoned,
}

#[derive(Debug, Error)]
pub enum MarkdownUpdateError {
    #[error(transparent)]
    Vault(#[from] VaultError),

    #[error("Markdown file does not exist: {0}")]
    NotFound(String),

    #[error("path is not a regular file: {0}")]
    NotRegularFile(String),

    #[error("Markdown file is too large: {observed} bytes; maximum is {maximum} bytes: {path}")]
    FileTooLarge {
        path: String,
        observed: u64,
        maximum: u64,
    },

    #[error("Markdown file is not valid UTF-8: {0}")]
    InvalidUtf8(String),

    #[error("Markdown file changed repeatedly while being read")]
    ReadConflict,

    #[error("Markdown hash conflict for {path}: expected {expected}, actual {actual}")]
    HashConflict {
        path: String,
        expected: String,
        actual: String,
    },

    #[error("filesystem operation failed for {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("file metadata is unavailable for {path}: {source}")]
    Metadata {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("Markdown write lock is poisoned")]
    LockPoisoned,

    #[error("Markdown target has no parent directory")]
    InvalidTarget,
}

impl MarkdownUpdateError {
    fn from_read(error: MarkdownReadError) -> Self {
        match error {
            MarkdownReadError::Vault(error) => Self::Vault(error),
            MarkdownReadError::NotFound(path) => Self::NotFound(path),
            MarkdownReadError::NotRegularFile(path) => Self::NotRegularFile(path),
            MarkdownReadError::FileTooLarge {
                path,
                observed,
                maximum,
            } => Self::FileTooLarge {
                path,
                observed,
                maximum,
            },
            MarkdownReadError::InvalidUtf8(path) => Self::InvalidUtf8(path),
            MarkdownReadError::ReadConflict => Self::ReadConflict,
            MarkdownReadError::Io { path, source } => Self::Io { path, source },
            MarkdownReadError::Metadata(source) => Self::Metadata {
                path: "unknown".to_owned(),
                source,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, io, sync::Arc, thread};

    use tempfile::TempDir;

    use sha2::{Digest, Sha256};

    use super::{MarkdownWriteError, MarkdownWriter};
    use crate::{domain::path::MarkdownPath, infrastructure::vault::VaultRoot};

    fn markdown_path(value: &str) -> MarkdownPath {
        MarkdownPath::parse(value).expect("test Markdown path must be valid")
    }

    fn content_hash(content: &str) -> String {
        format!("sha256:{}", hex::encode(Sha256::digest(content.as_bytes())))
    }

    #[test]
    fn creates_utf8_markdown_and_returns_metadata() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        fs::create_dir(directory.path().join("프로젝트"))
            .expect("nested directory should be created");
        let writer = MarkdownWriter::new(
            VaultRoot::open(directory.path()).expect("Vault should open"),
            1024,
        );

        let document = writer
            .create(
                &markdown_path("프로젝트/지식 노트.md"),
                "# 지식\n".to_owned(),
            )
            .expect("Markdown should be created");

        assert_eq!(document.content, "# 지식\n");
        assert_eq!(document.size, "# 지식\n".len() as u64);
        assert_eq!(
            fs::read_to_string(directory.path().join("프로젝트/지식 노트.md"))
                .expect("created Markdown should be readable"),
            "# 지식\n"
        );
    }

    #[test]
    fn accepts_empty_and_exact_limit_but_rejects_larger_content() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        let writer = MarkdownWriter::new(
            VaultRoot::open(directory.path()).expect("Vault should open"),
            8,
        );

        writer
            .create(&markdown_path("empty.md"), String::new())
            .expect("empty Markdown should be created");
        writer
            .create(&markdown_path("exact.md"), "12345678".to_owned())
            .expect("exact limit Markdown should be created");
        assert!(matches!(
            writer.create(&markdown_path("large.md"), "123456789".to_owned()),
            Err(MarkdownWriteError::FileTooLarge { .. })
        ));
        assert!(!directory.path().join("large.md").exists());
    }

    #[test]
    fn never_overwrites_an_existing_target() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        fs::write(directory.path().join("existing.md"), "original")
            .expect("existing Markdown should be written");
        let writer = MarkdownWriter::new(
            VaultRoot::open(directory.path()).expect("Vault should open"),
            1024,
        );

        assert!(matches!(
            writer.create(&markdown_path("existing.md"), "replacement".to_owned()),
            Err(MarkdownWriteError::AlreadyExists(path)) if path == "existing.md"
        ));
        assert_eq!(
            fs::read_to_string(directory.path().join("existing.md"))
                .expect("original Markdown should remain readable"),
            "original"
        );
    }

    #[test]
    fn allows_only_one_concurrent_create_for_the_same_path() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        let writer = Arc::new(MarkdownWriter::new(
            VaultRoot::open(directory.path()).expect("Vault should open"),
            1024,
        ));
        let mut handles = Vec::new();

        for content in ["first", "second"] {
            let writer = Arc::clone(&writer);
            handles.push(thread::spawn(move || {
                writer.create(&markdown_path("race.md"), content.to_owned())
            }));
        }

        let results = handles
            .into_iter()
            .map(|handle| handle.join().expect("create thread should finish"))
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(MarkdownWriteError::AlreadyExists(_))))
                .count(),
            1
        );
    }

    #[test]
    fn removes_an_incomplete_file_when_persistence_fails() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        let writer = MarkdownWriter::new(
            VaultRoot::open(directory.path()).expect("Vault should open"),
            1024,
        );
        let path = markdown_path("incomplete.md");

        let result = writer.create_with(&path, "content".to_owned(), |file, content| {
            use std::io::Write;

            file.write_all(&content[..3])?;
            Err(io::Error::other("injected persistence failure"))
        });

        assert!(matches!(result, Err(MarkdownWriteError::Io { .. })));
        assert!(!directory.path().join("incomplete.md").exists());
    }

    #[test]
    fn updates_a_file_when_the_base_hash_matches() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        fs::write(directory.path().join("note.md"), "original")
            .expect("original Markdown should be written");
        let writer = MarkdownWriter::new(
            VaultRoot::open(directory.path()).expect("Vault should open"),
            1024,
        );

        let document = writer
            .update(
                &markdown_path("note.md"),
                "updated".to_owned(),
                &content_hash("original"),
            )
            .expect("matching hash should update Markdown");

        assert_eq!(document.content, "updated");
        assert_eq!(document.hash, content_hash("updated"));
        assert_eq!(
            fs::read_to_string(directory.path().join("note.md"))
                .expect("updated Markdown should be readable"),
            "updated"
        );
    }

    #[test]
    fn stale_hash_and_persistence_failure_preserve_the_original() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        fs::write(directory.path().join("note.md"), "original")
            .expect("original Markdown should be written");
        let writer = MarkdownWriter::new(
            VaultRoot::open(directory.path()).expect("Vault should open"),
            1024,
        );
        let path = markdown_path("note.md");

        assert!(matches!(
            writer.update(&path, "stale".to_owned(), &content_hash("older")),
            Err(super::MarkdownUpdateError::HashConflict { .. })
        ));
        let failed = writer.update_with(
            &path,
            "replacement".to_owned(),
            &content_hash("original"),
            |file, content| {
                use std::io::Write;

                file.write_all(&content[..3])?;
                Err(io::Error::other("injected persistence failure"))
            },
        );

        assert!(matches!(failed, Err(super::MarkdownUpdateError::Io { .. })));
        assert_eq!(
            fs::read_to_string(directory.path().join("note.md"))
                .expect("original Markdown should remain readable"),
            "original"
        );
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("Vault should be readable")
                .count(),
            1
        );
    }

    #[test]
    fn concurrent_updates_with_one_base_hash_allow_only_one_success() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        fs::write(directory.path().join("note.md"), "original")
            .expect("original Markdown should be written");
        let writer = Arc::new(MarkdownWriter::new(
            VaultRoot::open(directory.path()).expect("Vault should open"),
            1024,
        ));
        let base_hash = content_hash("original");
        let mut handles = Vec::new();

        for content in ["first", "second"] {
            let writer = Arc::clone(&writer);
            let base_hash = base_hash.clone();
            handles.push(thread::spawn(move || {
                writer.update(&markdown_path("note.md"), content.to_owned(), &base_hash)
            }));
        }

        let results = handles
            .into_iter()
            .map(|handle| handle.join().expect("update thread should finish"))
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(
                    result,
                    Err(super::MarkdownUpdateError::HashConflict { .. })
                ))
                .count(),
            1
        );
    }
}
