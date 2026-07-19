use std::{
    fs::{self, Metadata},
    io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::domain::{
    path::{CanonicalPath, MarkdownPath},
    tree::{DirectoryListing, TreeEntry, TreeEntryKind},
};

use super::vault::{VaultError, VaultRoot};

/// Vault의 특정 디렉터리에서 직계 자식만 조회하는 filesystem adapter입니다.
#[derive(Clone, Debug)]
pub struct TreeReader {
    vault: VaultRoot,
}

impl TreeReader {
    #[must_use]
    pub const fn new(vault: VaultRoot) -> Self {
        Self { vault }
    }

    /// None은 Vault root, Some은 검증된 하위 디렉터리를 의미합니다.
    ///
    /// # Errors
    ///
    /// 대상이 없거나 디렉터리가 아니거나, Vault 정책을 위반하거나, 목록 조회 중 I/O가 실패하면
    /// `TreeReadError`를 반환합니다.
    pub fn list(
        &self,
        directory: Option<&CanonicalPath>,
    ) -> Result<DirectoryListing, TreeReadError> {
        let public_path = directory.map_or_else(String::new, ToString::to_string);
        let absolute = match directory {
            Some(path) => self
                .vault
                .resolve_existing(path)
                .map_err(TreeReadError::Vault)?,
            None => self.vault.canonical_path().to_path_buf(),
        };

        let metadata = fs::symlink_metadata(&absolute)
            .map_err(|source| map_target_error(&public_path, &absolute, source))?;
        if metadata.file_type().is_symlink() {
            return Err(TreeReadError::Vault(VaultError::SymlinkNotAllowed(
                public_path,
            )));
        }
        if !metadata.is_dir() {
            return Err(TreeReadError::NotDirectory(public_path));
        }

        let entries = scan_directory(&absolute, directory)?;
        Ok(DirectoryListing {
            path: directory.cloned(),
            entries,
        })
    }
}

fn scan_directory(
    absolute: &Path,
    directory: Option<&CanonicalPath>,
) -> Result<Vec<TreeEntry>, TreeReadError> {
    scan_directory_with(absolute, directory, |path| fs::symlink_metadata(path))
}

/// metadata 조회를 주입 가능하게 두어 scan 중 항목 소멸 race를 결정적으로 테스트합니다.
fn scan_directory_with(
    absolute: &Path,
    directory: Option<&CanonicalPath>,
    mut read_metadata: impl FnMut(&Path) -> io::Result<Metadata>,
) -> Result<Vec<TreeEntry>, TreeReadError> {
    let mut entries = Vec::new();
    let children = fs::read_dir(absolute).map_err(|source| TreeReadError::ReadDirectory {
        path: absolute.to_path_buf(),
        source,
    })?;

    for child in children {
        let child = child.map_err(|source| TreeReadError::ReadDirectory {
            path: absolute.to_path_buf(),
            source,
        })?;
        let child_absolute = child.path();
        let metadata = match read_metadata(&child_absolute) {
            Ok(metadata) => metadata,
            Err(source) if source.kind() == io::ErrorKind::NotFound => continue,
            Err(source) => {
                return Err(TreeReadError::Metadata {
                    path: child_absolute,
                    source,
                });
            }
        };

        let Some(name) = child.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(entry) = build_entry(directory, name, &metadata, child_absolute)? else {
            continue;
        };
        entries.push(entry);
    }

    entries.sort_by(|left, right| {
        entry_rank(left.kind)
            .cmp(&entry_rank(right.kind))
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(entries)
}

fn build_entry(
    directory: Option<&CanonicalPath>,
    name: String,
    metadata: &Metadata,
    absolute: PathBuf,
) -> Result<Option<TreeEntry>, TreeReadError> {
    if name.starts_with('.') || metadata.file_type().is_symlink() {
        return Ok(None);
    }

    let relative = match directory {
        Some(parent) => format!("{}/{name}", parent.as_str()),
        None => name.clone(),
    };

    let (kind, path, size) = if metadata.is_dir() {
        let Ok(path) = CanonicalPath::parse(&relative) else {
            return Ok(None);
        };
        (TreeEntryKind::Directory, path, None)
    } else if metadata.is_file() {
        let Ok(path) = MarkdownPath::parse(&relative) else {
            return Ok(None);
        };
        (
            TreeEntryKind::File,
            path.as_canonical().clone(),
            Some(metadata.len()),
        )
    } else {
        return Ok(None);
    };

    let modified_at = metadata
        .modified()
        .map_err(|source| TreeReadError::Metadata {
            path: absolute,
            source,
        })?;

    Ok(Some(TreeEntry {
        kind,
        name,
        path,
        size,
        modified_at,
    }))
}

const fn entry_rank(kind: TreeEntryKind) -> u8 {
    match kind {
        TreeEntryKind::Directory => 0,
        TreeEntryKind::File => 1,
    }
}

fn map_target_error(public_path: &str, absolute: &Path, source: io::Error) -> TreeReadError {
    if source.kind() == io::ErrorKind::NotFound {
        TreeReadError::NotFound(public_path.to_owned())
    } else {
        TreeReadError::Metadata {
            path: absolute.to_path_buf(),
            source,
        }
    }
}

#[derive(Debug, Error)]
pub enum TreeReadError {
    #[error(transparent)]
    Vault(#[from] VaultError),

    #[error("directory does not exist: {0}")]
    NotFound(String),

    #[error("path is not a directory: {0}")]
    NotDirectory(String),

    #[error("failed to read directory {path}: {source}")]
    ReadDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to read tree metadata for {path}: {source}")]
    Metadata {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[cfg(test)]
mod tests {
    use std::{fs, io};

    use tempfile::TempDir;

    use super::{TreeReadError, TreeReader, scan_directory_with};
    use crate::{domain::path::CanonicalPath, infrastructure::vault::VaultRoot};

    #[test]
    fn skips_a_child_that_disappears_during_scan() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        let disappearing = directory.path().join("gone.md");
        fs::write(&disappearing, "gone").expect("Markdown should be written");
        fs::write(directory.path().join("kept.md"), "kept").expect("Markdown should be written");

        let entries = scan_directory_with(directory.path(), None, |path| {
            if path == disappearing {
                Err(io::Error::from(io::ErrorKind::NotFound))
            } else {
                fs::symlink_metadata(path)
            }
        })
        .expect("disappearing child should be skipped");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "kept.md");
    }

    #[test]
    fn returns_non_not_found_metadata_errors() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        fs::write(directory.path().join("note.md"), "note").expect("Markdown should be written");

        let error = scan_directory_with(directory.path(), None, |_| {
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"))
        })
        .expect_err("metadata failure should abort the listing");

        assert!(matches!(error, TreeReadError::Metadata { .. }));
    }

    #[test]
    fn lists_nested_directory_through_vault_policy() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        fs::create_dir(directory.path().join("projects"))
            .expect("nested directory should be created");
        fs::write(directory.path().join("projects/note.md"), "note")
            .expect("Markdown should be written");
        let reader = TreeReader::new(
            VaultRoot::open(directory.path()).expect("test Vault should initialize"),
        );
        let path = CanonicalPath::parse("projects").expect("path should be valid");

        let listing = reader.list(Some(&path)).expect("directory should list");

        assert_eq!(listing.path, Some(path));
        assert_eq!(listing.entries.len(), 1);
        assert_eq!(listing.entries[0].path.as_str(), "projects/note.md");
    }
}
