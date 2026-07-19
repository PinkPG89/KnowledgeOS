use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    domain::path::PathError,
    infrastructure::{
        markdown::MarkdownReadError, markdown_writer::MarkdownUpdateError,
        markdown_writer::MarkdownWriteError, vault::VaultError,
    },
};

/// 외부 클라이언트(프론트엔드 및 AI 어댑터)에 반환할 구조화된 HTTP 에러 응답 개체입니다.
///
/// ## 웹 API 에러 표준화 설계
/// - 단순히 HTTP 상태 코드만 돌려주는 대신, 기계가 즉각 분기 처리할 수 있는 고유 식별 코드(`code`)와
///   사람이 읽고 디버깅할 수 있는 에러 메시지(`message`), 그리고 구체적인 콘텍스트 정보(`details`)를
///   규격화된 JSON 데이터 포맷으로 전송합니다.
#[derive(Debug)]
pub struct ApiError {
    /// HTTP 응답 상태 코드 (예: 404, 403, 500 등)
    status: StatusCode,
    /// 에러 종류를 나타내는 기계 인식용 영문 식별 키 (예: `file_not_found`)
    code: &'static str,
    /// 에러 현상을 요약한 개발자 디버깅용 메시지
    message: &'static str,
    /// 상세 오류 위치나 수치 등을 담는 JSON 개체 (선택 사항)
    details: Option<Value>,
}

