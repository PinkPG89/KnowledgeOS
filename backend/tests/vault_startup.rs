use tempfile::TempDir;

use knowledgeos_backend::{build_router, config::AppConfig, infrastructure::vault::VaultError};

/// 잘못된 Vault 설정에서는 HTTP Router조차 생성되지 않는 fail-fast 계약을 검증합니다.
#[test]
fn missing_active_vault_prevents_application_startup() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let missing = directory.path().join("missing-vault");

    let result = build_router(AppConfig::for_test(&missing));

    assert!(matches!(
        result,
        Err(VaultError::RootNotFound(path)) if path == missing
    ));
}
