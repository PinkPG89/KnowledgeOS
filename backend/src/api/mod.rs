//! # HTTP API Adapter 모듈
//!
//! 이 모듈은 외부 클라이언트의 HTTP 요청을 직접 접수하고 적절한 응답으로 변환해주는 '어댑터(Adapter)' 역할을 담당합니다.
//!
//! ## 관심사의 분리와 아키텍처 원칙 (Clean Architecture)
//! * **HTTP 요청/응답 변환 전담**: `api` 모듈은 들어오는 HTTP Request 파싱 및 JSON 직렬화/역직렬화와 같은 웹 통신 관련 처리에 집중합니다.
//! * **비즈니스 로직 독립성**: 파일 시스템 처리 규칙이나 핵심 유스케이스(Use Case) 로직은 이 `api` 모듈의 외부에 격리시켜 설계합니다.
//!   이러한 격리는 비즈니스 로직이 웹 프레임워크인 `Axum`에 직접 의존하지 않게 만들며, 향후 gRPC, CLI 등 다른 프로토콜로 쉽게 확장할 수 있도록 도와줍니다.

// 헬스체크 관련 API 엔드포인트 핸들러와 라우터를 포함하는 하위 모듈을 공개합니다.
use axum::{Router, routing::post};

use crate::state::AppState;

pub mod error;
pub mod files;
pub mod health;
pub mod tree;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .merge(tree::router())
        .route("/files", post(files::create_file))
        .nest("/files", files::router())
}
