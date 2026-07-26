use super::path::MarkdownPath;

/// Markdown 원문에서 파생한 검색용 projection입니다.
///
/// 이 값은 삭제 후 다시 만들 수 있는 cache이며 Markdown 파일을 대체하는 원본이 아닙니다.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkdownProjection {
    pub path: MarkdownPath,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub links: Vec<ProjectionLink>,
    pub content_hash: String,
    pub frontmatter_json: Option<String>,
    pub frontmatter_status: FrontmatterStatus,
}

/// Frontmatter 처리 결과를 오류가 아닌 진단 정보로 보존합니다.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrontmatterStatus {
    Absent,
    Parsed,
    Malformed,
}

/// 검색·backlink 단계에서 해석할 수 있도록 보존하는 문서 참조입니다.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProjectionLink {
    pub kind: ProjectionLinkKind,
    pub target: String,
}

/// 링크가 작성된 Markdown 문법 종류입니다.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProjectionLinkKind {
    Markdown,
    MarkdownImage,
    Wiki,
    WikiEmbed,
}
