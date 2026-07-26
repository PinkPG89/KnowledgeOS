use super::path::{CanonicalPath, MarkdownPath};

/// FTS 문법으로 변환되기 전 검증 완료된 search use case 입력입니다.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchQuery {
    pub text: String,
    pub path_prefix: Option<CanonicalPath>,
    pub limit: u32,
}

/// 재생성 가능한 search projection에서 반환하는 단일 검색 결과입니다.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub path: MarkdownPath,
    pub title: String,
    pub snippet: String,
    pub score: f64,
}
