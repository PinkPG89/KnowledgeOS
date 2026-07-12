use std::fmt;

use thiserror::Error;

/// 경로 문자열 전체의 최대 바이트 수 제한 (1024바이트)
/// 너무 긴 경로가 메모리에 유입되는 것을 방지합니다.
const MAX_PATH_BYTES: usize = 1_024;

/// 경로 내부의 각 세그먼트(폴더나 파일명)의 최대 바이트 수 제한 (255바이트)
/// 대부분의 현대적 파일 시스템(ext4, NTFS 등)이 지원하는 파일명의 한계 크기입니다.
const MAX_SEGMENT_BYTES: usize = 255;

/// `knowledge/` 지식 원본 저장소 디렉터리를 기준으로 엄격하게 검증된 UTF-8 상대 경로 타입입니다.
///
/// ## 어휘적 정규성(Lexical Canonicality)
/// 이 타입은 실제 운영체제의 파일 시스템 디스크를 조회(I/O)하지 않으며, 어휘 분석 수준에서 단 하나의 고유한
/// 물리적 표현 구조(Lexical Expression)만을 갖도록 검증합니다.
/// - 따라서 `std::fs::canonicalize`를 통한 절대 경로 정규화 방식과는 다릅니다.
/// - 실제 상위 폴더 이탈 방지(Containment)나 심볼릭 링크 검사는 본 도메인 객체를 전달받은
///   파일 시스템 어댑터 계층(Infrastructure)이 추후 수행하게 됩니다.
///
/// ## 뉴타입 패턴(Newtype Pattern)
/// `pub struct CanonicalPath(String);`와 같이 단일 튜플 구조체로 내부 값을 래핑하면 외부에서 임의로 경로 문자열을
/// 수정할 수 없으므로, 한 번 검증된 경로는 시스템 내에서 항상 안전하고 깨끗하다는 강한 보증을 갖습니다.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CanonicalPath(String);

impl CanonicalPath {
    /// 원시 문자열(raw) 입력을 받아, 사양에 어긋나지 않는지 검증하고 `CanonicalPath` 인스턴스를 반환합니다.
    ///
    /// ## 유효성 검증 순서
    /// 1. 전체 경로 문자열 차원에서의 유효성 및 보안 검사 (`validate_whole_path`)
    /// 2. `/` 구분자로 쪼갠 개별 세그먼트(파일명, 디렉터리명)별 검사 (`validate_segment`)
    ///
    /// # Errors
    ///
    /// 빈 경로, 절대 경로, 숨겨진 파일, 상위 디렉터리 참조(`..`), Windows 스타일 드라이브명이나 경로 구분자(`\`),
    /// NUL(\0) 혹은 제어 문자, 최대 길이 한계를 어긴 경우 [`PathError`]를 에러로 반환합니다.
    pub fn parse(raw: &str) -> Result<Self, PathError> {
        // 1. 전체 경로 범위에서 공통 포맷 및 위험 요소를 검사합니다.
        validate_whole_path(raw)?;

        // 2. 경로 구분자('/')를 기준으로 세그먼트를 쪼개어 각각이 안전한 규칙을 따르는지 검사합니다.
        for segment in raw.split('/') {
            validate_segment(segment)?;
        }

        // 모든 검사를 통과했다면 소유권을 갖는 String으로 복제(to_owned)하여 인스턴스를 안전하게 빌드합니다.
        Ok(Self(raw.to_owned()))
    }

    /// 내부의 검증된 경로 문자열 슬라이스(&str)를 참조(대여)할 수 있게 해주는 헬퍼 메서드입니다.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 경로의 개별 세그먼트를 순차적으로 또는 역순으로 순회(Iterator)할 수 있게 반환합니다.
    ///
    /// `DoubleEndedIterator` 트레이트 타입으로 반환하므로, 앞에서부터 순회할 수도 있고
    /// 뒤에서부터 거꾸로 순회(예: 파일 확장자를 구하기 위해 뒤에서부터 첫 슬래시를 찾음)할 수도 있습니다.
    #[must_use]
    pub fn segments(&self) -> impl DoubleEndedIterator<Item = &str> {
        self.0.split('/')
    }
}

