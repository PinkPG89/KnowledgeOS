use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use knowledgeos_backend::{build_router, config::AppConfig};

/// 실제 네트워크 포트(예: localhost:3000)를 개방하여 리스닝하지 않고,
/// 가상의 HTTP 요청을 생성해 메모리 내에서 Axum Router를 직접 호출하여 응답을 검증합니다.
///
/// ## API 계약 테스트 (Contract Test)
/// 이 테스트는 프론트엔드 및 AI 어댑터 등의 클라이언트가 백엔드 서버에 의존할 때 기대하는
/// HTTP 상태 코드(예: 200 OK)와 반환되는 JSON 데이터 구조(Shape)를 강하게 보장(Lock-in)하기 위해 수행됩니다.
/// API의 응답 형식이 바뀌면 프론트엔드가 비정상적으로 동작할 수 있으므로, 빌드/배포 전 이를 방지하는 역할을 합니다.
///
/// `#[tokio::test]` 매크로는 이 테스트 함수가 비동기(`async`) 연산을 수행하고
/// 테스트 전용 비동기 런타임 위에서 실행되도록 표시해 줍니다.
#[tokio::test]
async fn health_endpoint_matches_public_contract() {
    // 1. 테스트 전용 설정(Mock Config)을 활용해 라우터를 빌드합니다.
    // 2. `oneshot` 메서드는 라우터를 단발성(oneshot) 서비스로 구성하여 단 하나의 요청만 처리하고 즉시 연결을 종료합니다.
    let response = build_router(AppConfig::for_test())
        .oneshot(
            // 가상의 HTTP GET /api/health 요청을 설계합니다.
            Request::builder()
                .uri("/api/health")
                .body(Body::empty()) // 요청 본문(body)은 비어 있습니다.
                .expect("test request must be valid"),
        )
        .await
        .expect("health endpoint must respond"); // 비동기로 가상 요청을 호출하여 응답을 받습니다.

    // 3. 반환된 HTTP 상태 코드가 200 OK인지 단언(Assert)합니다.
    assert_eq!(response.status(), StatusCode::OK);

    // 4. 비동기식 스트림 형태로 전달되는 응답 본문(Body) 데이터를 하나로 수집(collect)하여 바이트 배열로 변환합니다.
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body must be readable")
        .to_bytes();

    // 5. 수집된 바이트 배열을 범용 JSON 개체(`serde_json::Value`) 형태로 역직렬화(파싱)합니다.
    let payload: Value = serde_json::from_slice(&body).expect("response must contain valid JSON");

    // 6. 파싱된 JSON 구조 및 결과값이 우리가 약속한 공개 응답 데이터와 완벽하게 일치하는지 최종 검증합니다.
    // `json!` 매크로는 Rust 코드 내에 JSON 구조를 직관적으로 바로 하드코딩해서 Value 타입으로 생성할 수 있게 돕습니다.
    assert_eq!(
        payload,
        json!({
            "status": "ok",
            "version": "0.1.0",
            "knowledge_root": "knowledge"
        })
    );
}
