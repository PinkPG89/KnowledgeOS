use std::{
    fs::{File, Metadata},
    io::{self, Read},
};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::{document::MarkdownDocument, path::MarkdownPath};

use super::vault::{VaultError, VaultRoot};

/// 단일 활성 저장소(Vault) 내에서 파일 내용의 일관성을 검증하며
/// 안정적인 UTF-8 Markdown 파일 스냅샷을 읽어내는 판독기(Reader)입니다.
#[derive(Clone, Debug)]
pub struct MarkdownReader {
    /// 검증용 파일 시스템 보안 어댑터
    vault: VaultRoot,
    /// 서버가 허용하는 단일 파일의 최대 바이트 수 제한 (메모리 고갈 방어)
    max_bytes: u64,
}

impl MarkdownReader {
    /// 새로운 `MarkdownReader` 인스턴스를 상수 생성자(`const fn`)를 통해 신속하게 생성합니다.
    #[must_use]
    pub const fn new(vault: VaultRoot, max_bytes: u64) -> Self {
        Self { vault, max_bytes }
    }

    /// 파일 시스템으로부터 마크다운 문서를 읽어옵니다.
    /// 읽어오는 짧은 찰나에 다른 프로세스나 편집기가 파일 내용을 변경하면 자동으로 한 번 재시도합니다.
    ///
    /// ## 동시 변경 감지 및 우아한 복구 (Concurrency Guard)
    /// 웹 백엔드 서버가 디스크에서 파일을 순차적으로 읽는 도중 외부 요인(예: Git 동기화, 사용자 편집기 저장)으로
    /// 파일 내용이 바뀌면 꼬인 데이터가 유입될 수 있습니다. (TOCTOU 취약점 방지)
    /// 이 함수는 감지 시 자동 1회 재시도 흐름을 제공하여 데이터 일관성을 보증합니다.
    ///
    /// # Errors
    ///
    /// 대상이 폴더이거나, 일반 파일이 아니거나, 크기가 제한 크기(`max_bytes`)를 초과하거나,
    /// 유효한 UTF-8 텍스트가 아니거나, 읽는 과정에서 연속 2회 이상 외부 수정 충돌이 발생하면
    /// [`MarkdownReadError`]를 반환합니다.
    pub fn read(&self, path: &MarkdownPath) -> Result<MarkdownDocument, MarkdownReadError> {
        // 단 1회의 재시도 기회를 보장하는 헬퍼 매커니즘(`with_single_retry`)을 경유하여 한 번씩 읽습니다.
        with_single_retry(|| self.read_once(path))
    }

    /// 파일 시스템에 실제로 1회 접근하여 검증을 동반한 읽기 작업을 시도합니다.
    fn read_once(&self, path: &MarkdownPath) -> Result<MarkdownDocument, ReadAttemptError> {
        // 1. Vault 격리 규칙에 맞춰 파일의 물리 절대 경로를 도출해 냅니다.
        let absolute = self
            .vault
            .resolve_existing(path.as_canonical())
            .map_err(MarkdownReadError::Vault)?;

        // 2. 파일을 오픈합니다.
        let mut file =
            File::open(&absolute).map_err(|source| map_io_error(path.as_str(), source))?;

        // 3. 읽기 시작 시점의 파일 메타데이터(수정일자, 용량 등)를 기록합니다. (TOCTOU 감지용)
        let before = file
            .metadata()
            .map_err(|source| map_io_error(path.as_str(), source))?;

        // 4. 대상 노드가 정상적인 정규 파일(regular file)인지 확인합니다. (디렉터리나 파이프 장치 등 배제)
        if !before.file_type().is_file() {
            return Err(MarkdownReadError::NotRegularFile(path.to_string()).into());
        }

        // 5. 파일 디바이스 레벨의 사전 용량이 허용한계보다 큰지 먼저 검사해봅니다.
        enforce_size(path, before.len(), self.max_bytes)?;

        // 6. 파일에서 데이터를 안전하게 읽습니다.
        //    최대 바이트 크기 제한에 안전 연산(saturating_add)을 추가하여 u64 최댓값을 넘어 오버플로우가 나는 것을 막고,
        //    한계치를 1바이트 더 초과하여 읽으려 시도(take)함으로써 실제 크기가 용량 제한을 한 발짝 넘는지를 체크합니다.
        let mut bytes = Vec::new();
        file.by_ref()
            .take(self.max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|source| map_io_error(path.as_str(), source))?;

        // 7. 실제로 읽어 들인 물리 바이트 크기가 한도를 초과했는지 최종 검사합니다.
        enforce_size(path, bytes.len() as u64, self.max_bytes)?;

        // 8. 읽기를 완결한 직후에 다시 한번 파일 메타데이터를 가져옵니다.
        let after = file
            .metadata()
            .map_err(|source| map_io_error(path.as_str(), source))?;

        // 9. [비교 단계] 읽기 시작하기 전과 읽고 난 후의 파일 상태가 조금이라도 변경되었거나,
        //    읽은 내용의 총량이 메타데이터상 용량과 매칭되지 않으면 파일이 도중에 깨진 것이므로
        //    동시 변경 예외(`ReadAttemptError::Changed`)를 던집니다.
        if metadata_changed(&before, &after, bytes.len() as u64)? {
            return Err(ReadAttemptError::Changed);
        }

        // 10. 원본 byte buffer를 복제하지 않고 먼저 SHA-256과 byte 크기를 계산합니다.
        let hash = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
        let size = bytes.len() as u64;

        // 11. byte buffer의 소유권을 String으로 이동하며 UTF-8을 검증합니다.
        let content = String::from_utf8(bytes)
            .map_err(|_| MarkdownReadError::InvalidUtf8(path.to_string()))?;

        // 성공적으로 정적 스냅샷이 완성되었으므로 도메인 객체를 생성해 Ok로 돌려줍니다.
        Ok(MarkdownDocument {
            path: path.clone(),
            content,
            hash,
            size,
            modified_at: after
                .modified()
                .map_err(|source| map_io_error(path.as_str(), source))?,
        })
    }
}

