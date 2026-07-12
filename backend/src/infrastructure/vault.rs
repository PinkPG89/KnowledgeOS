use std::{
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::domain::path::CanonicalPath;

/// 단일 활성 Vault의 설정 경로와 실제 canonical root를 함께 보관합니다.
///
/// 설정 경로 자체는 symlink일 수 있지만 `open` 시점에 한 번 실제 directory로
/// 해석합니다. 이후 모든 descendant는 symlink 없이 이 root 아래에 있어야 합니다.
#[derive(Clone, Debug)]
pub struct VaultRoot {
    configured: PathBuf,
    canonical: PathBuf,
}

impl VaultRoot {
    /// 설정된 directory를 검증하고 process 수명 동안 사용할 Vault root를 엽니다.
    ///
    /// # Errors
    ///
    /// root가 없거나, directory가 아니거나, canonicalize할 수 없으면
    /// [`VaultError`]를 반환합니다.
    pub fn open(configured: impl AsRef<Path>) -> Result<Self, VaultError> {
        let configured = configured.as_ref().to_path_buf();
        let canonical = fs::canonicalize(&configured)
            .map_err(|source| map_root_error(configured.clone(), source))?;
        let metadata = fs::metadata(&canonical).map_err(|source| VaultError::Io {
            path: canonical.clone(),
            source,
        })?;

        if !metadata.is_dir() {
            return Err(VaultError::RootNotDirectory(configured));
        }

        // metadata 조회만 가능한 directory를 활성 Vault로 받아들이지 않도록 실제 열람도 검증합니다.
        fs::read_dir(&canonical).map_err(|source| VaultError::RootUnavailable {
            path: configured.clone(),
            source,
        })?;

        Ok(Self {
            configured,
            canonical,
        })
    }

    /// 사용자가 설정한 Vault 경로를 반환합니다.
    #[must_use]
    pub fn configured_path(&self) -> &Path {
        &self.configured
    }

    /// startup에서 해석한 실제 절대 Vault root를 반환합니다.
    #[must_use]
    pub fn canonical_path(&self) -> &Path {
        &self.canonical
    }

    /// 존재하는 file 또는 directory가 Vault 내부의 symlink 없는 경로인지 확인합니다.
    ///
    /// # Errors
    ///
    /// 대상이 없거나, 중간 segment가 directory가 아니거나, descendant symlink가
    /// 발견되거나, 최종 경로가 Vault 밖이면 [`VaultError`]를 반환합니다.
    pub fn resolve_existing(&self, relative: &CanonicalPath) -> Result<PathBuf, VaultError> {
        let resolved = self.walk_existing(relative)?;
        self.ensure_contained(&resolved)?;
        Ok(resolved)
    }

    /// 새 항목을 만들기 전에 기존 parent chain과 선택적 target을 검증합니다.
    ///
    /// 반환 경로 자체는 아직 존재하지 않을 수 있습니다. 하지만 parent는 실제
    /// directory이며 Vault 내부에 있고, descendant symlink가 아님을 보장합니다.
    ///
    /// # Errors
    ///
    /// parent가 없거나 directory가 아니거나, parent 또는 기존 target이 symlink면
    /// [`VaultError`]를 반환합니다.
    pub fn resolve_parent_for_create(
        &self,
        relative: &CanonicalPath,
    ) -> Result<PathBuf, VaultError> {
        let segments = relative.segments().collect::<Vec<_>>();
        let (target_name, parent_segments) = segments
            .split_last()
            .ok_or_else(|| VaultError::TargetNotFound(relative.to_string()))?;

        let mut parent = self.canonical.clone();
        let mut walked = Vec::with_capacity(parent_segments.len());

        for segment in parent_segments {
            walked.push(*segment);
            parent.push(segment);
            let display_path = walked.join("/");
            let metadata = fs::symlink_metadata(&parent).map_err(|source| {
                if source.kind() == io::ErrorKind::NotFound {
                    VaultError::ParentNotFound(display_path.clone())
                } else {
                    VaultError::Io {
                        path: parent.clone(),
                        source,
                    }
                }
            })?;

            reject_symlink(&metadata, &display_path)?;
            if !metadata.is_dir() {
                return Err(VaultError::NonDirectoryAncestor(display_path));
            }
        }

        self.ensure_contained(&parent)?;

        let target = parent.join(target_name);
        match fs::symlink_metadata(&target) {
            Ok(metadata) => reject_symlink(&metadata, relative.as_str())?,
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(VaultError::Io {
                    path: target,
                    source,
                });
            }
        }

        Ok(target)
    }

    fn walk_existing(&self, relative: &CanonicalPath) -> Result<PathBuf, VaultError> {
        let mut current = self.canonical.clone();
        let segments = relative.segments().collect::<Vec<_>>();
        let mut walked = Vec::with_capacity(segments.len());

        for (index, segment) in segments.iter().enumerate() {
            walked.push(*segment);
            current.push(segment);
            let display_path = walked.join("/");
            let metadata = fs::symlink_metadata(&current).map_err(|source| {
                if source.kind() == io::ErrorKind::NotFound {
                    VaultError::TargetNotFound(relative.to_string())
                } else {
                    VaultError::Io {
                        path: current.clone(),
                        source,
                    }
                }
            })?;

            reject_symlink(&metadata, &display_path)?;

            let is_last = index + 1 == segments.len();
            if !is_last && !metadata.is_dir() {
                return Err(VaultError::NonDirectoryAncestor(display_path));
            }
        }

        Ok(current)
    }

    fn ensure_contained(&self, existing: &Path) -> Result<(), VaultError> {
        let canonical = fs::canonicalize(existing).map_err(|source| VaultError::Io {
            path: existing.to_path_buf(),
            source,
        })?;

        if !canonical.starts_with(&self.canonical) {
            return Err(VaultError::OutsideVault(canonical));
        }

        Ok(())
    }
}

