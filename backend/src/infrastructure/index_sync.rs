use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::domain::{
    document::MarkdownDocument,
    path::{MarkdownPath, PathError},
    projection::FrontmatterStatus,
};

use super::{
    markdown::{MarkdownReadError, MarkdownReader},
    markdown_projection::MarkdownProjectionParser,
    search_index::{
        IndexDocument, IndexMutation, IndexReconciliation, SearchIndex, SearchIndexError,
    },
    vault::VaultRoot,
};

const TRASH_ROOT: &str = "_trash";

/// Vault scan 결과와 `SQLite` reconciliation 변경 집계입니다.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IndexSyncReport {
    pub discovered: u64,
    pub skipped: u64,
    pub malformed_frontmatter: u64,
    pub changes: IndexReconciliation,
}

/// Markdown 원본과 재생성 가능한 `SQLite` projection을 동기화합니다.
#[derive(Clone, Debug)]
pub struct SearchIndexSynchronizer {
    vault: VaultRoot,
    reader: MarkdownReader,
    index: SearchIndex,
}

impl SearchIndexSynchronizer {
    #[must_use]
    pub fn new(vault: VaultRoot, max_markdown_bytes: u64, index: SearchIndex) -> Self {
        Self {
            reader: MarkdownReader::new(vault.clone(), max_markdown_bytes),
            vault,
            index,
        }
    }

    /// 이미 안정적으로 읽힌 Markdown snapshot을 incremental upsert합니다.
    ///
    /// `_trash/` 문서는 검색 대상에서 제거합니다.
    ///
    /// # Errors
    ///
    /// `SQLite` projection transaction이 실패하면 [`IndexSyncError`]를 반환합니다.
    pub fn upsert_document(
        &self,
        document: &MarkdownDocument,
    ) -> Result<IndexMutation, IndexSyncError> {
        if !is_searchable(&document.path) {
            self.index.delete(&document.path)?;
            return Ok(IndexMutation::Unchanged);
        }
        let indexed = index_document(document);
        self.index.upsert(&indexed).map_err(IndexSyncError::from)
    }

    /// 삭제 또는 trash 이동된 경로의 projection을 제거합니다.
    ///
    /// # Errors
    ///
    /// `SQLite` projection transaction이 실패하면 [`IndexSyncError`]를 반환합니다.
    pub fn delete_path(&self, path: &MarkdownPath) -> Result<bool, IndexSyncError> {
        self.index.delete(path).map_err(IndexSyncError::from)
    }

    /// 이동 전 경로를 제거하고 새 경로의 projection을 한 transaction에서 반영합니다.
    ///
    /// destination이 `_trash/`이면 source와 destination projection을 모두 제거합니다.
    ///
    /// # Errors
    ///
    /// `SQLite` projection transaction이 실패하면 [`IndexSyncError`]를 반환합니다.
    pub fn move_document(
        &self,
        source: &MarkdownPath,
        destination: &MarkdownDocument,
    ) -> Result<IndexMutation, IndexSyncError> {
        if !is_searchable(&destination.path) {
            self.index.delete(source)?;
            if source != &destination.path {
                self.index.delete(&destination.path)?;
            }
            return Ok(IndexMutation::Unchanged);
        }
        self.index
            .move_document(source, &index_document(destination))
            .map_err(IndexSyncError::from)
    }

    /// Vault 전체를 스캔해 누락·변경·삭제된 projection drift를 조정합니다.
    ///
    /// 숨김 경로, symlink, non-Markdown 파일과 `_trash/`는 탐색하지 않습니다. 읽을 수 없는
    /// Markdown 파일은 집계와 warning에 남기고 검색 projection에서는 제거합니다.
    ///
    /// # Errors
    ///
    /// directory traversal 또는 `SQLite` reconciliation이 실패하면 [`IndexSyncError`]를
    /// 반환하며 기존 index transaction은 변경되지 않습니다.
    pub fn reconcile(&self) -> Result<IndexSyncReport, IndexSyncError> {
        let mut scan = ScanResult::default();
        scan_directory(
            self.vault.canonical_path(),
            &mut Vec::new(),
            &self.reader,
            &mut scan,
        )?;
        let changes = self.index.reconcile(scan.documents)?;
        Ok(IndexSyncReport {
            discovered: scan.discovered,
            skipped: scan.skipped,
            malformed_frontmatter: scan.malformed_frontmatter,
            changes,
        })
    }
}

