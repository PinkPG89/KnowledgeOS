//! `KnowledgeOS` backend library.
//!
//! # `KnowledgeOS` 백엔드 라이브러리 모듈
//!
//! ## 구조 분리 설계의 장점
//! 실행 파일(`main.rs`)과 애플리케이션의 핵심 로직을 조립하는 코드(`lib.rs`)를 분리하면,
//! 실제 네트워크의 TCP 포트를 점유하여 리스닝하지 않고도 HTTP 라우터 자체를 가상으로 테스트할 수 있습니다.
//! Rust에서는 단일 프로젝트 안에서 바이너리 크레이트(binary crate, `main.rs`)와
//! 라이브러리 크레이트(library crate, `lib.rs`)를 함께 개발하는 방식이 권장되며,
//! 이는 웹 서비스의 테스트 편의성과 모듈화 가능성을 크게 향상시킵니다.

// 하위 모듈들을 외부로 공개(pub)하여 라이브러리 사용자 또는 main.rs가 접근할 수 있도록 선언합니다.
pub mod api;
pub mod config;
pub mod domain;
pub mod error;
pub mod infrastructure;
pub mod state;

use axum::Router;

// 크레이트 루트(crate root)로부터 필요한 하위 모듈과 설정 구조체를 가져옵니다.
use crate::{config::AppConfig, infrastructure::vault::VaultError, state::AppState};

/// 웹 서버에서 사용할 모든 HTTP 경로(Route)와 전역 공유 상태(State)를 조립하여 반환합니다.
///
/// * `config`: 애플리케이션의 설정 정보를 담고 있는 `AppConfig` 구조체
///
/// 이 함수가 네트워크 포트 바인딩 없이 Axum의 `Router` 인스턴스만 반환하므로,
/// 실제 서비스를 구동하는 프로덕션 서버와 가상 요청을 보내 검증하는 통합 테스트 코드가
/// 정확히 동일한 라우터 및 상태 설정을 공유하여 일관된 동작을 보증할 수 있습니다.
///
/// # Errors
///
/// 설정된 단일 활성 Vault를 초기화할 수 없으면 [`VaultError`]를 반환합니다.
pub fn build_router(config: AppConfig) -> Result<Router, VaultError> {
    let state = AppState::initialize(config)?;
    Ok(Router::new()
        // `/api` 하위 경로로 들어오는 요청을 `health::router()`가 정의한 라우터 모듈로 전달(nesting)합니다.
        .nest("/api", api::router())
        // 모든 handler가 검증된 Vault와 설정을 공유하도록 `AppState`를 주입합니다.
        .with_state(state))
}
