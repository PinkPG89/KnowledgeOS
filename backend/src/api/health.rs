use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;

use crate::config::AppConfig;

/// `/api/health` 엔드포인트가 외부 클라이언트에 반환할 공공 응답 스키마(공개 계약, Public Contract)입니다.
///
/// ## 직렬화(Serialization)와 Serde 라이브러리
/// `#[derive(Debug, Serialize)]`는 Rust 구조체(struct)를 웹 브라우저나 클라이언트가 읽을 수 있는
/// JSON 텍스트 포맷으로 자동 변환해주는 직렬화 코드를 컴파일 타임에 생성해 줍니다.
/// 이를 통해 개발자가 직접 문자열 포맷팅을 처리하지 않아도, 안전하고 빠르게 응답 데이터를 JSON으로 바꿀 수 있습니다.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// 서버가 정상 가동 중인지 나타내는 상태 문자열 (예: "ok")
    status: &'static str,
    /// 빌드 시점의 애플리케이션 버전 정보
    version: &'static str,
    /// 서버가 접근하고 있는 로컬 마크다운 지식 저장소 루트 경로
    knowledge_root: String,
}

/// `health` 모듈의 라우터 및 상태 규격을 정의하는 생성 함수입니다.
///
/// 이 함수는 `AppConfig` 타입을 상태로 주입받는 Axum의 `Router` 인스턴스를 생성해 리턴합니다.
/// `get(health)`를 등록함으로써 `/health` 경로로 유입되는 HTTP GET 요청이 `health` 비동기 함수로 연결됩니다.
pub fn router() -> Router<AppConfig> {
    Router::new().route("/health", get(health))
}

/// 서버의 정상 구동 상태를 나타내는 비동기 핸들러 함수입니다.
///
/// ## Axum의 상태 추출기(`State` Extractor)
/// 매개변수 중 `State(config): State<AppConfig>`는 Axum이 관리하는 전역/로컬 읽기 전용 상태로부터
/// 자동으로 `AppConfig` 인스턴스를 추출(Extract)하여 주입해 줍니다.
///
/// * 성능 참고: `AppConfig`는 복제(Clone) 연산 비용이 매우 저렴한 필드들만 담고 있으므로
///   매 요청마다 소유권을 가져오거나 클론해도 성능 손실이 미미합니다.
///   만약 무거운 캐시 디비 커넥션 풀 등 큰 데이터를 상태로 공유해야 한다면,
///   참조 카운팅 포인터인 `Arc<T>`를 사용하여 메모리를 공유하는 방식으로 개선할 수 있습니다.
async fn health(State(config): State<AppConfig>) -> Json<HealthResponse> {
    // Json(HealthResponse)으로 래핑함으로써, Axum이 HTTP 응답 헤더의 Content-Type을
    // 자동으로 "application/json"으로 설정하고 응답 데이터를 직렬화해 전달하도록 지시합니다.
    Json(HealthResponse {
        status: "ok",
        // `env!("CARGO_PKG_VERSION")`은 Rust 컴파일러가 빌드를 실행하는 시점에
        // 해당 프로젝트 Cargo.toml 내의 version 항목을 가져와 정적 문자열로 코드에 하드코딩해 주는 매크로입니다.
        version: env!("CARGO_PKG_VERSION"),
        knowledge_root: config.knowledge_root,
    })
}
