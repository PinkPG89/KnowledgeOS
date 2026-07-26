use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::{
    domain::{
        path::CanonicalPath,
        search::{SearchQuery, SearchResult},
    },
    state::AppState,
};

use super::error::ApiError;

const DEFAULT_SEARCH_LIMIT: u32 = 20;
const MAX_SEARCH_LIMIT: u32 = 100;
const MAX_SEARCH_QUERY_BYTES: usize = 512;

pub fn router() -> Router<AppState> {
    Router::new().route("/search", get(search))
}

#[derive(Debug, Deserialize)]
struct RawSearchQuery {
    q: Option<String>,
    path_prefix: Option<String>,
    limit: Option<String>,
}

async fn search(
    State(state): State<AppState>,
    raw: Result<Query<RawSearchQuery>, QueryRejection>,
) -> Result<Json<SearchResponse>, ApiError> {
    let Query(raw) = raw.map_err(|error| ApiError::invalid_search_parameters(&error))?;
    let query = validate_query(raw)?;
    let index = state
        .search_index
        .clone()
        .ok_or_else(ApiError::search_unavailable)?;
    let search_query = query.clone();
    let results = tokio::task::spawn_blocking(move || index.search(&search_query))
        .await
        .map_err(|error| ApiError::task_failure(&error))?
        .map_err(|error| ApiError::from_search(&error))?;
    Ok(Json(SearchResponse::from_query(query, results)))
}

fn validate_query(raw: RawSearchQuery) -> Result<SearchQuery, ApiError> {
    let text = raw
        .q
        .as_deref()
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .ok_or_else(ApiError::invalid_search_query)?;
    if text.len() > MAX_SEARCH_QUERY_BYTES {
        return Err(ApiError::invalid_search_query());
    }
    let path_prefix = raw
        .path_prefix
        .filter(|path| !path.is_empty())
        .map(|path| CanonicalPath::parse(&path))
        .transpose()?;
    let limit = match raw.limit {
        Some(limit) => limit
            .parse::<u32>()
            .ok()
            .filter(|limit| (1..=MAX_SEARCH_LIMIT).contains(limit))
            .ok_or_else(ApiError::invalid_search_limit)?,
        None => DEFAULT_SEARCH_LIMIT,
    };
    Ok(SearchQuery {
        text: text.to_owned(),
        path_prefix,
        limit,
    })
}

#[derive(Debug, Serialize)]
struct SearchResponse {
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path_prefix: Option<String>,
    limit: u32,
    results: Vec<SearchResultResponse>,
}

impl SearchResponse {
    fn from_query(query: SearchQuery, results: Vec<SearchResult>) -> Self {
        Self {
            query: query.text,
            path_prefix: query.path_prefix.map(|path| path.to_string()),
            limit: query.limit,
            results: results.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct SearchResultResponse {
    path: String,
    title: String,
    snippet: String,
    score: f64,
}

impl From<SearchResult> for SearchResultResponse {
    fn from(result: SearchResult) -> Self {
        Self {
            path: result.path.to_string(),
            title: result.title,
            snippet: result.snippet,
            score: result.score,
        }
    }
}
