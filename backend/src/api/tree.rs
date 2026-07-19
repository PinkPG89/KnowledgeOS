use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::FormatItem, macros::format_description};

use crate::{
    domain::{
        path::CanonicalPath,
        tree::{DirectoryListing, TreeEntry, TreeEntryKind},
    },
    state::AppState,
};

use super::error::ApiError;

const RFC3339_MILLISECONDS: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");

pub fn router() -> Router<AppState> {
    Router::new().route("/tree", get(list_tree))
}

#[derive(Debug, Default, Deserialize)]
struct TreeQuery {
    path: Option<String>,
}

async fn list_tree(
    State(state): State<AppState>,
    Query(query): Query<TreeQuery>,
) -> Result<Json<TreeResponse>, ApiError> {
    let directory = query
        .path
        .filter(|path| !path.is_empty())
        .map(|path| CanonicalPath::parse(&path))
        .transpose()?;
    let reader = state.tree_reader.clone();
    let listing = tokio::task::spawn_blocking(move || reader.list(directory.as_ref()))
        .await
        .map_err(|error| ApiError::task_failure(&error))?
        .map_err(ApiError::from_tree)?;

    Ok(Json(TreeResponse::try_from(listing)?))
}

#[derive(Debug, Serialize)]
struct TreeResponse {
    path: String,
    entries: Vec<TreeEntryResponse>,
}

impl TryFrom<DirectoryListing> for TreeResponse {
    type Error = ApiError;

    fn try_from(listing: DirectoryListing) -> Result<Self, Self::Error> {
        let entries = listing
            .entries
            .into_iter()
            .map(TreeEntryResponse::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            path: listing
                .path
                .map_or_else(String::new, |path| path.to_string()),
            entries,
        })
    }
}

#[derive(Debug, Serialize)]
struct TreeEntryResponse {
    #[serde(rename = "type")]
    kind: TreeEntryType,
    name: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
    modified_at: String,
}

impl TryFrom<TreeEntry> for TreeEntryResponse {
    type Error = ApiError;

    fn try_from(entry: TreeEntry) -> Result<Self, Self::Error> {
        let modified_at = OffsetDateTime::from(entry.modified_at)
            .format(RFC3339_MILLISECONDS)
            .map_err(|error| {
                tracing::error!(%error, "failed to format tree entry modified time");
                ApiError::internal()
            })?;

        Ok(Self {
            kind: entry.kind.into(),
            name: entry.name,
            path: entry.path.to_string(),
            size: entry.size,
            modified_at,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum TreeEntryType {
    Directory,
    File,
}

impl From<TreeEntryKind> for TreeEntryType {
    fn from(kind: TreeEntryKind) -> Self {
        match kind {
            TreeEntryKind::Directory => Self::Directory,
            TreeEntryKind::File => Self::File,
        }
    }
}
