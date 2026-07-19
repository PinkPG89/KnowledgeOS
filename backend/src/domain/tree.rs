use std::time::SystemTime;

use crate::domain::path::CanonicalPath;

/// 한 디렉터리의 직계 자식만 담는 lazy tree 조회 결과입니다.
#[derive(Debug, Eq, PartialEq)]
pub struct DirectoryListing {
    pub path: Option<CanonicalPath>,
    pub entries: Vec<TreeEntry>,
}

/// 프론트엔드 파일 트리에 표시할 하나의 디렉터리 또는 Markdown 파일입니다.
#[derive(Debug, Eq, PartialEq)]
pub struct TreeEntry {
    pub kind: TreeEntryKind,
    pub name: String,
    pub path: CanonicalPath,
    pub size: Option<u64>,
    pub modified_at: SystemTime,
}

/// Directory를 별도 타입으로 구분해 정렬과 JSON 변환에서 문자열 비교를 피합니다.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreeEntryKind {
    Directory,
    File,
}
