use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::FormatItem, macros::format_description};

use crate::{
    domain::{document::MarkdownDocument, path::MarkdownPath},
    state::AppState,
};

use super::error::ApiError;

/// RFC3339 표준 규격에 밀리초 및 UTC 타임존('Z') 정보를 정밀 기입하도록 설계된 날짜 문자열 포맷팅 정보 상수입니다.
const RFC3339_MILLISECONDS: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");

/// 파일 조작 및 조회 API의 라우터 매핑 설정을 빌드합니다.
pub fn router() -> Router<AppState> {
    // `{*path}`는 Axum의 와일드카드(Wildcard) 경로 매치 규칙입니다.
    // 슬래시('/') 기호가 여러 번 중첩되어 유입되는 하위 폴더 트리 구조(예: projects/knowledgeos/readme.md)의
    // 파일 경로 전체를 가로채어 하나의 문자열로 일관되게 수용할 수 있게 돕습니다.
    Router::new().route("/{*path}", get(read_file).put(update_file))
}

#[derive(Debug, Deserialize)]
struct CreateFileRequest {
    path: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct UpdateFileRequest {
    content: String,
    base_hash: String,
}

pub(crate) async fn create_file(
    State(state): State<AppState>,
    body: Body,
) -> Result<(StatusCode, Json<FileResponse>), ApiError> {
    let maximum = state.config.max_markdown_bytes;
    let request: CreateFileRequest = parse_json_body(body, maximum).await?;
    let path = MarkdownPath::parse(&request.path)?;
    let writer = state.markdown_writer.clone();
    let document = tokio::task::spawn_blocking(move || writer.create(&path, request.content))
        .await
        .map_err(|error| ApiError::task_failure(&error))?
        .map_err(ApiError::from_write)?;

    Ok((StatusCode::CREATED, Json(FileResponse::try_from(document)?)))
}

async fn update_file(
    State(state): State<AppState>,
    Path(raw_path): Path<String>,
    body: Body,
) -> Result<Json<FileResponse>, ApiError> {
    let maximum = state.config.max_markdown_bytes;
    let request: UpdateFileRequest = parse_json_body(body, maximum).await?;
    let path = MarkdownPath::parse(&raw_path)?;
    if !is_valid_sha256_hash(&request.base_hash) {
        return Err(ApiError::invalid_base_hash());
    }
    let writer = state.markdown_writer.clone();
    let document = tokio::task::spawn_blocking(move || {
        writer.update(&path, request.content, &request.base_hash)
    })
    .await
    .map_err(|error| ApiError::task_failure(&error))?
    .map_err(ApiError::from_update)?;

    Ok(Json(FileResponse::try_from(document)?))
}

async fn parse_json_body<T: DeserializeOwned>(body: Body, maximum: u64) -> Result<T, ApiError> {
    let body_limit = usize::try_from(maximum)
        .unwrap_or(usize::MAX)
        .saturating_mul(6)
        .saturating_add(64 * 1024);
    let bytes = to_bytes(body, body_limit)
        .await
        .map_err(|error| ApiError::request_too_large(&error, maximum))?;
    serde_json::from_slice(&bytes).map_err(|error| ApiError::invalid_request(&error))
}

fn is_valid_sha256_hash(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

/// 클라이언트가 보낸 마크다운 파일 경로를 읽어 JSON 포맷으로 돌려주는 비동기 HTTP GET 핸들러입니다.
async fn read_file(
    // Axum이 보관 중인 읽기 전용 상태 구조체를 추출(Extract)합니다.
    State(state): State<AppState>,
    // 와일드카드 경로 문자열을 획득합니다.
    Path(raw_path): Path<String>,
) -> Result<Json<FileResponse>, ApiError> {
    // 1. 도메인 유효성 검사 규칙을 사용해 안전하고 규칙에 부합하는 마크다운 파일 경로인지 파싱합니다.
    let path = MarkdownPath::parse(&raw_path)?;
    let reader = state.markdown_reader.clone();

    // 2. 디스크에서 마크다운 파일을 정적 스냅샷으로 판독합니다.
    //
    // ## 비동기 입출력 스레드 격리 (`tokio::task::spawn_blocking`)
    // 운영체제(OS)의 디스크 파일 조작은 일반적으로 동기식(blocking) 시스템 콜로 구동됩니다.
    // 만약 비동기 이벤트 루프 스레드 상에서 디스크 I/O를 직접 돌리면 해당 스레드가 멈추어(Block) 웹 서버 성능이 급전직하합니다.
    // 이를 피하고자, 블로킹 전용 독립 스레드 풀에서 동기식 I/O를 실행하도록 격리해 줌으로써 고성능 비동기 구동을 보호합니다.
    let document = tokio::task::spawn_blocking(move || reader.read(&path))
        .await
        // 스레드 조인 실패(JoinError) 대응 매핑
        .map_err(|error| ApiError::task_failure(&error))?
        // 판독기 내부 유효성/보안 필터 에러 매핑
        .map_err(ApiError::from_read)?;

    // 3. 읽어 들인 도메인 문서 엔티티를 프론트엔드가 요구하는 JSON 응답 데이터 규격으로 최종 전환하여 반환합니다.
    Ok(Json(FileResponse::try_from(document)?))
}

/// 성공적으로 파일을 판독했을 때 클라이언트 브라우저로 흘려보내는 JSON 응답 규격입니다.
#[derive(Debug, Serialize)]
pub(crate) struct FileResponse {
    /// 가공된 상대 경로 문자열
    path: String,
    /// 안전하게 로드 완료된 마크다운 텍스트 원문
    content: String,
    /// 문서 위변조 및 동시 변경 검증용 SHA256 체크섬 해시 코드
    hash: String,
    /// 문서 크기 (바이트 수)
    size: u64,
    /// ISO8601/RFC3339 밀리초 규격을 준수하는 수정 시간 텍스트
    modified_at: String,
}

/// 도메인 엔티티(`MarkdownDocument`)에서 웹 전송 규격(`ReadFileResponse`)으로
/// 안전하게 타입 형변환을 시도할 수 있도록 `TryFrom` 트레이트를 정의합니다.
impl TryFrom<MarkdownDocument> for FileResponse {
    type Error = ApiError;

    fn try_from(document: MarkdownDocument) -> Result<Self, Self::Error> {
        // 도메인의 원시 시간 정보(`SystemTime`)를 표준 UTC 시간대로 변환한 뒤 포맷 규칙에 맞춰 문자열화합니다.
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
