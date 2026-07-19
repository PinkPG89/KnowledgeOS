use std::time::SystemTime;

use super::path::MarkdownPath;

/// 파일 시스템으로부터 안정적으로 로드해 온 단일 마크다운 문서에 관한
/// 데이터 정보와 무결성 메타데이터를 일체형으로 담고 있는 핵심 도메인 모델(Entity)입니다.
///
/// ## 도메인 데이터 무결성 보장
/// - 이 타입은 변경 불가능한(immutable) 상태 스냅샷을 나타냅니다.
/// - `Eq, PartialEq` 트레이트를 구현하여 두 문서 엔티티가 메모리 레벨에서 정확히 동등한지
///   쉽고 빠르게 대조 검증할 수 있도록 지원합니다.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkdownDocument {
    /// 검증 완료된 마크다운 문서의 고유한 가상 상대 경로
    pub path: MarkdownPath,
    /// 마크다운 문서 내에 작성된 텍스트 전체 원문
    pub content: String,
    /// 문서 본문 훼손 및 동시성 병목을 탐지하기 위해 콘텐츠를 해싱한 SHA256 체크섬 키 (예: "sha256:...")
    pub hash: String,
    /// 문서 내용의 실제 바이트 크기 (u64)
    pub size: u64,
    /// 파일 시스템(OS)상에서 마지막으로 해당 파일이 수정 및 기입된 표준 절대 시각
    pub modified_at: SystemTime,
}
