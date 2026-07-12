use std::{io, net::AddrParseError};

use thiserror::Error;

use crate::infrastructure::vault::VaultError;

/// 프로세스 시작 및 서버 실행 중에 발생할 수 있는 애플리케이션 최상위 오류들을 모아놓은 열거형(Enum)입니다.
///
/// ## Rust의 에러 핸들링과 `thiserror` 라이브러리
/// Rust에서는 에러를 반환할 때 안전하고 구조적인 에러 처리를 위해 열거형을 널리 사용합니다.
/// `thiserror` 크레이트는 이 열거형 에러 타입을 편리하게 설계하도록 도와주는 라이브러리입니다.
/// - `#[derive(Debug, Error)]`는 디버그 출력과 표준 `std::error::Error` 트레이트를 자동으로 구현해 줍니다.
/// - `#[error("...")]` 매크로는 각 에러 변형이 화면이나 로그에 출력될 때의 사용자 정의 에러 메시지 형식을 정의합니다.
/// - `#[from]` 어노테이션은 외부 라이브러리 등에서 발생한 하위 에러를 `AppError`로 자동 변환할 수 있도록 해줍니다.
///   (예: `io::Error` 발생 시 `AppError::Io(error)` 형태로 `?` 연산자에 의해 자동 래핑되어 호출됨)
#[derive(Debug, Error)]
pub enum AppError {
    /// 바인딩할 IP 및 포트(SocketAddress) 문자열의 문법이 올바르지 않을 때 발생하는 에러입니다.
    #[error("invalid socket address: {0}")]
    InvalidSocketAddress(#[from] AddrParseError),

    /// 파일 읽기/쓰기, 네트워크 연결 등의 입출력(I/O) 과정에서 문제가 발생했을 때의 에러입니다.
    #[error("I/O operation failed: {0}")]
    Io(#[from] io::Error),

    /// 로그 필터 설정 문자열(예: debug, info 등)을 파싱하는 도중 규칙에 맞지 않아 발생한 에러입니다.
    #[error("invalid log filter: {0}")]
    InvalidLogFilter(#[from] tracing_subscriber::filter::ParseError),

    /// 로그 프레임워크(`tracing`)를 초기화(가동)하는 과정 자체에서 실패했을 때의 에러입니다.
    #[error("failed to initialize logging: {0}")]
    LoggingInitialization(String),

    /// 설정된 단일 활성 Vault를 초기화할 수 없을 때 발생합니다.
    #[error(transparent)]
    Vault(#[from] VaultError),
}
