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

/// 임시 파일명 충돌을 방지하기 위한 전역 멀티스레드 안전 카운터입니다.
///
/// ## 원자적(Atomic) 연산과 `AtomicU64`
/// 비동기/멀티스레드 환경에서 여러 작업이 동시에 임시 파일을 만들려고 할 때, 단순 정수(`u64`) 카운터를 사용하면
/// 동일한 숫자가 할당되는 경쟁 상태(Race Condition)가 발생할 수 있습니다.
/// `AtomicU64`는 별도 뮤텍스 없이 값을 원자적으로 증가시켜 프로세스 내부 요청에 서로 다른 순번을 제공합니다.
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// 단일 활성 저장소(Vault) 내에서 마크다운 파일을 안전하고 배타적으로 작성/생성 및 수정하는 어댑터입니다.
///
/// ## 동시성 제어 및 원자성 보장 설계
/// * **Mutex 잠금 (`write_lock`)**: 프로세스 내의 여러 비동기 스레드가 동시에 쓰기 작업에 진입하는 것을 1차적으로 조율합니다.
/// * **TOCTOU 방어**: `OpenOptions::create_new(true)`를 통해 파일의 존재 여부 판단과 생성 작업을 단일 시스템 콜로 묶어 처리합니다.
/// * **원자적 파일 교체 (Atomic Replace)**: 파일 수정 시 임시 파일에 먼저 기록하고 같은 filesystem의 `rename`으로 교체해, 독자가 중간 내용의 파일을 보지 않도록 합니다.
#[derive(Clone, Debug)]
pub struct MarkdownWriter {
    /// 격리 폴더 경계 및 심볼릭 링크 보안 규칙을 관리하는 Vault 루트 객체
    vault: VaultRoot,
    /// 허용 가능한 단일 마크다운 문서의 최대 크기 제한 (메모리 고갈 및 디바이스 가득 참 방지)
    max_bytes: u64,
    /// 프로세스 내부에서 쓰기 동작을 원자적으로 직렬화하기 위해 공유하는 뮤텍스
    write_lock: Arc<Mutex<()>>,
}

impl MarkdownWriter {
    /// 새로운 `MarkdownWriter` 인스턴스를 초기화합니다.
    #[must_use]
    pub fn new(vault: VaultRoot, max_bytes: u64) -> Self {
        Self {
            vault,
            max_bytes,
            // 뮤텍스는 복제가 가능하도록 참조 카운팅 스마트 포인터 `Arc`로 감쌉니다.
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    /// 파일 시스템에 마크다운 문서를 새로 생성하고 디바이스 동기화가 완수된 결과 스냅샷을 반환합니다.
    ///
    /// ## 특징
    /// 동명의 파일이 이미 존재하면 생성에 실패하며 덮어쓰지 않습니다.
    ///
    /// # Errors
    ///
    /// 부모 폴더가 누락됐거나 디렉터리가 아닌 경우, 파일이 이미 실존하는 경우, 내용물 크기가 한계를 초과한 경우,
    /// 혹은 물리 I/O 작업 중 실패가 발생하면 [`MarkdownWriteError`]를 던집니다.
    pub fn create(
        &self,
        path: &MarkdownPath,
        content: String,
    ) -> Result<MarkdownDocument, MarkdownWriteError> {
        self.create_with(path, content, persist_content)
    }

    /// 파일 생성의 세부 구현 논리식입니다. 주입받은 `persist` 클로저를 사용해 실제 쓰기를 처리합니다.
    fn create_with(
        &self,
        path: &MarkdownPath,
        content: String,
        persist: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
    ) -> Result<MarkdownDocument, MarkdownWriteError> {
        // 1. 프로세스 단위 쓰기 잠금을 획득하여 경쟁을 사전 조율합니다.
        //    뮤텍스가 다른 스레드의 패닉으로 인해 깨졌다면 `LockPoisoned` 에러를 반환합니다.
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| MarkdownWriteError::LockPoisoned)?;

        // 2. 전달된 데이터 바이트 크기가 한계선을 준수하는지 1차로 체크합니다.
        let size = u64::try_from(content.len()).unwrap_or(u64::MAX);
        if size > self.max_bytes {
            return Err(MarkdownWriteError::FileTooLarge {
                path: path.to_string(),
                observed: size,
                maximum: self.max_bytes,
            });
        }

        // 3. Vault 보안 격리 검증을 거쳐 새로 생성될 파일의 절대 경로를 해석해 냅니다.
        let absolute = self.vault.resolve_parent_for_create(path.as_canonical())?;

        // 4. `create_new(true)` 옵션으로 파일을 오픈합니다.
        //    이 옵션은 파일이 존재하지 않는 경우에만 원자적으로 새 파일을 생성합니다.
        //    (존재 여부 확인과 생성을 하나의 OS 호출로 처리해 동일 target 생성 경쟁을 방어함)
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&absolute)
            .map_err(|source| map_create_error(path, source))?;

        // 5. 디바이스에 데이터 쓰기 및 디스크 물리 동기화(fsync)를 지시합니다.
        if let Err(source) = persist(&mut file, content.as_bytes()) {
            // 쓰기에 실패하면 찌꺼기 불완전 파일이 남지 않도록 제거 헬퍼를 실행합니다.
            drop(file);
            remove_incomplete_file(&absolute, path);
            return Err(MarkdownWriteError::Io {
                path: path.to_string(),
                source,
            });
        }

        // 6. 생성 완료 직후 메타데이터를 조회하여 크기와 최종 수정시각 정보를 가져옵니다.
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

        // 7. 문서 검증 해시값을 획득하고 최종 성공 도메인 엔티티를 빌드해 리턴합니다.
        let hash = format!("sha256:{}", hex::encode(Sha256::digest(content.as_bytes())));
        Ok(MarkdownDocument {
            path: path.clone(),
            content,
            hash,
            size: metadata.len(),
            modified_at,
        })
    }

