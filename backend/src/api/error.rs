use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    domain::path::PathError,
    infrastructure::{markdown::MarkdownReadError, vault::VaultError},
};

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
    details: Option<Value>,
}

impl ApiError {
    pub fn from_read(error: MarkdownReadError) -> Self {
        match error {
            MarkdownReadError::Vault(error) => Self::from_vault(error),
            MarkdownReadError::NotFound(path) => Self::new(
                StatusCode::NOT_FOUND,
                "file_not_found",
                "Markdown file was not found",
                Some(json!({ "path": path })),
            ),
            MarkdownReadError::NotRegularFile(path) => Self::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "not_a_regular_file",
                "Path does not reference a regular file",
                Some(json!({ "path": path })),
            ),
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
            MarkdownReadError::InvalidUtf8(path) => Self::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_utf8",
                "Markdown file is not valid UTF-8",
                Some(json!({ "path": path })),
            ),
            MarkdownReadError::ReadConflict => Self::new(
                StatusCode::CONFLICT,
                "read_conflict",
                "Markdown file changed repeatedly while being read",
                None,
            ),
            MarkdownReadError::Io { path, source } => {
                tracing::error!(%path, %source, "Markdown read I/O failure");
                Self::internal()
            }
            MarkdownReadError::Metadata(source) => {
                tracing::error!(%source, "Markdown metadata failure");
                Self::internal()
            }
        }
    }

    pub fn task_failure(error: &tokio::task::JoinError) -> Self {
        tracing::error!(%error, "blocking Markdown read task failed");
        Self::internal()
    }

    fn from_vault(error: VaultError) -> Self {
        match error {
            VaultError::TargetNotFound(path) => Self::new(
                StatusCode::NOT_FOUND,
                "file_not_found",
                "Markdown file was not found",
                Some(json!({ "path": path })),
            ),
            VaultError::SymlinkNotAllowed(path) => Self::new(
                StatusCode::FORBIDDEN,
                "path_not_allowed",
                "Path is not allowed by the active Vault policy",
                Some(json!({ "path": path })),
            ),
            VaultError::OutsideVault(path) => {
                tracing::warn!(resolved_path = %path.display(), "Vault containment rejected a path");
                Self::new(
                    StatusCode::FORBIDDEN,
                    "path_not_allowed",
                    "Path is not allowed by the active Vault policy",
                    None,
                )
            }
            VaultError::NonDirectoryAncestor(path) => Self::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "not_a_regular_file",
                "A path segment is not a directory",
                Some(json!({ "path": path })),
            ),
            other => {
                tracing::error!(error = %other, "Vault read failure");
                Self::internal()
            }
        }
    }

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

    pub(crate) fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Internal server error",
            None,
        )
    }
}

impl From<PathError> for ApiError {
    fn from(error: PathError) -> Self {
        let path_error = error.to_string();
        if matches!(error, PathError::MarkdownExtensionRequired) {
            return Self::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "not_a_markdown_file",
                "Path must reference a lowercase .md file",
                Some(json!({ "reason": path_error })),
            );
        }

        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_path",
            "Path is invalid",
            Some(json!({ "reason": path_error })),
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorEnvelope {
            error: ErrorBody {
                code: self.code,
                message: self.message,
                details: self.details,
            },
        };
        (self.status, Json(body)).into_response()
    }
}

#[derive(Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}