impl ApiError {
    /// 인프라 계층의 마크다운 읽기 과정(`MarkdownReadError`)에서 도출된 상세 실패 사유들을
    /// 클라이언트가 이해할 수 있는 알맞은 HTTP 상태 코드와 메시지로 맵핑 및 가공해 반환합니다.
    pub fn from_read(error: MarkdownReadError) -> Self {
        // Rust의 강력한 매칭 문법을 활용하여 하위 에러를 낱낱이 분해합니다.
        match error {
            // 저장소 권한 및 격리 문제인 경우 Vault용 매퍼로 이관
            MarkdownReadError::Vault(error) => Self::from_read_vault(error),
            // 파일이 유실되었을 때: 404 Not Found 리턴
            MarkdownReadError::NotFound(path) => Self::new(
                StatusCode::NOT_FOUND,
                "file_not_found",
                "Markdown file was not found",
                Some(json!({ "path": path })),
            ),
            // 디렉터리 등 특수 파일 접근 시도 시: 422 Unprocessable Entity 리턴
            MarkdownReadError::NotRegularFile(path) => Self::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "not_a_regular_file",
                "Path does not reference a regular file",
                Some(json!({ "path": path })),
            ),
            // 설정된 용량 한도를 어겼을 때: 413 Payload Too Large 리턴
            MarkdownReadError::FileTooLarge {
                path,
                observed,
                maximum,
            } => Self::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "file_too_large",
                "Markdown file exceeds the configured size limit",
                Some(json!({
                    "path": path,
                    "observed_bytes": observed,
                    "maximum_bytes": maximum
                })),
            ),
            // UTF-8 포맷 위반 시: 422 Unprocessable Entity 리턴
            MarkdownReadError::InvalidUtf8(path) => Self::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_utf8",
                "Markdown file is not valid UTF-8",
                Some(json!({ "path": path })),
            ),
            // 읽는 중 연속된 내용 수정으로 스냅샷 취득이 깨졌을 때: 409 Conflict 리턴
            MarkdownReadError::ReadConflict => Self::new(
                StatusCode::CONFLICT,
                "read_conflict",
                "Markdown file changed repeatedly while being read",
                None,
            ),
            // 저수준 입출력 문제 발생 시: 서버 에러 로그를 기록하고, 민감 정보 노출을 피하기 위해 단순 500 에러 포맷 리턴
            MarkdownReadError::Io { path, source } => {
                tracing::error!(%path, %source, "Markdown read I/O failure");
                Self::internal()
            }
            // 메타데이터 획득 실패 시: 에러 로깅 후 500 에러 처리
            MarkdownReadError::Metadata(source) => {
                tracing::error!(%source, "Markdown metadata failure");
                Self::internal()
            }
        }
    }

    /// Markdown 생성 계층의 typed error를 공개 HTTP 오류 계약으로 변환합니다.
    pub fn from_write(error: MarkdownWriteError) -> Self {
        match error {
            MarkdownWriteError::Vault(error) => Self::from_write_vault(error),
            MarkdownWriteError::AlreadyExists(path) => Self::new(
                StatusCode::CONFLICT,
                "file_already_exists",
                "A file or directory already exists at the requested path",
                Some(json!({ "path": path })),
            ),
            MarkdownWriteError::FileTooLarge {
                path,
                observed,
                maximum,
            } => Self::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "file_too_large",
                "Markdown content exceeds the configured size limit",
                Some(json!({
                    "path": path,
                    "observed_bytes": observed,
                    "maximum_bytes": maximum
                })),
            ),
            MarkdownWriteError::Io { path, source } => {
                tracing::error!(%path, %source, "Markdown create I/O failure");
                Self::internal()
            }
            MarkdownWriteError::Metadata { path, source } => {
                tracing::error!(%path, %source, "Markdown create metadata failure");
                Self::internal()
            }
            MarkdownWriteError::LockPoisoned => {
                tracing::error!("Markdown create lock is poisoned");
                Self::internal()
            }
        }
    }

    pub fn from_update(error: MarkdownUpdateError) -> Self {
        match error {
            MarkdownUpdateError::Vault(error) => Self::from_read_vault(error),
            MarkdownUpdateError::NotFound(path) => Self::new(
                StatusCode::NOT_FOUND,
                "file_not_found",
                "Markdown file was not found",
                Some(json!({ "path": path })),
            ),
            MarkdownUpdateError::NotRegularFile(path) => Self::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "not_a_regular_file",
                "Path does not reference a regular file",
                Some(json!({ "path": path })),
            ),
            MarkdownUpdateError::FileTooLarge {
                path,
                observed,
                maximum,
            } => Self::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "file_too_large",
                "Markdown content exceeds the configured size limit",
                Some(json!({
                    "path": path,
                    "observed_bytes": observed,
                    "maximum_bytes": maximum
                })),
            ),
            MarkdownUpdateError::InvalidUtf8(path) => Self::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_utf8",
                "Markdown file is not valid UTF-8",
                Some(json!({ "path": path })),
            ),
            MarkdownUpdateError::ReadConflict => Self::new(
                StatusCode::CONFLICT,
                "write_conflict",
                "Markdown file changed repeatedly while preparing the update",
                None,
            ),
            MarkdownUpdateError::HashConflict {
                path,
                expected: _,
                actual,
            } => Self::new(
                StatusCode::CONFLICT,
                "write_conflict",
                "Markdown file has changed since it was read",
                Some(json!({ "path": path, "current_hash": actual })),
            ),
            MarkdownUpdateError::Io { path, source } => {
                tracing::error!(%path, %source, "Markdown update I/O failure");
                Self::internal()
            }
            MarkdownUpdateError::Metadata { path, source } => {
                tracing::error!(%path, %source, "Markdown update metadata failure");
                Self::internal()
            }
            MarkdownUpdateError::LockPoisoned => {
                tracing::error!("Markdown update lock is poisoned");
                Self::internal()
            }
            MarkdownUpdateError::InvalidTarget => {
                tracing::error!("Markdown update target has no parent directory");
                Self::internal()
            }
        }
    }

    #[must_use]
    pub fn invalid_base_hash() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_base_hash",
            "base_hash must use sha256 followed by 64 lowercase hexadecimal characters",
            None,
        )
    }

    pub fn invalid_request(error: &serde_json::Error) -> Self {
        tracing::debug!(%error, "invalid create-file JSON request");
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Request body must be valid JSON with path and content strings",
            None,
        )
    }

    pub fn request_too_large(error: &axum::Error, maximum: u64) -> Self {
        tracing::debug!(%error, "create-file request body could not be buffered");
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "file_too_large",
            "Request body exceeds the accepted size limit",
            Some(json!({ "maximum_content_bytes": maximum })),
        )
    }

    /// 비동기 전용 스레드 풀(`spawn_blocking`) 내에서 스레드 조인 실패 등이 났을 때 처리하는 핸들러입니다.
    pub fn task_failure(error: &tokio::task::JoinError) -> Self {
        tracing::error!(%error, "blocking Markdown read task failed");
        Self::internal()
    }

    /// 저장소 경계 관리(Vault) 에러를 수신하여 적절한 HTTP 코드로 분기합니다.
    fn from_read_vault(error: VaultError) -> Self {
        match error {
            // 경로 대상을 찾지 못함 -> 404
            VaultError::TargetNotFound(path) => Self::new(
                StatusCode::NOT_FOUND,
                "file_not_found",
                "Markdown file was not found",
                Some(json!({ "path": path })),
            ),
            // 심볼릭 링크 등 불허된 대상 진입 시도 -> 403 Forbidden
            VaultError::SymlinkNotAllowed(path) => Self::new(
                StatusCode::FORBIDDEN,
                "path_not_allowed",
                "Path is not allowed by the active Vault policy",
                Some(json!({ "path": path })),
            ),
            // Vault 밖으로 탈출 시도 탐지 -> 경고 로깅을 남기고 403 Forbidden 리턴 (보안 침투 감시 목적)
            VaultError::OutsideVault(path) => {
                tracing::warn!(resolved_path = %path.display(), "Vault containment rejected a path");
                Self::new(
                    StatusCode::FORBIDDEN,
                    "path_not_allowed",
                    "Path is not allowed by the active Vault policy",
                    None,
                )
            }
            // 중간 계층에 디렉터리가 아닌 요소가 포함됨 -> 422
            VaultError::NonDirectoryAncestor(path) => Self::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "not_a_regular_file",
                "A path segment is not a directory",
                Some(json!({ "path": path })),
            ),
            // 나머지 알 수 없는 저수준 볼트/디스크 에러 -> 500 에러 처리
            other => {
                tracing::error!(error = %other, "Vault read failure");
                Self::internal()
            }
        }
    }

    fn from_write_vault(error: VaultError) -> Self {
        match error {
            VaultError::ParentNotFound(path) => Self::new(
                StatusCode::NOT_FOUND,
                "parent_not_found",
                "Parent directory was not found",
                Some(json!({ "path": path })),
            ),
            VaultError::NonDirectoryAncestor(path) => Self::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "parent_not_directory",
                "A parent path segment is not a directory",
                Some(json!({ "path": path })),
            ),
            VaultError::SymlinkNotAllowed(path) => Self::new(
                StatusCode::FORBIDDEN,
                "path_not_allowed",
                "Path is not allowed by the active Vault policy",
                Some(json!({ "path": path })),
            ),
            VaultError::OutsideVault(path) => {
                tracing::warn!(resolved_path = %path.display(), "Vault containment rejected a create path");
                Self::new(
                    StatusCode::FORBIDDEN,
                    "path_not_allowed",
                    "Path is not allowed by the active Vault policy",
                    None,
                )
            }
            other => {
                tracing::error!(error = %other, "Vault create failure");
                Self::internal()
            }
        }
    }

    /// 신규 에러 객체를 규격대로 생성해 내는 생성 헬퍼 함수입니다.
    fn new(
        status: StatusCode,
        code: &'static str,
        message: &'static str,
        details: Option<Value>,
    ) -> Self {
        Self {
            status,
            code,
            message,
            details,
        }
    }

    /// 500 Internal Server Error(서버 내부 오류) 응답을 즉석 조립합니다.
    pub(crate) fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Internal server error",
            None,
        )
    }
}