    /// 파일 내용이 수정되지 않았음을 `base_hash`를 통해 확인하고, 동일할 때만 원자적으로 내용을 업데이트합니다.
    ///
    /// ## 수정 시 낙관적 락(Optimistic Concurrency Control, OCC) 적용
    /// 데이터베이스뿐 아니라 파일 시스템에서도 다수의 편집자가 동시 수정을 시도할 때 정합성이 깨질 수 있습니다.
    /// 본 함수는 클라이언트가 읽어간 시점의 해시값(`base_hash`)과 디스크 내 현재 파일 해시가
    /// 정확히 일치할 때만 수정을 승인함으로써 타인이 작성한 내용을 인지 없이 덮어써 버리는 사고를 예방합니다.
    ///
    /// # Errors
    ///
    /// 파일이 존재하지 않는 경우, 대상이 일반 파일이 아닌 경우, 용량이 제한을 초과한 경우,
    /// 해시가 어긋나는 경우(낙관적 락 충돌), 파일 입출력 오류 발생 시 [`MarkdownUpdateError`]를 반환합니다.
    pub fn update(
        &self,
        path: &MarkdownPath,
        content: String,
        base_hash: &str,
    ) -> Result<MarkdownDocument, MarkdownUpdateError> {
        self.update_with(path, content, base_hash, persist_content)
    }

    /// 파일 수정의 세부 구현 논리식입니다.
    fn update_with(
        &self,
        path: &MarkdownPath,
        content: String,
        base_hash: &str,
        persist: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
    ) -> Result<MarkdownDocument, MarkdownUpdateError> {
        // 1. 데이터 용량 규격 초과 여부를 먼저 체크합니다.
        let size = u64::try_from(content.len()).unwrap_or(u64::MAX);
        if size > self.max_bytes {
            return Err(MarkdownUpdateError::FileTooLarge {
                path: path.to_string(),
                observed: size,
                maximum: self.max_bytes,
            });
        }

        // 2. 프로세스 전역 쓰기 락을 획득하여 동시 진입 경쟁을 조율합니다.
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| MarkdownUpdateError::LockPoisoned)?;

        // 3. 파일 판독기(Reader)를 생성하여 디스크 상의 기존 파일 상태와 해시를 조회합니다.
        let reader = MarkdownReader::new(self.vault.clone(), self.max_bytes);
        let current = reader.read(path).map_err(MarkdownUpdateError::from_read)?;

        // 4. 기존 파일의 해시 정보와 클라이언트가 명시한 `base_hash`를 대조하여 락 일치성을 검사합니다.
        enforce_base_hash(path, base_hash, &current.hash)?;