fn index_document(document: &MarkdownDocument) -> IndexDocument {
    IndexDocument {
        projection: MarkdownProjectionParser::parse(document),
        modified_at: document.modified_at,
    }
}

fn is_searchable(path: &MarkdownPath) -> bool {
    path.as_canonical().segments().next() != Some(TRASH_ROOT)
}

#[derive(Default)]
struct ScanResult {
    documents: Vec<IndexDocument>,
    discovered: u64,
    skipped: u64,
    malformed_frontmatter: u64,
}

fn scan_directory(
    absolute: &Path,
    relative_segments: &mut Vec<String>,
    reader: &MarkdownReader,
    result: &mut ScanResult,
) -> Result<(), IndexSyncError> {
    let entries = fs::read_dir(absolute)
        .map_err(|source| IndexSyncError::ScanIo {
            path: relative_display(relative_segments),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| IndexSyncError::ScanIo {
            path: relative_display(relative_segments),
            source,
        })?;

    for entry in entries {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            result.skipped += 1;
            continue;
        };
        if name.starts_with('.') || (relative_segments.is_empty() && name == TRASH_ROOT) {
            continue;
        }

        let file_type = entry.file_type().map_err(|source| IndexSyncError::ScanIo {
            path: relative_child_display(relative_segments, &name),
            source,
        })?;
        if file_type.is_symlink() {
            result.skipped += 1;
            continue;
        }

        relative_segments.push(name);
        if file_type.is_dir() {
            if crate::domain::path::CanonicalPath::parse(&relative_segments.join("/")).is_err() {
                result.skipped += 1;
            } else {
                scan_directory(&entry.path(), relative_segments, reader, result)?;
            }
            relative_segments.pop();
            continue;
        }
        if !file_type.is_file() {
            result.skipped += 1;
            relative_segments.pop();
            continue;
        }

        let raw_path = relative_segments.join("/");
        relative_segments.pop();
        if Path::new(&raw_path).extension() != Some(OsStr::new("md")) {
            continue;
        }
        result.discovered += 1;
        let path = match MarkdownPath::parse(&raw_path) {
            Ok(path) => path,
            Err(error) => {
                result.skipped += 1;
                tracing::warn!(%error, path = %raw_path, "Markdown path skipped during index scan");
                continue;
            }
        };
        match reader.read(&path) {
            Ok(document) => {
                let indexed = index_document(&document);
                if indexed.projection.frontmatter_status == FrontmatterStatus::Malformed {
                    result.malformed_frontmatter += 1;
                }
                result.documents.push(indexed);
            }
            Err(error) => {
                result.skipped += 1;
                tracing::warn!(
                    %error,
                    path = %path,
                    "Markdown document skipped during index reconciliation"
                );
            }
        }
    }
    Ok(())
}

fn relative_display(segments: &[String]) -> PathBuf {
    if segments.is_empty() {
        PathBuf::from(".")
    } else {
        segments.iter().collect()
    }
}

fn relative_child_display(segments: &[String], child: &str) -> PathBuf {
    let mut path = relative_display(segments);
    path.push(child);
    path
}

#[derive(Debug, Error)]
pub enum IndexSyncError {
    #[error("search index reconciliation scan failed for {path}: {source}")]
    ScanIo {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Read(#[from] MarkdownReadError),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error(transparent)]
    Index(#[from] SearchIndexError),
}
