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

/// Vault의 특정 디렉터리에서 직계 자식(Direct Children)만 조회하는 파일 시스템 어댑터입니다.
///
/// 지식 저장소(Vault)의 트리 구조를 탐색할 때 사용되며, 성능과 효율성을 위해
/// 전체 트리를 한 번에 읽지 않고 요청된 디렉터리의 직계 자식 노드만 지연 조회(Lazy Loading)합니다.
#[derive(Clone, Debug)]
pub struct TreeReader {
    /// 경로 검증 및 보안 격리를 수행하는 Vault 루트 객체
    vault: VaultRoot,
}

impl TreeReader {
    /// 새로운 `TreeReader` 인스턴스를 상수 생성자(`const fn`)를 통해 생성합니다.
    #[must_use]
    pub const fn new(vault: VaultRoot) -> Self {
        Self { vault }
    }

    /// 지정된 디렉터리의 직계 자식 목록을 안전하게 조회합니다.
    ///
    /// `directory` 매개변수가 `None`이면 Vault의 루트 디렉터리를 가리키고,
    /// `Some(path)`이면 Vault 내부의 검증된 하위 디렉터리를 가리킵니다.
    ///
    /// # Errors
    ///
    /// 다음 상황에서 `TreeReadError`를 반환합니다:
    /// * 대상 경로가 존재하지 않는 경우 (`NotFound`)
    /// * 대상이 디렉터리가 아닌 경우 (`NotDirectory`)
    /// * 중간 경로에 심볼릭 링크가 존재하거나 Vault 영역 외부를 참조하여 보안 정책을 위반한 경우 (`Vault`)
    /// * 디렉터리를 읽거나 메타데이터를 조회하는 중 OS I/O 에러가 발생한 경우
    pub fn list(
        &self,
        directory: Option<&CanonicalPath>,
    ) -> Result<DirectoryListing, TreeReadError> {
        // 1. 디렉터리 경로의 공개용 문자열 표현을 생성합니다. (에러 발생 시 출력용)
        let public_path = directory.map_or_else(String::new, ToString::to_string);

        // 2. 디렉터리가 지정된 경우 Vault의 경로 검증 규칙에 따라 물리 절대 경로를 구하고,
        //    지정되지 않은(None) 경우 Vault 루트의 절대 물리 경로를 사용합니다.
        let absolute = match directory {
            Some(path) => self
                .vault
                .resolve_existing(path)
                .map_err(TreeReadError::Vault)?,
            None => self.vault.canonical_path().to_path_buf(),
        };

        // 3. 대상 디렉터리 자체의 메타데이터를 조회합니다. (심볼릭 링크 여부 검사 포함)
        let metadata = fs::symlink_metadata(&absolute)
            .map_err(|source| map_target_error(&public_path, &absolute, source))?;

        // 4. 대상 디렉터리가 심볼릭 링크인 경우 보안 정책에 따라 즉각 에러를 반환합니다.
        if metadata.file_type().is_symlink() {
            return Err(TreeReadError::Vault(VaultError::SymlinkNotAllowed(
                public_path,
            )));
        }

        // 5. 대상이 디렉터리가 아닌 경우(예: 일반 파일) 에러를 반환합니다.
        if !metadata.is_dir() {
            return Err(TreeReadError::NotDirectory(public_path));
        }

        // 6. 디렉터리 내부를 스캔하여 자식 항목(엔트리) 목록을 가져옵니다.
        let entries = scan_directory(&absolute, directory)?;

        // 7. 조회한 디렉터리 경로와 자식 엔트리 목록을 결합하여 도메인 모델 객체를 반환합니다.
        Ok(DirectoryListing {
            path: directory.cloned(),
            entries,
        })
    }
}

/// 디렉터리의 실제 자식 항목들을 스캔합니다.
fn scan_directory(
    absolute: &Path,
    directory: Option<&CanonicalPath>,
) -> Result<Vec<TreeEntry>, TreeReadError> {
    // 실제 파일 시스템의 메타데이터 조회 함수(`fs::symlink_metadata`)를 주입하여 스캔을 위임합니다.
    scan_directory_with(absolute, directory, |path| fs::symlink_metadata(path))
}