/// 경로 관련 비즈니스 규칙 위반 에러(`PathError`)가 감지되었을 때,
/// 이 타입을 `ApiError`로 안전하고 편안하게 상향 자동 변환할 수 있도록 `From` 트레이트를 구현합니다.
impl From<PathError> for ApiError {
    fn from(error: PathError) -> Self {
        let path_error = error.to_string();
        // 특히 마크다운 확장자 미충족 이슈인 경우, 원인을 자세히 표출하며 422 상태코드로 유도합니다.
        if matches!(error, PathError::MarkdownExtensionRequired) {
            return Self::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "not_a_markdown_file",
                "Path must reference a lowercase .md file",
                Some(json!({ "reason": path_error })),
            );
        }

        // 그 외의 일반 경로 오염 에러(Traversals 등) -> 400 Bad Request
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_path",
            "Path is invalid",
            Some(json!({ "reason": path_error })),
        )
    }
}

/// Axum 웹 프레임워크가 핸들러 결과로 리턴된 `ApiError`를 보고
/// 자동으로 표준 HTTP 응답 구조로 인코딩할 수 있도록 `IntoResponse` 인터페이스를 제공합니다.
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // 규격화된 봉투 구조체(`ErrorEnvelope`)에 에러 정보를 집어넣습니다.
        let body = ErrorEnvelope {
            error: ErrorBody {
                code: self.code,
                message: self.message,
                details: self.details,
            },
        };
        // 설정된 상태코드와 JSON 데이터 바디를 조립하여 최종 Axum Response로 격상시킵니다.
        (self.status, Json(body)).into_response()
    }
}

/// 외부 통신용 최상위 에러 응답 봉투(Envelope) 구조체입니다.
#[derive(Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

/// 실제 JSON 데이터 본문 형태의 구체적 에러 사양서 구조체입니다.
#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: &'static str,
    /// `details` 필드가 만약 비어 있으면(None), 클라이언트에 나가는 JSON 텍스트에서
    /// 아예 이 필드를 제외(`skip_serializing_if`)하여 바디를 깔끔하고 가볍게 조절합니다.
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}
