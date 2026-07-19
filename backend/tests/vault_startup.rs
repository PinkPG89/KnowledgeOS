use tempfile::TempDir;

use knowledgeos_backend::{build_router, config::AppConfig, infrastructure::vault::VaultError};

/// 존재하지 않는 디렉터리 경로를 지식 저장소 루트(Vault Root)로 설정하고 실행했을 때,
/// 웹 서버가 초기화 과정에서 부팅을 즉각 거부(Fail-Fast)하여 실행 오류를 조기에 예방하는지 검증합니다.
///
/// ## 페일 패스트(Fail-Fast) 설계
/// 서버가 기동하여 클라이언트 요청을 수신하기 시작한 후 디스크 오류를 발견하는 것이 아니라,
/// 최초 애플리케이션 빌딩 단계(`build_router`)에서 의존 설정들의 유효성을 완전하게 판독해 냄으로써
/// 잘못 세팅된 인프라 구성을 사전에 바로 차단해 운영 복잡성을 낮추어 줍니다.
#[test]
fn missing_active_vault_prevents_application_startup() {
    // 1. 임시 디렉터리를 가상 격리용으로 생성합니다.
    let directory = TempDir::new().expect("temporary directory should be created");
    // 2. 해당 디렉터리 하위에 존재하지 않는 경로 "missing-vault"를 목표 경로로 임시 정의합니다.
    let missing = directory.path().join("missing-vault");

    // 3. 존재하지 않는 경로를 주입하여 라우터 빌드 동작을 수행합니다.
    let result = build_router(AppConfig::for_test(&missing));

    // 4. 빌드 결과가 실패(`Err`)이며 구체적 실패 사유가 `VaultError::RootNotFound`로 전달되는지 확인 단언합니다.
    assert!(matches!(
        result,
        Err(VaultError::RootNotFound(path)) if path == missing
    ));
}