/// 클로저 형태의 시도 작업을 감싸서 실행하고, 동시성 충돌 감지 시 딱 한 번 재수행의 기회를 줍니다.
///
/// ## 클로저 트레이트 (`FnMut`)
/// `FnMut` 트레이트를 사용하여 클로저의 내부 상태 변수(테스트용 호출 카운터 등)를 호출 과정에서
/// 안전하게 수정할 수 있게 유연성을 열어 둡니다.
fn with_single_retry<T>(
    mut attempt: impl FnMut() -> Result<T, ReadAttemptError>,
) -> Result<T, MarkdownReadError> {
    match attempt() {
        // 첫 번째 시도에서 한 방에 통과하면 즉시 완료
        Ok(value) => Ok(value),
        // 일반적 에러(파일 없음 등) 발생 시 재시도 없이 상위 전파
        Err(ReadAttemptError::Read(error)) => Err(error),
        // 동시 편집으로 읽는 도중에 내용 변경이 검출된 경우: 단 한 번 더 시도
        Err(ReadAttemptError::Changed) => match attempt() {
            Ok(value) => Ok(value),
            Err(ReadAttemptError::Read(error)) => Err(error),
            // 두 번째 재시도마저도 연속으로 도중에 내용이 바뀌었다면 극심한 병목 상태이므로 갈등 에러를 던집니다.
            Err(ReadAttemptError::Changed) => Err(MarkdownReadError::ReadConflict),
        },
    }
}

/// 읽은 크기가 허용된 한계를 초과하는 경우 에러 객체를 생성합니다.
fn enforce_size(path: &MarkdownPath, observed: u64, maximum: u64) -> Result<(), MarkdownReadError> {
    if observed > maximum {
        return Err(MarkdownReadError::FileTooLarge {
            path: path.to_string(),
            observed,
            maximum,
        });
    }
    Ok(())
}

/// 읽기 이전(`before`)과 읽기 이후(`after`)의 메타데이터를 대조해
/// 파일 용량 변동, 읽은 바이트 총량 불일치, 또는 수정 시간(`modified`)의 차이가 잡히는지 확인합니다.
fn metadata_changed(
    before: &Metadata,
    after: &Metadata,
    bytes_read: u64,
) -> Result<bool, MarkdownReadError> {
    let before_modified = before.modified().map_err(MarkdownReadError::Metadata)?;
    let after_modified = after.modified().map_err(MarkdownReadError::Metadata)?;
    Ok(before.len() != after.len()
        || after.len() != bytes_read
        || before_modified != after_modified)
}