/// `AsRef<str>` 트레이트를 구현하여, `CanonicalPath` 인스턴스의 참조 타입을
/// 일반 문자열 참조인 `&str`로 편리하게 읽을 수 있게 합니다.
impl AsRef<str> for CanonicalPath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// 디버깅 및 포맷팅 출력 시, 내부에 래핑된 원시 경로 문자열이 그대로 화면에 표시되게 포맷터를 정의합니다.
impl fmt::Display for CanonicalPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// 문자열 슬라이스로부터 `CanonicalPath` 구조체 생성을 시도할 수 있도록 `TryFrom` 트레이트를 구현합니다.
/// `try_from`을 호출하면 내부적으로 `parse` 함수가 자동 구동됩니다.
impl TryFrom<&str> for CanonicalPath {
    type Error = PathError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

/// 지식 저장소 API가 엄격하게 취급할 수 있도록 최종 검증된 마크다운(`.md`) 파일 전용 경로 구조체입니다.
///
/// ## 도메인 모델 상의 타입 분리(Type Segregation)
/// 일반 경로를 나타내는 `CanonicalPath`와 파일명이 `.md`로 끝남을 보장하는 `MarkdownPath`를 완전히 분리하여
/// 서로 다른 타입으로 설계했습니다.
/// 이를 통해 개발자가 '디렉터리를 만들어야 하는 상황'에 잘못해서 마크다운 확장자를 검사하거나,
/// '파일을 읽어야 하는 함수'에서 마크다운 확장자 검증 과정을 누락하여 취약점이 생기는 실수를 컴파일 타임에 철저히 방지합니다.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MarkdownPath(CanonicalPath);

impl MarkdownPath {
    /// 원시 문자열 경로를 검증하고, 가장 마지막 세그먼트가 소문자 `.md` 확장자로 끝나는지 확인하여 인스턴스를 빌드합니다.
    ///
    /// # Errors
    ///
    /// 경로 유효성 정책을 위반했거나, 파일 이름에 `.md` 확장자가 아예 없거나 대문자가 섞여 있는 경우 [`PathError`]를 던집니다.
    pub fn parse(raw: &str) -> Result<Self, PathError> {
        // 우선 기본적인 Lexical 상대 경로 유효성 검사를 거치기 위해 CanonicalPath로 파싱합니다.
        let path = CanonicalPath::parse(raw)?;

        // 마지막 구분자('/') 뒷부분을 가져와서 실제 파일명이 존재하는지 확인합니다.
        let file_name = path
            .as_str()
            .rsplit('/')
            .next()
            .ok_or(PathError::EmptyPath)?;

        // 파일 이름 뒷부분의 마지막 마침표('.')를 기준으로 확장자를 분리해 냅니다.
        let extension = file_name.rsplit_once('.').map(|(_, extension)| extension);

        // 확장자가 정확히 소문자 "md"가 아닌 경우는 명확한 규격 위반이므로 에러 처리합니다.
        if extension != Some("md") {
            return Err(PathError::MarkdownExtensionRequired);
        }

        // 문제 없다면 상위 레벨의 안전한 래퍼 객체인 MarkdownPath로 반환합니다.
        Ok(Self(path))
    }

    /// 내부의 정규화된 상대 경로 구조체(`CanonicalPath`)에 대한 정적 참조를 획득합니다.
    ///
    /// `const fn`을 사용하여 컴파일 타임 상수로도 참조 조회가 가능하게 최적화되어 있습니다.
    #[must_use]
    pub const fn as_canonical(&self) -> &CanonicalPath {
        &self.0
    }

    /// 검증된 마크다운 경로의 문자열 슬라이스를 바로 대여합니다.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// `AsRef<str>` 구현을 통해 일반 문자열 참조로 자연스럽게 상향 변환할 수 있도록 지원합니다.
impl AsRef<str> for MarkdownPath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// 화면 출력을 도울 수 있게 내부 `CanonicalPath`의 `Display` 포맷터로 위임(Delegate)하여 문자열을 출력합니다.
impl fmt::Display for MarkdownPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// 문자열 슬라이스 타입 `&str`을 `MarkdownPath` 타입으로 변환하려고 안전하게 시도할 수 있도록 `TryFrom` 트레이트를 구현합니다.
impl TryFrom<&str> for MarkdownPath {
    type Error = PathError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

/// 경로 정책(Path Policy) 및 유효성 검사 위반 시 반환되는 정교한 에러 상태들의 집합입니다.
/// 이 에러 분류 구조를 통해 비즈니스 흐름을 처리하는 상위 모듈이 상황별 에러 처리를 유연하게 할 수 있습니다.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum PathError {
    /// 경로가 아무 글자도 없는 완전 빈 값일 때 발생합니다.
    #[error("path must not be empty")]
    EmptyPath,

