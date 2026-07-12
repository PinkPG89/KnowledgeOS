use std::{
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::domain::path::CanonicalPath;

/// 단일 활성 지식 저장소(Vault)의 사용자가 설정한 파일 경로와
/// 운영체제(OS)가 해석한 실제 정규 절대 물리 경로를 쌍으로 보관하는 구조체입니다.
///
/// ## 심볼릭 링크(Symlink) 무력화 보안 아키텍처
/// - 사용자가 설정한 최초 경로 자체는 심볼릭 링크일 수 있습니다. (예: `../knowledge` 링크 폴더)
/// - 그러나 서버가 시작되거나 Vault를 개방(`open`)하는 시점에 단 한 번, 실제 대상의 진짜 절대 경로(`canonical`)로 완전히 해석합니다.
/// - 이 작업 이후에 생성되거나 읽히는 Vault 하위의 모든 후손(Descendant) 경로들은 중간에 어떠한 심볼릭 링크도 포함할 수 없도록 강제합니다.
///   (이를 통해 링크 파일 뒤에 민감한 시스템 파일 `/etc/passwd` 등을 숨기는 우회 위험을 줄입니다.)
#[derive(Clone, Debug)]
pub struct VaultRoot {
    /// 사용자가 부팅 옵션이나 설정 파일에 지정해놓은 저장소 파일 경로
    configured: PathBuf,
    /// 운영체제(OS)가 물리 디스크를 기준으로 추적해낸 단 하나의 정규 절대 경로
    canonical: PathBuf,
}

impl VaultRoot {
    /// 사용자가 지정한 저장소 디렉터리 경로를 물리적으로 철저하게 검증하고,
    /// 백엔드 프로세스의 수명 동안 신뢰하고 사용할 수 있는 `VaultRoot` 인스턴스를 열어 둡니다.
    ///
    /// ## `impl AsRef<Path>` 제네릭
    /// 이 함수는 `AsRef<Path>` 트레이트를 구현하는 어떤 타입이든 입력으로 허용합니다.
    /// 따라서 호출자는 일반 문자열 `&str`, `String`, `PathBuf`, 혹은 `&Path` 타입을 유연하게 넘겨줄 수 있습니다.
    ///
    /// # Errors
    ///
    /// 디렉터리가 실제로 존재하지 않거나, 존재해도 일반 파일일 뿐 디렉터리가 아니거나,
    /// 운영체제가 물리 절대 경로로 정규화(`canonicalize`)할 수 없거나, 읽기/검색 권한이 없는 경우 [`VaultError`]를 던집니다.
    pub fn open(configured: impl AsRef<Path>) -> Result<Self, VaultError> {
        // 입력값을 소유권을 가진 경로 버퍼인 `PathBuf` 형태로 받아옵니다.
        let configured = configured.as_ref().to_path_buf();

        // 1. fs::canonicalize를 호출하여 상위 경로참조(..)나 링크를 물리 절대 경로로 일관되게 해석해 냅니다.
        let canonical = fs::canonicalize(&configured)
            .map_err(|source| map_root_error(configured.clone(), source))?;

        // 2. 해당 물리 경로의 메타데이터(파일 종류, 권한 등)를 조회합니다.
        let metadata = fs::metadata(&canonical).map_err(|source| VaultError::Io {
            path: canonical.clone(),
            source,
        })?;

        // 3. 대상이 폴더가 아니라 일반 텍스트 파일 등이라면 저장소로 삼을 수 없으므로 거부합니다.
        if !metadata.is_dir() {
            return Err(VaultError::RootNotDirectory(configured));
        }

        // 4. 경로 메타데이터 조회만 되고 실제 디렉터리 목록 열람(열기 권한)은 되지 않는
        //    불완전한 경로를 차단하기 위해 디렉터리 자식 노드들을 시험 삼아 한 번 읽어 봅니다.
        fs::read_dir(&canonical).map_err(|source| VaultError::RootUnavailable {
            path: configured.clone(),
            source,
        })?;

        // 검증이 정상 완결되면 초기화된 객체를 돌려줍니다.
        Ok(Self {
            configured,
            canonical,
        })
    }

    /// 사용자가 최초에 설정한 원본 저장소 경로를 참조합니다.
    #[must_use]
    pub fn configured_path(&self) -> &Path {
        &self.configured
    }

    /// 서버 시동 단계에서 해석된 실제 정규화 절대 저장소 경로를 참조합니다.
    #[must_use]
    pub fn canonical_path(&self) -> &Path {
        &self.canonical
    }

    /// 가상 또는 실존하는 파일/폴더 경로가 Vault 내부의 정당한 바운더리 내에 위치하는지 실제 디스크를 뒤져 검증합니다.
    ///
    /// # Errors
    ///
    /// 하위 대상 파일이 없거나, 중간 디렉터리가 폴더가 아니거나, 중간 세그먼트 중 심볼릭 링크가 검출되거나,
    /// 최종 절대 경로의 루트가 Vault 영역 밖으로 뻗어 있으면 [`VaultError`]를 던집니다.
    pub fn resolve_existing(&self, relative: &CanonicalPath) -> Result<PathBuf, VaultError> {
        // 1. 실제로 경로를 하나씩 밟아가며 확인합니다.
        let resolved = self.walk_existing(relative)?;
        // 2. 최종 결과 절대 물리 경로가 우리 저장소 폴더 바운더리 내부인지 확인합니다.
        self.ensure_contained(&resolved)?;
        Ok(resolved)
    }

    /// 새로운 문서나 폴더를 물리적으로 새로 생성하기 전에, 목표 대상의 부모 디렉터리 구조가 안전하게 존재하는지 검증합니다.
    ///
    /// 반환되는 최종 파일 경로는 '새로 만들 예정'이므로 아직 존재하지 않을 수 있습니다.
    /// 하지만 그 직전 단계의 부모 경로는 반드시 실존하는 폴더여야 하고, Vault 내부에 격리되어 있으며,
    /// 중간 경로에 어떠한 심볼릭 링크도 존재하지 않음을 강하게 보장해 줍니다.
    ///
    /// # Errors
    ///
    /// 부모 경로가 유실되었거나 디렉터리가 아니거나, 타겟의 부모 체인 중 하나라도 심볼릭 링크인 경우 [`VaultError`]를 리턴합니다.
    pub fn resolve_parent_for_create(
        &self,
        relative: &CanonicalPath,
    ) -> Result<PathBuf, VaultError> {
        // 상대 경로에서 개별 폴더/파일명 단위를 분해합니다.
        let segments = relative.segments().collect::<Vec<_>>();
        // 마지막 타겟(새로 만들 파일명)과 그 앞의 부모 폴더 경로들을 분리합니다.
        let (target_name, parent_segments) = segments
            .split_last()
            .ok_or_else(|| VaultError::TargetNotFound(relative.to_string()))?;

        let mut parent = self.canonical.clone();
        let mut walked = Vec::with_capacity(parent_segments.len());

        // 부모 폴더 체인을 최상위 루트로부터 한 단계씩 실제로 내려가며 검증합니다.
        for segment in parent_segments {
            walked.push(*segment);
            parent.push(segment);
            let display_path = walked.join("/");

            // `fs::symlink_metadata`는 가리키는 실제 대상을 쫓아가지 않고,
            // 현재 타겟 노드 자체가 링크 파일인지 검사하기 위해 사용됩니다. (보안상의 핵심 기능)
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

            // 부모 경로 자체가 심볼릭 링크라면 우회 통로가 존재하므로 즉각 탈락시킵니다.
            reject_symlink(&metadata, &display_path)?;

            // 부모 경로의 중간에 일반 텍스트 파일이 위치해 있으면 더 이상 내려갈 수 없으므로 에러 처리합니다.
            if !metadata.is_dir() {
                return Err(VaultError::NonDirectoryAncestor(display_path));
            }
        }

        // 완성된 부모 경로가 안전하게 Vault 내부 바운더리 밑에 포함되어 있는지 최종 체크합니다.
        self.ensure_contained(&parent)?;

        // 부모 경로 아래에 우리가 생성하고자 하는 새로운 타겟 파일의 명칭을 덧붙입니다.
        let target = parent.join(target_name);

        // 생성할 목표 명칭과 완전히 똑같은 파일이 기존에 이미 존재하는데 심볼릭 링크로 위장해 있다면 거부해야 합니다.
        match fs::symlink_metadata(&target) {
            Ok(metadata) => reject_symlink(&metadata, relative.as_str())?,
            // 대상 파일이 아직 생성 전이라서 존재하지 않는 상태(NotFound)라면 지극히 정상적인 흐름입니다.
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

    /// 파일 시스템 상에 실존하는 기존 경로를 루트부터 상대 세그먼트 순서대로 한 계단씩 밟아나가며
    /// 중간 경로가 정상 폴더인지, 심볼릭 링크 같은 해킹 우회 패턴이 숨어 있지 않은지 정밀 추적합니다.
    fn walk_existing(&self, relative: &CanonicalPath) -> Result<PathBuf, VaultError> {
        let mut current = self.canonical.clone();
        let segments = relative.segments().collect::<Vec<_>>();
        let mut walked = Vec::with_capacity(segments.len());

        for (index, segment) in segments.iter().enumerate() {
            walked.push(*segment);
            current.push(segment);
            let display_path = walked.join("/");

            // 링크를 타지 않고 오직 해당 위치의 현재 파일 메타데이터(심볼릭 링크 파일 자체의 정보)만 낚아챕니다.
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

            // 중간 경로가 링크 파일인 것이 확인되는 순간 폭발(reject)시킵니다.
            reject_symlink(&metadata, &display_path)?;

            // 경로의 가장 끝부분(단말 노드)이 아닌데도 중간 경로 속성이 폴더(directory)가 아니면 구조적 오류입니다.
            let is_last = index + 1 == segments.len();
            if !is_last && !metadata.is_dir() {
                return Err(VaultError::NonDirectoryAncestor(display_path));
            }
        }

        Ok(current)
    }

    /// 최종 해석된 절대 경로를 시스템 디스크를 기준으로 한 번 더 정규화(`canonicalize`)한 뒤,
    /// 이 서버의 마크다운 저장소 절대 경로(`self.canonical`)로 온전히 시작하는지 접두사(`starts_with`) 검사를 하여
    /// 절대 경로 바운더리 밖(예: `/etc`, `/usr` 등)으로 탈출하려는 시도를 완벽히 잠재웁니다.
    fn ensure_contained(&self, existing: &Path) -> Result<(), VaultError> {
        let canonical = fs::canonicalize(existing).map_err(|source| VaultError::Io {
            path: existing.to_path_buf(),
            source,
        })?;

        // 정규화 절대 경로가 우리 Vault 루트 절대 경로로 기점하지 않으면 격리 위반입니다.
        if !canonical.starts_with(&self.canonical) {
            return Err(VaultError::OutsideVault(canonical));
        }

        Ok(())
    }
}

/// 경로 요소가 심볼릭 링크 형태인 경우 이를 탐지하고 에러로 반환시킵니다.
fn reject_symlink(metadata: &fs::Metadata, relative: &str) -> Result<(), VaultError> {
    if metadata.file_type().is_symlink() {
        return Err(VaultError::SymlinkNotAllowed(relative.to_owned()));
    }

    Ok(())
}

/// Vault 루트 개방 시 발생한 에러를 분석하여 가독성 높은 구체적 에러 타입으로 변환 매핑합니다.
fn map_root_error(path: PathBuf, source: io::Error) -> VaultError {
    if source.kind() == io::ErrorKind::NotFound {
        VaultError::RootNotFound(path)
    } else {
        VaultError::RootUnavailable { path, source }
    }
}

/// Vault의 파일 시스템 입출력 라이프사이클 및 보안 검증 시 발생하는 디테일한 에러 열거형입니다.
#[derive(Debug, Error)]
pub enum VaultError {
    /// 지정된 Vault 루트 폴더를 디스크 상에서 찾을 수 없을 때 발생합니다.
    #[error("Vault root does not exist: {0}")]
    RootNotFound(PathBuf),

    /// Vault 루트로 지정한 타겟이 디렉터리가 아닌 일반 파일 등일 때 발생합니다.
    #[error("Vault root is not a directory: {0}")]
    RootNotDirectory(PathBuf),

    /// 디렉터리 목록 열람 권한이 차단되어 있거나 기타 시스템 I/O 에러로 사용 불가할 때 발생합니다.
    #[error("Vault root is unavailable: {path}: {source}")]
    RootUnavailable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// 조회하려는 대상 파일이 존재하지 않을 때 발생합니다.
    #[error("Vault target does not exist: {0}")]
    TargetNotFound(String),

    /// 상위 폴더 계층이 존재하지 않아 파일을 생성할 수 없을 때 발생합니다.
    #[error("Vault parent does not exist: {0}")]
    ParentNotFound(String),

    /// 폴더나 파일 계층에 심볼릭 링크가 포착되어 보안 정책 상 작동을 중단시킬 때 발생합니다.
    #[error("symlink descendants are not allowed: {0}")]
    SymlinkNotAllowed(String),

    /// 폴더가 아니어서 하위 검색에 들어갈 수 없는 요소가 경로 중간에 있을 때 발생합니다.
    #[error("path segment is not a directory: {0}")]
    NonDirectoryAncestor(String),

    /// 최종 확인된 물리 파일 위치가 저장소 격리구역 외부임이 파악되었을 때 발생합니다. (보안 가드 작동)
    #[error("resolved path is outside the Vault: {0}")]
    OutsideVault(PathBuf),

    /// 그 외 운영체제 내부 입출력 호출이 실패했을 때의 에러입니다.
    #[error("filesystem operation failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Vault의 보안 격리 가드와 경로 조회 어댑터가 완벽하게 설계 사양대로 디스크에서 오동작을 잡는지 테스트합니다.
#[cfg(test)]
mod tests {
    use std::{fs, io, path::Path};

    // Unix/Linux 환경인 경우에만 심볼릭 링크 생성 API 및 권한 모드 설정을 활성화하여 테스트에 사용합니다.
    #[cfg(unix)]
    use std::os::unix::fs::{PermissionsExt, symlink};
    use tempfile::TempDir;

    use super::{VaultError, VaultRoot, map_root_error};
    use crate::domain::path::CanonicalPath;

    /// 테스트용 헬퍼 함수로 문자열을 도메인용 `CanonicalPath` 타입으로 즉석 변환합니다.
    fn path(value: &str) -> CanonicalPath {
        CanonicalPath::parse(value).expect("test path must be valid")
    }

    /// 디스크 상의 실제 디렉터리를 껍데기가 아닌 온전한 저장소 루트로 정상 활성화하는지 테스트합니다.
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

    /// 존재하지 않는 가상의 폴더나 일반 파일을 주입했을 때 각각 알맞은 유형의 오류로 인계하는지 테스트합니다.
    #[test]
    fn distinguishes_missing_file_and_unavailable_roots() {
        // 존재하지 않는 가짜 경로 지정 시: RootNotFound 감지 검증
        let directory = TempDir::new().expect("temporary directory should be created");
        let missing = directory.path().join("missing");
        assert!(matches!(
            VaultRoot::open(&missing),
            Err(VaultError::RootNotFound(path)) if path == missing
        ));

        // 폴더가 아닌 파일 지정 시: RootNotDirectory 감지 검증
        let file = directory.path().join("note.md");
        fs::write(&file, "content").expect("test file should be written");
        assert!(matches!(
            VaultRoot::open(&file),
            Err(VaultError::RootNotDirectory(path)) if path == file
        ));

        // 접근 불가능 등 권한 이슈 발생 시: RootUnavailable 감지 검증
        let permission_error = io::Error::from(io::ErrorKind::PermissionDenied);
        assert!(matches!(
            map_root_error(directory.path().to_path_buf(), permission_error),
            VaultError::RootUnavailable { .. }
        ));
    }

    /// [Unix 전용] 일반 유저가 들어갈 수 없도록 읽기 권한을 완전히 0으로 소거한 제한 폴더를
    /// 저장소로 여는 시도를 안전하게 거절하는지 검증합니다.
    #[cfg(unix)]
    #[test]
    fn rejects_an_inaccessible_root_for_regular_users() {
        let directory = TempDir::new().expect("temporary directory should be created");
        let restricted = directory.path().join("restricted");
        fs::create_dir(&restricted).expect("restricted directory should be created");
        // 권한 코드를 0o000으로 밀어서 아무도 읽지 못하게 격리합니다.
        fs::set_permissions(&restricted, fs::Permissions::from_mode(0o000))
            .expect("permissions should be changed");

        let result = VaultRoot::open(&restricted);

        // 테스트 완료 후 타작업에 방해되지 않게 임시 디렉터리 권한을 복구합니다.
        fs::set_permissions(&restricted, fs::Permissions::from_mode(0o700))
            .expect("permissions should be restored");

        // 만약 root(관리자) 권한으로 테스트가 돌고 있다면 000 권한 세팅도 우회 통과될 수 있으므로 통과 판별을 생략합니다.
        if result.is_ok() {
            return;
        }
        assert!(matches!(result, Err(VaultError::RootUnavailable { .. })));
    }

    /// [Unix 전용] Vault 루트 설정 경로명 그 자체는 실제 저장소를 가리키는 외부 심볼릭 링크여도
    /// 기동 시점에 실주소로 Canonicalize하여 정상 개방해 주는지 검증합니다.
    #[cfg(unix)]
    #[test]
    fn allows_the_configured_root_itself_to_be_a_symlink() {
        let directory = TempDir::new().expect("temporary directory should be created");
        let actual = directory.path().join("actual-vault");
        let configured = directory.path().join("selected-vault");
        fs::create_dir(&actual).expect("actual Vault should be created");
        // 설정 위치에 심볼릭 링크 파일을 심습니다.
        symlink(&actual, &configured).expect("root symlink should be created");

        let vault = VaultRoot::open(&configured).expect("root symlink should be accepted");

        assert_eq!(vault.configured_path(), configured);
        assert_eq!(
            vault.canonical_path(),
            fs::canonicalize(actual).expect("actual Vault should canonicalize")
        );
    }

    /// 디렉터리 아래의 다층적 경로 탐색 및 파일 매핑 작업이 올바르게 해결(resolve)되는지 검사합니다.
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

    /// [Unix 전용] Vault 영역 내부에 교묘하게 기재된 링크 파일이 내부나 외부의 민감 파일을
    /// 링크 가리키기를 시도하는 위협을 탐지하여 에러(`SymlinkNotAllowed`)로 잡아내는지 테스트합니다.
    #[cfg(unix)]
    #[test]
    fn rejects_descendant_symlinks_to_inside_and_outside_the_vault() {
        let directory = TempDir::new().expect("temporary directory should be created");
        let outside = TempDir::new().expect("outside directory should be created");
        let real = directory.path().join("real.md");

        fs::write(&real, "inside").expect("inside file should be written");
        fs::write(outside.path().join("outside.md"), "outside")
            .expect("outside file should be written");

        // Vault 내의 진짜 파일로 향하는 내부 링크 생성
        symlink(&real, directory.path().join("inside-link.md"))
            .expect("inside symlink should be created");
        // Vault 바깥 임시 저장소로 향하는 외부 링크 생성
        symlink(
            outside.path().join("outside.md"),
            directory.path().join("outside-link.md"),
        )
        .expect("outside symlink should be created");

        let vault = VaultRoot::open(directory.path()).expect("Vault should open");

        // 내부든 외부든, 자식 계층의 어떠한 심볼릭 링크 접근도 격리를 해치므로 철저하게 차단해야 합니다.
        assert!(matches!(
            vault.resolve_existing(&path("inside-link.md")),
            Err(VaultError::SymlinkNotAllowed(value)) if value == "inside-link.md"
        ));
        assert!(matches!(
            vault.resolve_existing(&path("outside-link.md")),
            Err(VaultError::SymlinkNotAllowed(value)) if value == "outside-link.md"
        ));
    }

    /// [Unix 전용] 중간에 위치한 디렉터리 그 자체가 다른 곳을 바라보는 링크 폴더로 변장하여
    /// 격리를 위반하려 할 때, 파일 읽기와 파일 생성 과정 모두에서 조기에 색출하는지 검증합니다.
    #[cfg(unix)]
    #[test]
    fn rejects_intermediate_and_create_parent_symlinks() {
        let directory = TempDir::new().expect("temporary directory should be created");
        let actual = directory.path().join("actual");
        fs::create_dir(&actual).expect("actual directory should be created");
        fs::write(actual.join("note.md"), "content").expect("test file should be written");
        // 중간 링크 디렉터리를 생성합니다.
        symlink(&actual, directory.path().join("linked"))
            .expect("directory symlink should be created");

        let vault = VaultRoot::open(directory.path()).expect("Vault should open");

        // 1. 기존 파일 조회 시도 차단 확인
        assert!(matches!(
            vault.resolve_existing(&path("linked/note.md")),
            Err(VaultError::SymlinkNotAllowed(value)) if value == "linked"
        ));
        // 2. 신규 파일 생성 검증 시도 차단 확인
        assert!(matches!(
            vault.resolve_parent_for_create(&path("linked/new.md")),
            Err(VaultError::SymlinkNotAllowed(value)) if value == "linked"
        ));
    }

    /// 없는 대상을 읽으려 하거나 존재하지 않는 상위 디렉터리 경로 내에 신규 파일을 쓰려고 시도할 때
    /// 적합한 에러 코드로 분리해 알려주는지 테스트합니다.
    #[test]
    fn distinguishes_missing_read_targets_and_create_parents() {
        let directory = TempDir::new().expect("temporary directory should be created");
        let vault = VaultRoot::open(directory.path()).expect("Vault should open");

        // 1. 없는 파일 조회 -> TargetNotFound 리턴
        assert!(matches!(
            vault.resolve_existing(&path("missing.md")),
            Err(VaultError::TargetNotFound(value)) if value == "missing.md"
        ));
        // 2. 없는 부모에 파일 생성 -> ParentNotFound 리턴
        assert!(matches!(
            vault.resolve_parent_for_create(&path("missing/new.md")),
            Err(VaultError::ParentNotFound(value)) if value == "missing"
        ));
    }

    /// [Unix 전용] 새로 쓰려고 설계한 목적지 대상과 동일한 명칭의 심볼릭 링크 파일이 이미 생성되어 있을 때,
    /// 이를 덮어쓰거나 링크를 타지 않게 생성 검증 단계(`resolve_parent_for_create`)에서 차단하는지 테스트합니다.
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