/// 저수준 I/O 계층에서 올라오는 오류를 도메인 서비스가 이해할 수 있는 오류들로 구분 매핑해줍니다.
fn map_io_error(path: &str, source: io::Error) -> MarkdownReadError {
    if source.kind() == io::ErrorKind::NotFound {
        MarkdownReadError::NotFound(path.to_owned())
    } else {
        MarkdownReadError::Io {
            path: path.to_owned(),
            source,
        }
    }
}

/// 개별 읽기 트랜잭션 시도 실패의 원인을 임시 보관하는 용도의 내부용 열거형(Enum)입니다.
enum ReadAttemptError {
    /// 읽기 전후로 파일의 상태가 어긋났음을 의미 (재시도 기회 제공 대상)
    Changed,
    /// 디렉터리 경로 파싱 불가능 등 실질적인 조회 자체의 문제 발생 의미
    Read(MarkdownReadError),
}

/// `MarkdownReadError`를 `ReadAttemptError`로 변환(From)하는 자동화 규칙을 정의해 컴파일러가 코드를 유연하게 다루도록 합니다.
impl From<MarkdownReadError> for ReadAttemptError {
    fn from(value: MarkdownReadError) -> Self {
        Self::Read(value)
    }
}

/// 마크다운 파일을 안전하게 읽어오는 라이프사이클 전반에서 일어나는 세부 오류 정보들의 정의입니다.
#[derive(Debug, Error)]
pub enum MarkdownReadError {
    /// Vault 영역 검증 및 접근 상에서 실패한 에러
    #[error(transparent)]
    Vault(#[from] VaultError),

    /// 마크다운 파일이 존재하지 않는 경우의 에러
    #[error("Markdown file does not exist: {0}")]
    NotFound(String),

    /// 일반 텍스트 파일이 아닌 폴더나 단말 소켓 장치 등을 읽으려 할 때의 에러
    #[error("path is not a regular file: {0}")]
    NotRegularFile(String),

    /// 보안 정책 한도 이상의 비정상 크기의 마크다운 파일인 경우의 에러
    #[error("Markdown file is too large: {observed} bytes; maximum is {maximum} bytes: {path}")]
    FileTooLarge {
        path: String,
        observed: u64,
        maximum: u64,
    },

    /// UTF-8 이 규격에 맞지 않아 텍스트로 복원할 수 없을 때의 에러
    #[error("Markdown file is not valid UTF-8: {0}")]
    InvalidUtf8(String),

    /// 다수의 작성자가 동시 수정을 시도하여 연속 2회 이상 정합성 충돌이 포착될 때의 에러
    #[error("Markdown file changed repeatedly while being read")]
    ReadConflict,

    /// 기타 시스템 콜 입출력 오류
    #[error("filesystem operation failed for {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },

    /// 파일 시간이나 용량 등 메타데이터 정보를 획득하는 데 실패할 때의 에러
    #[error("file metadata is unavailable: {0}")]
    Metadata(#[source] io::Error),
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::{
        MarkdownReadError, MarkdownReader, ReadAttemptError, VaultRoot, enforce_size,
        with_single_retry,
    };
    use crate::domain::path::MarkdownPath;

    /// 테스트용 헬퍼 함수로 일반 문자열을 `MarkdownPath` 타입으로 검증 변환합니다.
    fn markdown_path(value: &str) -> MarkdownPath {
        MarkdownPath::parse(value).expect("test Markdown path must be valid")
    }

    /// 파일 속 유니코드 한국어가 깨지지 않고 읽히며 내용에 매핑된 올바른 해시값을 발급하는지 검증합니다.
    #[test]
    fn reads_utf8_content_and_builds_sha256_metadata() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        fs::create_dir(directory.path().join("프로젝트"))
            .expect("nested directory should be created");
        fs::write(directory.path().join("프로젝트/지식 노트.md"), "# 지식\n")
            .expect("Markdown should be written");
        let reader = MarkdownReader::new(
            VaultRoot::open(directory.path()).expect("Vault should open"),
            5 * 1024 * 1024,
        );

        let document = reader
            .read(&markdown_path("프로젝트/지식 노트.md"))
            .expect("Markdown should be read");

        assert_eq!(document.content, "# 지식\n");
        assert_eq!(document.size, "# 지식\n".len() as u64);
        assert_eq!(document.hash.len(), "sha256:".len() + 64);
        assert!(document.hash.starts_with("sha256:"));
    }

    #[test]
    fn computes_the_standard_sha256_digest() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        fs::write(directory.path().join("hash.md"), "abc").expect("Markdown should be written");
        let reader = MarkdownReader::new(
            VaultRoot::open(directory.path()).expect("Vault should open"),
            1024,
        );

        let document = reader
            .read(&markdown_path("hash.md"))
            .expect("Markdown should be read");

        assert_eq!(
            document.hash,
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    /// 최대 수용 파일의 용량 한계선(Limit boundary) 필터가 정확히 바이트 단위로 작동하는지 검증합니다.
    #[test]
    fn accepts_empty_and_exact_limit_files_but_rejects_larger_files() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        fs::write(directory.path().join("empty.md"), "").expect("empty file should be written");
        fs::write(directory.path().join("exact.md"), "a".repeat(32))
            .expect("exact file should be written");
        fs::write(directory.path().join("large.md"), "a".repeat(33))
            .expect("large file should be written");
        let reader = MarkdownReader::new(
            VaultRoot::open(directory.path()).expect("Vault should open"),
            32,
        );

        assert!(reader.read(&markdown_path("empty.md")).is_ok());
        assert!(reader.read(&markdown_path("exact.md")).is_ok());
        assert!(matches!(
            reader.read(&markdown_path("large.md")),
            Err(MarkdownReadError::FileTooLarge { .. })
        ));
    }

    #[test]
    fn accepts_the_default_five_mib_boundary() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        let maximum = 5 * 1024 * 1024;
        fs::write(directory.path().join("maximum.md"), vec![b'a'; maximum])
            .expect("maximum file should be written");
        let reader = MarkdownReader::new(
            VaultRoot::open(directory.path()).expect("Vault should open"),
            maximum as u64,
        );

        let document = reader
            .read(&markdown_path("maximum.md"))
            .expect("five MiB should be accepted");
        assert_eq!(document.size, maximum as u64);
    }