        // 5. 업데이트 목표 대상 파일의 물리 경로를 도출해 냅니다.
        let target = self.vault.resolve_existing(path.as_canonical())?;

        // 6. 동일 폴더 내에 안전한 임시 난수 경로(`.knowledgeos-*.tmp`)를 생성합니다.
        //    동일한 파일시스템 안에 임시 파일을 만들어야 `fs::rename`이 원자적으로(Atomic) 오차 없이 동작합니다.
        let temporary = temporary_path(&target)?;
        let mut temporary_file = OpenOptions::new()
            .write(true)
            .create_new(true) // 임시 파일 자체도 단독 배타 생성
            .open(&temporary)
            .map_err(|source| MarkdownUpdateError::Io {
                path: path.to_string(),
                source,
            })?;

        // 7. 임시 파일에 수정한 내용을 기입하고 디스크 동기화를 수행합니다.
        if let Err(source) = persist(&mut temporary_file, content.as_bytes()) {
            drop(temporary_file);
            remove_temporary_file(&temporary, path);
            return Err(MarkdownUpdateError::Io {
                path: path.to_string(),
                source,
            });
        }

        // 8. [재검증 단계] 임시 파일을 쓰는 동안 다른 스레드나 프로세스가 원본 파일을 변경했는지 다시 한 번 꼼꼼히 확인합니다.
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

        // 9. 정상 수치 기록을 위해 임시 파일의 메타데이터를 확보해 둡니다.
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

        // 10. `fs::rename` 호출을 진행합니다.
        //     동일 파일시스템 상에서의 파일 이름 변경은 POSIX 표준에 의해 원자적(Atomic Replace)으로 실행됩니다.
        //     즉, 정상 동작 중 다른 reader는 기존 파일 또는 완성된 신규 파일만 관찰합니다.
        //     전원 장애 후 rename 자체의 영속성까지 보장하려면 parent directory fsync가 추가로 필요합니다.
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

/// 파일 데이터를 기록하고 사용자 공간 버퍼를 flush한 후, OS에 파일 데이터와 metadata 동기화(`fsync`)를 요청하는 내부 헬퍼 함수입니다.
///
/// ## OS 페이지 캐시와 `sync_all` (fsync)
/// 단순히 `write`만 호출하면 운영체제의 메모리 페이지 캐시(Page Cache)에만 데이터가 적재되어 전원 공급 중단 시 데이터 유실이 일어납니다.
/// `flush()`는 Rust writer의 사용자 공간 버퍼를 비우고, `sync_all()`은 OS와 filesystem에
/// 파일 데이터 및 metadata 동기화를 요청합니다. 실제 전원 장애 내구성은 filesystem과 저장장치 정책에도 의존합니다.
fn persist_content(file: &mut File, content: &[u8]) -> io::Result<()> {
    file.write_all(content)?;
    file.flush()?;
    file.sync_all()
}

/// 파일 신규 작성 오류 발생 시, 중복 생성 오류와 일반 입출력 에러를 가독성 높은 이넘 형태로 분기 변환합니다.
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

/// 생성 과정 중 비정상 실패가 난 경우 디바이스 내 찌꺼기 파일을 강제 삭제(Clean up)하여 저장소 위생을 관리합니다.
fn remove_incomplete_file(absolute: &Path, path: &MarkdownPath) {
    if let Err(cleanup_error) = fs::remove_file(absolute) {
        tracing::error!(
            relative_path = %path,
            %cleanup_error,
            "failed to remove incomplete Markdown file"
        );
    }
}

/// 제공받은 기대값 해시(`expected`)와 파일의 실제 최신 해시(`actual`)가 완벽히 매칭되는지 단언하고, 다르면 충돌 예외를 반환합니다.
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

/// 원자적 교체(Atomic Replace)에 사용할 프로세스 내부 고유 임시 파일 경로를 생성합니다.
///
/// ## 명명 규칙
/// `[부모디렉토리]/.knowledgeos-[프로세스ID]-[전역스레드안전시퀀스].tmp`
/// 프로세스 ID와 원자적 증가 순번을 결합해 현재 프로세스 내부의 파일명 경합을 피합니다.
/// 보안용 난수 이름은 아니며 최종 생성 시 `create_new(true)`가 기존 경로 덮어쓰기를 방지합니다.
fn temporary_path(target: &Path) -> Result<std::path::PathBuf, MarkdownUpdateError> {
    let parent = target.parent().ok_or(MarkdownUpdateError::InvalidTarget)?;
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(
        ".knowledgeos-{}-{sequence}.tmp",
        std::process::id()
    )))
}

