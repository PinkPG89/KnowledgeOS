use crate::{
    config::AppConfig,
    infrastructure::vault::{VaultError, VaultRoot},
};

/// 모든 HTTP handler가 공유하는 검증 완료 애플리케이션 상태입니다.
#[derive(Clone, Debug)]
pub struct AppState {
    pub config: AppConfig,
    pub vault: VaultRoot,
}

impl AppState {
    /// 설정을 검증하고 단일 활성 Vault를 엽니다.
    ///
    /// # Errors
    ///
    /// 설정된 Vault root를 사용할 수 없으면 [`VaultError`]를 반환합니다.
    pub fn initialize(config: AppConfig) -> Result<Self, VaultError> {
        let vault = VaultRoot::open(&config.knowledge_root)?;
        tracing::info!(
            configured_path = %vault.configured_path().display(),
            canonical_path = %vault.canonical_path().display(),
            "active Vault initialized"
        );
        Ok(Self { config, vault })
    }
}