    /// `/` 혹은 윈도우식 `C:`로 시작하는 절대 경로가 입력되었을 때 발생합니다. (보안 정책 상 상대 경로만 허용)
    #[error("path must be relative")]
    AbsolutePath,

    /// 윈도우 스타일의 역슬래시(`\`)가 경로에 포함되었을 때 발생합니다. (구분자는 오직 슬래시 `/`만 허용)
    #[error("path must use '/' separators only")]
    WindowsSeparator,

    /// 경로상에 퍼센트 기호(`%`)가 존재할 때 발생합니다.
    /// URL 디코딩 과정을 한 번 거친 뒤에 `%`가 여전히 포함되어 있으면 더블 인코딩 우회 공격(예: `%252e%252e`)의 위험이 있습니다.
    #[error("path must not contain '%' after URL decoding")]
    AmbiguousPercentEncoding,

    /// 문자열 종료를 뜻하는 NUL 문자(`\0`)가 포함되었을 때 발생합니다. (OS 수준 C언어 API 우회 주입 방지)
    #[error("path must not contain NUL")]
    NulCharacter,

    /// 개행문자(\n, \r) 등 눈에 보이지 않으며 시스템 흐름을 흐리는 제어 문자가 유입될 때 발생합니다.
    #[error("path must not contain control characters")]
    ControlCharacter,

    /// 경로의 전체 길이가 정해진 규격(`MAX_PATH_BYTES`)을 초과했을 때 발생합니다.
    #[error("path is {actual} bytes; maximum is {maximum} bytes")]
    PathTooLong { actual: usize, maximum: usize },

    /// `/`가 연달아 나타나거나 맨 앞/뒤에 붙어 빈 세그먼트(예: `a//b`, `/a/`)가 생성되었을 때 발생합니다.
    #[error("path must not contain empty segments")]
    EmptySegment,

    /// 현재 작업 위치를 지칭하는 `.` 경로 세그먼트가 포함되었을 때 발생합니다. (어휘 정규화 상태 강제)
    #[error("path must not contain '.' segments")]
    CurrentDirectorySegment,

    /// 상위 디렉터리 참조 지시자인 `..` 경로 세그먼트가 유입되었을 때 발생합니다.
    /// (디렉터리 트래버설 공격을 통해 시스템의 다른 영역에 침범하는 것을 원천 차단하는 가장 중대한 보안 규칙)
    #[error("path must not contain '..' segments")]
    ParentDirectorySegment,

    /// 파일명이나 폴더명이 `.` 마침표로 시작하는 숨겨진 파일 영역(예: `.git`, `.env`)인 경우 발생합니다.
    #[error("hidden path segments are not allowed: {0}")]
    HiddenSegment(String),

    /// 경로 내부의 단일 폴더명 또는 파일명의 길이가 한계치(`MAX_SEGMENT_BYTES`)를 넘을 때 발생합니다.
    #[error("path segment is {actual} bytes; maximum is {maximum} bytes")]
    SegmentTooLong { actual: usize, maximum: usize },