/// 생성된 교체용 임시 파일을 안전하게 물리 디스크상에서 삭제 소거합니다.
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

/// 마크다운 신규 생성 및 디스크 영속화 라이프사이클 상에서 발생하는 예외 상태 정의입니다.
#[derive(Debug, Error)]
pub enum MarkdownWriteError {
    /// Vault 영역 경계 격리 정책 및 보안 위반 시 발생하는 에러
    #[error(transparent)]
    Vault(#[from] VaultError),

    /// 동일한 경로의 파일이 이미 존재하는 상태를 감지한 오류
    #[error("Markdown file already exists: {0}")]
    AlreadyExists(String),

    /// 허용 한도 바이트 크기 이상으로 대용량 파일을 쓰려고 한 오류
    #[error("Markdown file is too large: {observed} bytes; maximum is {maximum} bytes: {path}")]
    FileTooLarge {
        path: String,
        observed: u64,
        maximum: u64,
    },

    /// 시스템 입출력 시스템 콜 실패 오류
    #[error("filesystem operation failed for {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },

    /// 시간 정보나 접근 권한 등 메타데이터 정보를 획득하는 데 실패할 때의 에러
    #[error("file metadata is unavailable for {path}: {source}")]
    Metadata {
        path: String,
        #[source]
        source: io::Error,
    },

    /// 비동기 전역 쓰기 락 뮤텍스가 타 스레드 패닉으로 오염(Poisoned)된 오류
    #[error("Markdown write lock is poisoned")]
    LockPoisoned,
}

/// 마크다운 낙관적 수정 및 디스크 원자적 교체 라이프사이클 상에서 발생하는 예외 상태 정의입니다.
#[derive(Debug, Error)]
pub enum MarkdownUpdateError {
    /// Vault 보안 규칙 및 접근 검증 실패 에러
    #[error(transparent)]
    Vault(#[from] VaultError),

    /// 수정 대상이 디스크에 실존하지 않는 오류
    #[error("Markdown file does not exist: {0}")]
    NotFound(String),

    /// 수정 대상이 일반 파일이 아니고 디렉터리 등 특수 장치인 오류
    #[error("path is not a regular file: {0}")]
    NotRegularFile(String),

    /// 수정 내용이 제한 바이트 용량을 상회하는 오류
    #[error("Markdown file is too large: {observed} bytes; maximum is {maximum} bytes: {path}")]
    FileTooLarge {
        path: String,
        observed: u64,
        maximum: u64,
    },

    /// 파일 인코딩이 정상 유니코드 규격인 UTF-8이 아닌 오류
    #[error("Markdown file is not valid UTF-8: {0}")]
    InvalidUtf8(String),

    /// 파일 판독 과정에서 다수 사용자의 수정 경합으로 정밀 스냅샷을 획득하지 못한 충돌 오류
    #[error("Markdown file changed repeatedly while being read")]
    ReadConflict,

    /// 클라이언트가 제시한 수정 시점 해시(expected)가 최신 디스크 해시(actual)와 상이할 때의 낙관적 락 충돌 오류
    #[error("Markdown hash conflict for {path}: expected {expected}, actual {actual}")]
    HashConflict {
        path: String,
        expected: String,
        actual: String,
    },

    /// 시스템 I/O 실행 실패 오류
    #[error("filesystem operation failed for {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },

    /// 메타데이터 정보 리딩 실패 오류
    #[error("file metadata is unavailable for {path}: {source}")]
    Metadata {
        path: String,
        #[source]
        source: io::Error,
    },

    /// 뮤텍스 오염(Poisoned) 감지 오류
    #[error("Markdown write lock is poisoned")]
    LockPoisoned,

    /// 수정하고자 하는 대상 파일의 상위 부모 폴더 노드가 부재하는 예외
    #[error("Markdown target has no parent directory")]
    InvalidTarget,
}

impl MarkdownUpdateError {
    /// 판독기 에러(`MarkdownReadError`)를 수정 과정 오류 타입(`MarkdownUpdateError`)으로 상세 분기 매핑해주는 유연한 변환 헬퍼입니다.
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

/// `MarkdownWriter`의 생성, 수정, 영속성 및 동시성 예외 복원 메커니즘을 상세히 단언 검증하는 단위 테스트 모듈입니다.
#[cfg(test)]
mod tests {
    use std::{fs, io, sync::Arc, thread};

