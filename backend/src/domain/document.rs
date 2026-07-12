use std::time::SystemTime;

use super::path::MarkdownPath;

/// filesystem에서 안정적으로 읽어 온 Markdown 원문과 변경 감지 metadata입니다.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkdownDocument {
    pub path: MarkdownPath,
    pub content: String,
    pub hash: String,
    pub size: u64,
    pub modified_at: SystemTime,
}