    /// 마크다운 전용 파일 경로임에도 소문자 `.md` 확장자로 끝나지 않는 대상이 입력되었을 때 발생합니다.
    #[error("Markdown file path must end with lowercase '.md'")]
    MarkdownExtensionRequired,
}

/// 경로의 유체(전체 문자열 범위) 단위에서 지켜져야 할 엄격한 보안 및 구문 제약 조건을 검증합니다.
fn validate_whole_path(raw: &str) -> Result<(), PathError> {
    // 경로가 비어 있으면 안 됩니다.
    if raw.is_empty() {
        return Err(PathError::EmptyPath);
    }

    // Unix 절대 경로('/') 또는 Windows 드라이브 지시자(C:)로 시작하면 상대 경로 원칙에 위배됩니다.
    if raw.starts_with('/') || has_windows_drive_prefix(raw) {
        return Err(PathError::AbsolutePath);
    }

    // 윈도우 스타일 경로 구분자('\')를 사용할 수 없습니다.
    if raw.contains('\\') {
        return Err(PathError::WindowsSeparator);
    }

    // 이미 URL 디코딩이 완료된 상태이므로 '%'가 남아 있으면 보안상 모호성을 제거하기 위해 금지합니다.
    if raw.contains('%') {
        return Err(PathError::AmbiguousPercentEncoding);
    }

    // NUL 바이트 주입 기법을 통한 파일 유효성 검사 우회를 방지합니다.
    if raw.contains('\0') {
        return Err(PathError::NulCharacter);
    }

    // 터미널 및 UI에 유해한 제어 문자 입력을 제한합니다.
    if raw.chars().any(char::is_control) {
        return Err(PathError::ControlCharacter);
    }

    // 경로가 너무 길어 파일 시스템이나 메모리에 문제를 일으키는 것을 막습니다.
    if raw.len() > MAX_PATH_BYTES {
        return Err(PathError::PathTooLong {
            actual: raw.len(),
            maximum: MAX_PATH_BYTES,
        });
    }

    Ok(())
}

/// 경로를 구성하는 각각의 세그먼트(파일명, 개별 디렉터리 이름) 내의 규칙을 검증합니다.
fn validate_segment(segment: &str) -> Result<(), PathError> {
    // 특수한 명칭들을 매칭하여 엄격히 불허합니다.
    match segment {
        "" => return Err(PathError::EmptySegment),
        "." => return Err(PathError::CurrentDirectorySegment),
        ".." => return Err(PathError::ParentDirectorySegment),
        _ => {}
    }

    // 시스템 및 형상 관리용 숨김 폴더/파일(예: `.git`, `.env`) 접근을 금지합니다.
    // 단, MVP 목표 중 하나인 `_trash/` 와 같은 밑줄 폴더는 통과해야 하므로 마침표 시작만 체크합니다.
    if segment.starts_with('.') {
        return Err(PathError::HiddenSegment(segment.to_owned()));
    }

    // 한글 등 유니코드 세그먼트가 바이트 수 한도를 위반하는지 제한을 둡니다.
    if segment.len() > MAX_SEGMENT_BYTES {
        return Err(PathError::SegmentTooLong {
            actual: segment.len(),
            maximum: MAX_SEGMENT_BYTES,
        });
    }

    Ok(())
}

/// 문자열이 Windows식 드라이브 명(예: `C:`, `d:`) 패턴으로 구성되어 시작하는지 식별합니다.
fn has_windows_drive_prefix(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    // 두 바이트 이상이고, 첫 바이트가 알파벳이고, 두 번째 바이트가 콜론(':') 문자인 경우 true를 반환합니다.
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// 도메인의 경로 유효성 및 보안 검증 비즈니스 규칙이 안전하게 작동하는지 검사하는 단위 테스트입니다.
#[cfg(test)]
mod tests {
    use super::{CanonicalPath, MAX_PATH_BYTES, MAX_SEGMENT_BYTES, MarkdownPath, PathError};

    /// 한글 유니코드 세그먼트와 띄어쓰기가 섞인 일반적 경로 포맷을 성공적으로 처리하는지 검증합니다.
    #[test]
    fn accepts_nested_unicode_and_spaces() {
        let path = MarkdownPath::parse("프로젝트/지식 운영체제.md").expect("path should be valid");

        assert_eq!(path.as_str(), "프로젝트/지식 운영체제.md");
    }

    /// 휴지통 역할을 할 밑줄(`_`)로 시작하는 시스템성 폴더 경로를 허용하는지 검증합니다.
    #[test]
    fn accepts_underscore_system_directories() {
        let path = MarkdownPath::parse("_trash/2026-07-12/deleted-note.md")
            .expect("underscore directories should be valid");

        assert_eq!(path.as_str(), "_trash/2026-07-12/deleted-note.md");
    }

    /// 디렉터리 경로 형태의 `CanonicalPath` 파싱이 정상 구동하고, 세그먼트들이 잘 분할되는지 검사합니다.
    #[test]
    fn canonical_path_can_represent_a_directory() {
        let path = CanonicalPath::parse("projects/knowledgeos").expect("directory should be valid");

        assert_eq!(
            path.segments().collect::<Vec<_>>(),
            ["projects", "knowledgeos"]
        );
    }

    /// 절대 경로로 진입하려 하거나 빈 경로를 보냈을 때 강력하게 거부하는지 테스트합니다.
    #[test]
    fn rejects_empty_and_absolute_paths() {
        assert_eq!(CanonicalPath::parse(""), Err(PathError::EmptyPath));
        assert_eq!(
            CanonicalPath::parse("/etc/passwd"),
            Err(PathError::AbsolutePath)
        );
        assert_eq!(
            CanonicalPath::parse("C:/notes/private.md"),
            Err(PathError::AbsolutePath)
        );
    }

    /// 현재 디렉터리(`.`) 또는 부모 디렉터리 우회(`..`) 세그먼트가 중간에 삽입되어 탈출을 시도하면 확실하게 잡아내는지 검증합니다.
    #[test]
    fn rejects_current_and_parent_directory_segments() {
        assert_eq!(
            CanonicalPath::parse("projects/./note.md"),
            Err(PathError::CurrentDirectorySegment)
        );
        assert_eq!(
            CanonicalPath::parse("projects/../secret.md"),
            Err(PathError::ParentDirectorySegment)
        );
    }

    /// 빈 세그먼트(예: 슬래시 연속 `//` 이나 경로 끝에 슬래시 `/`)를 자동 교정하지 않고 규격 위반으로 정직하게 에러를 내는지 검증합니다.
    #[test]
    fn rejects_empty_segments_instead_of_normalizing_them() {
        assert_eq!(
            CanonicalPath::parse("projects//note.md"),
            Err(PathError::EmptySegment)
        );
        assert_eq!(
            CanonicalPath::parse("projects/note.md/"),
            Err(PathError::EmptySegment)
        );
    }

    /// 윈도우 전용 경로 구분자인 역슬래시(`\`) 입력을 원천 차단하는지 검증합니다.
    #[test]
    fn rejects_windows_separators() {
        assert_eq!(
            CanonicalPath::parse("projects\\note.md"),
            Err(PathError::WindowsSeparator)
        );
    }

    /// 마침표로 시작하는 숨김 속성의 세그먼트(`.private`)를 적발해 비정상 경로로 분류하는지 검증합니다.
    #[test]
    fn rejects_hidden_segments() {
        assert_eq!(
            CanonicalPath::parse("projects/.private/note.md"),
            Err(PathError::HiddenSegment(".private".to_owned()))
        );
    }

    /// URL 디코딩을 거친 후 남아 있는 퍼센트 기호 `%` 등 더블 인코딩 우회 공격에 쓰일 수 있는 어휘적 취약점을 거부하는지 테스트합니다.
    #[test]
    fn rejects_encoded_traversal_left_after_one_decode() {
        assert_eq!(
            CanonicalPath::parse("%2e%2e/secret.md"),
            Err(PathError::AmbiguousPercentEncoding)
        );
        assert_eq!(
            CanonicalPath::parse("projects%2fnote.md"),
            Err(PathError::AmbiguousPercentEncoding)
        );
    }

    /// 시스템을 파괴하거나 오동작을 초래하는 C 스타일의 NUL 문자(`\0`) 및 개행(`\n`) 등의 제어 문자를 확실하게 차단하는지 테스트합니다.
    #[test]
    fn rejects_nul_and_control_characters() {
        assert_eq!(
            CanonicalPath::parse("projects/note\0.md"),
            Err(PathError::NulCharacter)
        );
        assert_eq!(
            CanonicalPath::parse("projects/note\n.md"),
            Err(PathError::ControlCharacter)
        );
    }

    /// 전체 경로의 한계 규격(1024바이트) 및 단일 세그먼트의 길이 한계 규격(255바이트) 한도를 초과할 때 정상적으로 에러를 내뿜는지 테스트합니다.
    #[test]
    fn enforces_utf8_byte_length_limits() {
        let oversized_segment = "a".repeat(MAX_SEGMENT_BYTES + 1);
        assert_eq!(
            CanonicalPath::parse(&oversized_segment),
            Err(PathError::SegmentTooLong {
                actual: MAX_SEGMENT_BYTES + 1,
                maximum: MAX_SEGMENT_BYTES,
            })
        );

        let segment = "a".repeat(200);
        let oversized_path = (0..6)
            .map(|_| segment.as_str())
            .collect::<Vec<_>>()
            .join("/");
        assert!(oversized_path.len() > MAX_PATH_BYTES);
        assert_eq!(
            CanonicalPath::parse(&oversized_path),
            Err(PathError::PathTooLong {
                actual: oversized_path.len(),
                maximum: MAX_PATH_BYTES,
            })
        );
    }

    /// 마크다운 전용 파일인 `MarkdownPath`를 검증할 때 반드시 소문자 `.md`로 완결되는 파일 경로만 승인하는지 검증합니다.
    #[test]
    fn markdown_path_requires_lowercase_md_extension() {
        assert_eq!(
            MarkdownPath::parse("projects/readme.txt"),
            Err(PathError::MarkdownExtensionRequired)
        );
        assert_eq!(
            MarkdownPath::parse("projects/README.MD"),
            Err(PathError::MarkdownExtensionRequired)
        );
        assert!(MarkdownPath::parse("projects/README.md").is_ok());
    }
}