    #[test]
    fn rejects_an_observed_growth_beyond_the_limit() {
        let error = enforce_size(&markdown_path("growing.md"), 33, 32)
            .expect_err("growth beyond the limit should be rejected");

        assert!(matches!(
            error,
            MarkdownReadError::FileTooLarge {
                observed: 33,
                maximum: 32,
                ..
            }
        ));
    }

    /// 파일이 아닌 폴더를 마크다운 파일로 읽으려 하거나, 없는 파일 조회, 혹은 유효하지 않은 UTF-8 바이트 시퀀스가 깨지는 상황을 거부하는지 테스트합니다.
    #[test]
    fn rejects_directories_missing_files_and_invalid_utf8() {
        let directory = TempDir::new().expect("temporary Vault should be created");
        fs::create_dir(directory.path().join("directory.md")).expect("directory should be created");
        fs::write(directory.path().join("invalid.md"), [0xff, 0xfe])
            .expect("invalid UTF-8 should be written");
        let reader = MarkdownReader::new(
            VaultRoot::open(directory.path()).expect("Vault should open"),
            1024,
        );

        assert!(matches!(
            reader.read(&markdown_path("directory.md")),
            Err(MarkdownReadError::NotRegularFile(_))
        ));
        assert!(matches!(
            reader.read(&markdown_path("missing.md")),
            Err(MarkdownReadError::Vault(super::VaultError::TargetNotFound(
                _
            )))
        ));
        assert!(matches!(
            reader.read(&markdown_path("invalid.md")),
            Err(MarkdownReadError::InvalidUtf8(_))
        ));
    }

    /// 파일 내용이 한 번 변경 감지되었으나, 그 다음 시도 시점에는 안전하게 유지되면
    /// 1회 복구 절차가 정상적으로 완수되는지 통합적인 테스트를 진행합니다.
    #[test]
    fn retries_one_changed_snapshot_then_returns_success() {
        let mut calls = 0;
        let result = with_single_retry(|| {
            calls += 1;
            if calls == 1 {
                Err(ReadAttemptError::Changed)
            } else {
                Ok("stable")
            }
        });

        assert_eq!(result.expect("second snapshot should succeed"), "stable");
        assert_eq!(calls, 2);
    }

    /// 읽기를 재차 수행하는 두 번째 단계마저 연속으로 상태 변경이 일어나면
    /// 무한 루프나 일관성 붕괴에 빠지지 않고 즉시 conflict 에러를 보고하는지 검증합니다.
    #[test]
    fn returns_conflict_after_two_changed_snapshots() {
        let result: Result<(), MarkdownReadError> =
            with_single_retry(|| Err(ReadAttemptError::Changed));

        assert!(matches!(result, Err(MarkdownReadError::ReadConflict)));
    }
}