/// 디렉터리 스캔을 수행하는 핵심 로직입니다.
/// 메타데이터 조회 방식을 주입(Dependency Injection)받도록 설계하여,
/// 스캔 중에 파일이 사라지는 동시성 레이스 컨디션 등의 상황을 결정적으로 테스트할 수 있게 합니다.
fn scan_directory_with(
    absolute: &Path,
    directory: Option<&CanonicalPath>,
    mut read_metadata: impl FnMut(&Path) -> io::Result<Metadata>,
) -> Result<Vec<TreeEntry>, TreeReadError> {
    let mut entries = Vec::new();

    // 1. 대상 디렉터리의 스트림을 엽니다.
    let children = fs::read_dir(absolute).map_err(|source| TreeReadError::ReadDirectory {
        path: absolute.to_path_buf(),
        source,
    })?;

    // 2. 디렉터리 내부 항목들을 순회합니다.
    for child in children {
        let child = child.map_err(|source| TreeReadError::ReadDirectory {
            path: absolute.to_path_buf(),
            source,
        })?;
        let child_absolute = child.path();

        // 3. 각 자식 항목의 메타데이터를 조회합니다.
        //    스캔하는 도중 다른 프로세스에 의해 파일이 삭제되는 경우(`ErrorKind::NotFound`),
        //    에러를 던져 전체 조회를 실패시키지 않고 해당 항목만 건너뛰고(skip) 스캔을 계속 진행합니다.
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

        // 4. 파일 이름이 올바른 UTF-8 문자열이 아닌 경우 건너뜁니다.
        let Some(name) = child.file_name().to_str().map(str::to_owned) else {
            continue;
        };

        // 5. 항목의 메타데이터와 절대 경로를 기반으로 도메인 TreeEntry 객체를 생성합니다.
        //    숨김 파일이거나 지원하지 않는 파일 형식인 경우 생성되지 않고 None을 반환하므로, 이를 건너뜁니다.
        let Some(entry) = build_entry(directory, name, &metadata, child_absolute)? else {
            continue;
        };
        entries.push(entry);
    }

    // 6. 스캔된 엔트리들을 정렬 규칙에 맞춰 정렬합니다.
    //    정렬 규칙: 디렉터리가 파일보다 항상 앞에 오고 (rank 0 < rank 1),
    //    종류가 같은 항목들끼리는 locale 보정 없이 Rust 문자열 순서로 오름차순 정렬합니다.
    entries.sort_by(|left, right| {
        entry_rank(left.kind)
            .cmp(&entry_rank(right.kind))
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(entries)
}

/// 개별 디렉터리 엔트리(자식 항목)를 빌드합니다.
/// 숨김 파일(점 `.`으로 시작)이나 심볼릭 링크는 보안 및 시스템 보호를 위해 무시(None)합니다.
fn build_entry(
    directory: Option<&CanonicalPath>,
    name: String,
    metadata: &Metadata,
    absolute: PathBuf,
) -> Result<Option<TreeEntry>, TreeReadError> {
    // 1. 점(`.`)으로 시작하는 숨김 폴더/파일이거나 심볼릭 링크인 경우 즉시 필터링(무시)합니다.
    if name.starts_with('.') || metadata.file_type().is_symlink() {
        return Ok(None);
    }

    // 2. 부모 디렉터리 경로와 자식 파일명을 조합하여 상대 경로 문자열을 빌드합니다.
    let relative = match directory {
        Some(parent) => format!("{}/{name}", parent.as_str()),
        None => name.clone(),
    };

    // 3. 항목의 타입(디렉터리 vs 파일)에 맞추어 도메인 경로 파싱을 시도하고 분류합니다.
    let (kind, path, size) = if metadata.is_dir() {
        // 디렉터리인 경우: CanonicalPath 규칙을 충족하는지 검증합니다.
        let Ok(path) = CanonicalPath::parse(&relative) else {
            return Ok(None);
        };
        (TreeEntryKind::Directory, path, None)
    } else if metadata.is_file() {
        // 파일인 경우: MarkdownPath 규칙(예: `.md` 확장자 등)을 충족하는지 검증합니다.
        let Ok(path) = MarkdownPath::parse(&relative) else {
            return Ok(None);
        };
        (
            TreeEntryKind::File,
            path.as_canonical().clone(),
            Some(metadata.len()),
        )
    } else {
        // 디렉터리도 파일도 아닌 경우 (예: 디바이스 장치, 소켓 등) 필터링합니다.
        return Ok(None);
    };

    // 4. 파일의 최종 수정 시간(modified time)을 조회합니다.
    let modified_at = metadata
        .modified()
        .map_err(|source| TreeReadError::Metadata {
            path: absolute,
            source,
        })?;

    // 5. 모든 검증을 마친 유효한 트리 엔트리 객체를 반환합니다.
    Ok(Some(TreeEntry {
        kind,
        name,
        path,
        size,
        modified_at,
    }))
}

/// 디렉터리와 파일 항목 간의 정렬 우선순위 랭크를 정의합니다.
/// 디렉터리(0)가 파일(1)보다 높은 우선순위(낮은 랭크값)를 가집니다.
const fn entry_rank(kind: TreeEntryKind) -> u8 {
    match kind {
        TreeEntryKind::Directory => 0,
        TreeEntryKind::File => 1,
    }
}

/// OS에서 발생한 파일 조회 에러를 도메인 관점의 상세 에러 타입으로 변환합니다.
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

/// 트리 조회 과정에서 발생할 수 있는 오류들을 정의하는 에러 열거형입니다.
#[derive(Debug, Error)]
pub enum TreeReadError {
    /// Vault 영역 검증 및 경로 해석 실패
    #[error(transparent)]
    Vault(#[from] VaultError),

    /// 조회하려는 디렉터리가 실제 파일 시스템에 존재하지 않는 경우
    #[error("directory does not exist: {0}")]
    NotFound(String),

    /// 조회하려는 경로가 존재하지만 디렉터리가 아닌 일반 파일인 경우
    #[error("path is not a directory: {0}")]
    NotDirectory(String),

    /// 디렉터리 목록 열람(`read_dir`)을 시도하는 중에 발생한 시스템 I/O 오류
    #[error("failed to read directory {path}: {source}")]
    ReadDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// 개별 파일의 수정 시간, 파일 형식 등 메타데이터 정보를 획득하는 과정에서의 에러
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