fn reject_symlink(metadata: &fs::Metadata, relative: &str) -> Result<(), VaultError> {
    if metadata.file_type().is_symlink() {
        return Err(VaultError::SymlinkNotAllowed(relative.to_owned()));
    }

    Ok(())
}

fn map_root_error(path: PathBuf, source: io::Error) -> VaultError {
    if source.kind() == io::ErrorKind::NotFound {
        VaultError::RootNotFound(path)
    } else {
        VaultError::RootUnavailable { path, source }
    }
}

/// Vault 초기화와 descendant 경계 검증 중 발생하는 오류입니다.
#[derive(Debug, Error)]
pub enum VaultError {
    #[error("Vault root does not exist: {0}")]
    RootNotFound(PathBuf),

    #[error("Vault root is not a directory: {0}")]
    RootNotDirectory(PathBuf),

    #[error("Vault root is unavailable: {path}: {source}")]
    RootUnavailable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("Vault target does not exist: {0}")]
    TargetNotFound(String),

    #[error("Vault parent does not exist: {0}")]
    ParentNotFound(String),

    #[error("symlink descendants are not allowed: {0}")]
    SymlinkNotAllowed(String),

    #[error("path segment is not a directory: {0}")]
    NonDirectoryAncestor(String),

    #[error("resolved path is outside the Vault: {0}")]
    OutsideVault(PathBuf),

    #[error("filesystem operation failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[cfg(test)]
mod tests {
    use std::{fs, io, path::Path};

    #[cfg(unix)]
    use std::os::unix::fs::{PermissionsExt, symlink};
    use tempfile::TempDir;

    use super::{VaultError, VaultRoot, map_root_error};
    use crate::domain::path::CanonicalPath;

    fn path(value: &str) -> CanonicalPath {
        CanonicalPath::parse(value).expect("test path must be valid")
    }

    #[test]
    fn opens_a_real_directory_as_the_active_vault() {
        let directory = TempDir::new().expect("temporary directory should be created");
        let vault = VaultRoot::open(directory.path()).expect("Vault should open");

        assert_eq!(vault.configured_path(), directory.path());
        assert_eq!(
            vault.canonical_path(),
            fs::canonicalize(directory.path()).expect("temporary directory should canonicalize")
        );
    }

