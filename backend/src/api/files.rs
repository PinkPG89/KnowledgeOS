use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use serde::Serialize;
use time::{OffsetDateTime, format_description::FormatItem, macros::format_description};

use crate::{
    domain::{document::MarkdownDocument, path::MarkdownPath},
    state::AppState,
};

use super::error::ApiError;

const RFC3339_MILLISECONDS: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");

pub fn router() -> Router<AppState> {
    Router::new().route("/{*path}", get(read_file))
}

async fn read_file(
    State(state): State<AppState>,
    Path(raw_path): Path<String>,
) -> Result<Json<ReadFileResponse>, ApiError> {
    let path = MarkdownPath::parse(&raw_path)?;
    let reader = state.markdown_reader.clone();
    let document = tokio::task::spawn_blocking(move || reader.read(&path))
        .await
        .map_err(|error| ApiError::task_failure(&error))?
        .map_err(ApiError::from_read)?;

    Ok(Json(ReadFileResponse::try_from(document)?))
}

#[derive(Debug, Serialize)]
struct ReadFileResponse {
    path: String,
    content: String,
    hash: String,
    size: u64,
    modified_at: String,
}

impl TryFrom<MarkdownDocument> for ReadFileResponse {
    type Error = ApiError;

    fn try_from(document: MarkdownDocument) -> Result<Self, Self::Error> {
        let modified_at = OffsetDateTime::from(document.modified_at)
            .format(RFC3339_MILLISECONDS)
            .map_err(|error| {
                tracing::error!(%error, "failed to format Markdown modified time");
                ApiError::internal()
            })?;

        Ok(Self {
            path: document.path.to_string(),
            content: document.content,
            hash: document.hash,
            size: document.size,
            modified_at,
        })
    }
}
