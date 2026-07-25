use std::fs;

use tempfile::TempDir;

use knowledgeos_backend::{config::AppConfig, state::AppState};

#[test]
fn application_starts_in_degraded_mode_when_index_state_is_unavailable() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let vault = directory.path().join("vault");
    fs::create_dir(&vault).expect("Vault should be created");
    let state_file = directory.path().join("state-file");
    fs::write(&state_file, "not a directory").expect("state file fixture should be written");
    let mut config = AppConfig::for_test(&vault);
    config.state_root = state_file;

    let state = AppState::initialize(config)
        .expect("index failure must not prevent Markdown application startup");

    assert!(state.search_index.is_none());
}

#[test]
fn application_exposes_an_initialized_search_index_when_state_is_available() {
    let directory = TempDir::new().expect("temporary directory should be created");
    let vault = directory.path().join("vault");
    let state_root = directory.path().join("state");
    fs::create_dir(&vault).expect("Vault should be created");
    let mut config = AppConfig::for_test(&vault);
    config.state_root = state_root;

    let state = AppState::initialize(config).expect("application state should initialize");

    assert_eq!(
        state
            .search_index
            .expect("search index should be available")
            .status()
            .expect("index status should be readable")
            .schema_version,
        1
    );
}