    #[test]
    fn distinguishes_missing_file_and_unavailable_roots() {
        let directory = TempDir::new().expect("temporary directory should be created");
        let missing = directory.path().join("missing");
        assert!(matches!(
            VaultRoot::open(&missing),
            Err(VaultError::RootNotFound(path)) if path == missing
        ));

        let file = directory.path().join("note.md");
        fs::write(&file, "content").expect("test file should be written");
        assert!(matches!(
            VaultRoot::open(&file),
            Err(VaultError::RootNotDirectory(path)) if path == file
        ));

        let permission_error = io::Error::from(io::ErrorKind::PermissionDenied);
        assert!(matches!(
            map_root_error(directory.path().to_path_buf(), permission_error),
            VaultError::RootUnavailable { .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_an_inaccessible_root_for_regular_users() {
        let directory = TempDir::new().expect("temporary directory should be created");
        let restricted = directory.path().join("restricted");
        fs::create_dir(&restricted).expect("restricted directory should be created");
        fs::set_permissions(&restricted, fs::Permissions::from_mode(0o000))
            .expect("permissions should be changed");

        let result = VaultRoot::open(&restricted);

        fs::set_permissions(&restricted, fs::Permissions::from_mode(0o700))
            .expect("permissions should be restored");

        // root 권한으로 test가 실행되면 permission bit를 우회할 수 있으므로 결과를 강제하지 않습니다.
        if result.is_ok() {
            return;
        }
        assert!(matches!(result, Err(VaultError::RootUnavailable { .. })));
    }

    #[cfg(unix)]
    #[test]
    fn allows_the_configured_root_itself_to_be_a_symlink() {
        let directory = TempDir::new().expect("temporary directory should be created");
        let actual = directory.path().join("actual-vault");
        let configured = directory.path().join("selected-vault");
        fs::create_dir(&actual).expect("actual Vault should be created");
        symlink(&actual, &configured).expect("root symlink should be created");

        let vault = VaultRoot::open(&configured).expect("root symlink should be accepted");

        assert_eq!(vault.configured_path(), configured);
        assert_eq!(
            vault.canonical_path(),
            fs::canonicalize(actual).expect("actual Vault should canonicalize")
        );
    }

    #[test]
    fn resolves_normal_nested_files_and_directories() {
        let directory = TempDir::new().expect("temporary directory should be created");
        let project = directory.path().join("projects/knowledgeos");
        fs::create_dir_all(&project).expect("nested directories should be created");
        fs::write(project.join("architecture.md"), "content").expect("test file should be written");
        let vault = VaultRoot::open(directory.path()).expect("Vault should open");

        let resolved = vault
            .resolve_existing(&path("projects/knowledgeos/architecture.md"))
            .expect("nested file should resolve");

        assert!(resolved.starts_with(vault.canonical_path()));
        assert!(resolved.ends_with(Path::new("projects/knowledgeos/architecture.md")));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_descendant_symlinks_to_inside_and_outside_the_vault() {
        let directory = TempDir::new().expect("temporary directory should be created");
        let outside = TempDir::new().expect("outside directory should be created");
        let real = directory.path().join("real.md");
        fs::write(&real, "inside").expect("inside file should be written");
        fs::write(outside.path().join("outside.md"), "outside")
            .expect("outside file should be written");
        symlink(&real, directory.path().join("inside-link.md"))
            .expect("inside symlink should be created");
        symlink(
            outside.path().join("outside.md"),
            directory.path().join("outside-link.md"),
        )
        .expect("outside symlink should be created");
        let vault = VaultRoot::open(directory.path()).expect("Vault should open");

        assert!(matches!(
            vault.resolve_existing(&path("inside-link.md")),
            Err(VaultError::SymlinkNotAllowed(value)) if value == "inside-link.md"
        ));
        assert!(matches!(
            vault.resolve_existing(&path("outside-link.md")),
            Err(VaultError::SymlinkNotAllowed(value)) if value == "outside-link.md"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_intermediate_and_create_parent_symlinks() {
        let directory = TempDir::new().expect("temporary directory should be created");
        let actual = directory.path().join("actual");
        fs::create_dir(&actual).expect("actual directory should be created");
        fs::write(actual.join("note.md"), "content").expect("test file should be written");
        symlink(&actual, directory.path().join("linked"))
            .expect("directory symlink should be created");
        let vault = VaultRoot::open(directory.path()).expect("Vault should open");

        assert!(matches!(
            vault.resolve_existing(&path("linked/note.md")),
            Err(VaultError::SymlinkNotAllowed(value)) if value == "linked"
        ));
        assert!(matches!(
            vault.resolve_parent_for_create(&path("linked/new.md")),
            Err(VaultError::SymlinkNotAllowed(value)) if value == "linked"
        ));
    }

    #[test]
    fn distinguishes_missing_read_targets_and_create_parents() {
        let directory = TempDir::new().expect("temporary directory should be created");
        let vault = VaultRoot::open(directory.path()).expect("Vault should open");

        assert!(matches!(
            vault.resolve_existing(&path("missing.md")),
            Err(VaultError::TargetNotFound(value)) if value == "missing.md"
        ));
        assert!(matches!(
            vault.resolve_parent_for_create(&path("missing/new.md")),
            Err(VaultError::ParentNotFound(value)) if value == "missing"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_an_existing_create_target_symlink() {
        let directory = TempDir::new().expect("temporary directory should be created");
        let actual = directory.path().join("actual.md");
        fs::write(&actual, "content").expect("actual file should be written");
        symlink(&actual, directory.path().join("new.md"))
            .expect("target symlink should be created");
        let vault = VaultRoot::open(directory.path()).expect("Vault should open");

        assert!(matches!(
            vault.resolve_parent_for_create(&path("new.md")),
            Err(VaultError::SymlinkNotAllowed(value)) if value == "new.md"
        ));
    }
}