    use tempfile::TempDir;

    use sha2::{Digest, Sha256};

    use super::{MarkdownWriteError, MarkdownWriter};
    use crate::{domain::path::MarkdownPath, infrastructure::vault::VaultRoot};

    /// 문자열 경로를 파싱하여 도메인용 경로 타입인 `MarkdownPath`로 바로 얻어내는 테스트 헬퍼 함수입니다.
    fn markdown_path(value: &str) -> MarkdownPath {
        MarkdownPath::parse(value).expect("test Markdown path must be valid")
    }

    /// 파일 무결성 해시값 매칭을 검사하기 위해 텍스트 콘텐츠의 SHA-256 해시를 도출해내는 테스트용 계산식입니다.
    fn content_hash(content: &str) -> String {
        format!("sha256:{}", hex::encode(Sha256::digest(content.as_bytes())))
    }

    /// 격리 폴더 내에 한글명이 섞인 마크다운 파일 생성을 요청했을 때, 정상적으로 파일이 써지고 올바른 메타데이터 구조를 뿜어내는지 확인합니다.
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

    /// 내용이 아예 없는 빈 마크다운 생성이나 최대 용량 한계선에 딱 맞물리는 파일 쓰기는 수용하되,
    /// 1바이트라도 크기 상한을 넘으면 디스크 영역 오염 없이 즉시 거부하는지 경계선 검사를 진행합니다.
    #[test]
    fn accepts_empty_and_exact_limit_but_rejects_larger_content() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        let writer = MarkdownWriter::new(
            VaultRoot::open(directory.path()).expect("Vault should open"),
            8, // 8바이트 최대 제한 적용
        );

        // A. 빈 파일(0바이트) -> 생성되어야 함
        writer
            .create(&markdown_path("empty.md"), String::new())
            .expect("empty Markdown should be created");

        // B. 8바이트 파일 -> 생성되어야 함
        writer
            .create(&markdown_path("exact.md"), "12345678".to_owned())
            .expect("exact limit Markdown should be created");

        // C. 9바이트 파일 -> 용량 초과로 무조건 거부되어야 함
        assert!(matches!(
            writer.create(&markdown_path("large.md"), "123456789".to_owned()),
            Err(MarkdownWriteError::FileTooLarge { .. })
        ));

        // 거부 확인: 디스크에 찌꺼기 파일이 남아있지 않는지 확인 단언
        assert!(!directory.path().join("large.md").exists());
    }

    /// 새로운 파일을 만드는 `create` 명령 실행 시, 이미 물리 파일이 해당 위치에 존재하는 상태라면
    /// 덮어쓰거나 수정하지 않고 "이미 존재함" 에러로 명확히 밀어내어 기존 데이터를 보존하는지 단언합니다.
    #[test]
    fn never_overwrites_an_existing_target() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        fs::write(directory.path().join("existing.md"), "original")
            .expect("existing Markdown should be written");
        let writer = MarkdownWriter::new(
            VaultRoot::open(directory.path()).expect("Vault should open"),
            1024,
        );

        // 기존 파일에 create를 다시 찔렀을 때의 거부 반응 테스트
        assert!(matches!(
            writer.create(&markdown_path("existing.md"), "replacement".to_owned()),
            Err(MarkdownWriteError::AlreadyExists(path)) if path == "existing.md"
        ));

        // 기존 원본의 훼손 유무를 단언 검증
        assert_eq!(
            fs::read_to_string(directory.path().join("existing.md"))
                .expect("original Markdown should remain readable"),
            "original"
        );
    }

    /// 동일한 경로에 두 개의 스레드가 동시에 `create`를 통해 신규 파일 작성을 격렬히 시도하더라도,
    /// 동시성 가드 및 락 장치가 발동해 오직 하나의 요청만 성공하고 하나는 중복 에러로 탈락되는지 확인합니다.
    #[test]
    fn allows_only_one_concurrent_create_for_the_same_path() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        // 두 스레드가 writer 소유권을 나누어 가질 수 있게 Arc로 래핑합니다.
        let writer = Arc::new(MarkdownWriter::new(
            VaultRoot::open(directory.path()).expect("Vault should open"),
            1024,
        ));
        let mut handles = Vec::new();

        // 두 가닥의 독립 스레드를 띄워 동일 경로("race.md")에 상이한 데이터를 들이붓게 기동합니다.
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

        // 결과 분석: 무조건 1개만 Ok이고, 나머지 1개는 AlreadyExists 에러여야 함
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(MarkdownWriteError::AlreadyExists(_))))
                .count(),
            1
        );
    }

    /// 파일 write·flush·sync 과정에서 오류가 났을 때,
    /// 찌꺼기 incomplete 임시 구조물이 디렉터리에 잔존하지 않게 자동 뒤처리가 실행되는지 검증합니다.
    #[test]
    fn removes_an_incomplete_file_when_persistence_fails() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        let writer = MarkdownWriter::new(
            VaultRoot::open(directory.path()).expect("Vault should open"),
            1024,
        );
        let path = markdown_path("incomplete.md");

        // 인위적인 쓰기 실패 시나리오 주입:
        // 내용을 3바이트만 임시 기록하다가 강제로 I/O 에러를 리턴시킵니다.
        let result = writer.create_with(&path, "content".to_owned(), |file, content| {
            use std::io::Write;

            file.write_all(&content[..3])?;
            Err(io::Error::other("injected persistence failure"))
        });

        // 입출력 에러를 잘 반환했는지 검사하고 파일이 최종 소거되었는지 검증
        assert!(matches!(result, Err(MarkdownWriteError::Io { .. })));
        assert!(!directory.path().join("incomplete.md").exists());
    }

    /// 파일 수정 요청 시 제공한 `base_hash`가 원본 파일 해시와 정직하게 부합할 때,
    /// 디렉터리 내 타겟이 안정적으로 수정 및 교체 완료되는지 기본 동시성 락 수용 여부를 테스트합니다.
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
                &content_hash("original"), // 원본 해시 제공
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

    /// 업데이트 요청에 실어 보낸 이전 해시값이 디바이스 내 해시와 불일치하거나(락 충돌),
    /// 임시 파일에 수정 데이터 기입 중 입출력 물리 장애가 나면,
    /// 원본 데이터를 파괴하거나 교체하지 않고 깨끗하게 지켜내는지 안전 복원성을 테스트합니다.
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

        // Case A. 어긋난 엉뚱한 해시 정보 전송 시도 -> HashConflict로 거부 확인
        assert!(matches!(
            writer.update(&path, "stale".to_owned(), &content_hash("older")),
            Err(super::MarkdownUpdateError::HashConflict { .. })
        ));

        // Case B. 올바른 해시를 주었으나 데이터 기록 도중 영속화 에러가 난 시나리오
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

        // 결과 검증: 두 실패 상황을 밟았음에도 원본 파일은 오염되지 않고 "original"로 잘 생존했는가?
        assert!(matches!(failed, Err(super::MarkdownUpdateError::Io { .. })));
        assert_eq!(
            fs::read_to_string(directory.path().join("note.md"))
                .expect("original Markdown should remain readable"),
            "original"
        );

        // Vault 내부에 지저분한 임시 파일이 정리되지 않고 축적되어 방치되지 않았는지 파일 개수를 확인 단언
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("Vault should be readable")
                .count(),
            1
        );
    }

    /// 동일 원본 해시(`base_hash`)를 기준으로 두 명의 수정 요청자가 비동기 스레드로 덤벼들어 동시에 덮어쓰기를 꾀할 때,
    /// 오직 한 명만 200 OK 수정을 완수하고, 다른 하나는 락 충돌(`HashConflict`)을 먹고 튕겨 나가는지 트랜잭션의 정합성을 단언합니다.
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

        // 2개의 스레드가 "note.md"에 동일한 base_hash를 쥐고 비동기로 쓰기를 경합시킵니다.
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

        // 정합성 검증: 무조건 1개만 수정 성공, 1개는 HashConflict 에러여야 함
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
