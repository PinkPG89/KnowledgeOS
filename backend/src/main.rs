use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

use knowledgeos_backend::{build_router, config::AppConfig, error::AppError};

/// `KnowledgeOS` 백엔드 서버의 메인 진입점(Entry Point)입니다.
///
/// `#[tokio::main]` 매크로는 Rust의 표준 비동기 런타임인 `tokio`를 활성화하여
/// 비동기 함수인 `async fn main()`을 실행할 수 있도록 변환해 줍니다.
/// 이 함수는 에러 발생 시 `Result` 타입의 에러(`AppError`)를 반환합니다.
#[tokio::main]
async fn main() -> Result<(), AppError> {
    // 1. 환경 변수로부터 서버 설정을 읽어옵니다. (기본값 설정 제공)
    // `?` 연산자는 에러 발생 시 즉시 에러를 반환(early return)하는 Rust의 문법입니다.
    let config = AppConfig::from_env()?;

    // 2. 디버깅 및 분석용 로깅(Tracing) 시스템을 초기화합니다.
    init_tracing(&config.log_filter)?;

    // `SocketAddr`는 Copy 타입이므로 설정 전체의 소유권을 옮기기 전에 값만 복사합니다.
    let bind_address = config.bind_address;

    // 3. socket을 열기 전에 단일 활성 Vault를 검증합니다.
    let application = build_router(config)?;

    // 4. 지정된 IP 주소와 포트(예: 127.0.0.1:3000)로 TCP 리스너를 결합(Bind)하여 대기합니다.
    // `.await`는 비동기 작업이 끝날 때까지 대기함을 의미합니다.
    let listener = TcpListener::bind(bind_address).await?;

    // 5. 실제로 바인딩된 로컬 네트워크 주소 정보(IP 및 포트)를 가져옵니다.
    let local_address = listener.local_addr()?;

    // 6. 서버가 정상적으로 시작되었음을 로그(INFO 레벨)로 남깁니다.
    // `%local_address`는 변수의 포맷팅 표현 방식입니다.
    info!(address = %local_address, "KnowledgeOS backend started");

    // 7. Axum 웹 프레임워크를 사용해 HTTP 서버를 구동(Serve)합니다.
    // `.with_graceful_shutdown(...)`은 종료 신호가 들어왔을 때
    // 처리 중이던 연결을 안전하게 마친 후 종료(우아한 종료)할 수 있게 해줍니다.
    axum::serve(listener, application)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // 에러 없이 정상 실행 및 종료되었음을 반환합니다.
    Ok(())
}

/// `tracing` 라이브러리를 통해 출력되는 로그 이벤트를 JSON 포맷으로 표준 출력(stdout)에 기록합니다.
///
/// * `log_filter`: 로그 출력 수준을 제어하는 필터 정보 (예: "info", "debug")
///
/// 이 JSON 형식의 로그는 향후 컨테이너 환경(Docker), 중앙 로그 수집기(Loki),
/// 혹은 분산 추적 시스템(OpenTelemetry) 등으로 전달 및 수집하기 용이합니다.
fn init_tracing(log_filter: &str) -> Result<(), AppError> {
    // 로그 수준을 판별하기 위한 필터 규칙을 파싱합니다.
    let filter = EnvFilter::try_new(log_filter)?;

    // 콘솔(stdout) 포맷을 설정합니다.
    // `.json()` 체이닝을 통해 사람이 읽기 쉬운 텍스트가 아닌 JSON 포맷의 구조화된 로그를 생성합니다.
    // `.try_init()`을 통해 로깅 시스템을 가동하며, 실패 시 에러를 AppError 형태로 변환해 반환합니다.
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .try_init()
        .map_err(|error| AppError::LoggingInitialization(error.to_string()))?;

    Ok(())
}

/// 프로그램 종료 신호(SIGINT 또는 SIGTERM)가 감지될 때까지 대기하는 비동기 함수입니다.
///
/// 이 함수는 두 가지 종료 신호를 수신 대기합니다.
/// 1. `Ctrl+C` (SIGINT): 터미널에서 사용자가 중단할 때 주로 발생
/// 2. `SIGTERM`: 서비스 오케스트레이션 도구(예: Docker, Kubernetes)가 컨테이너를 정상 종료시킬 때 주로 발생
async fn shutdown_signal() {
    // Ctrl+C 신호를 대기하는 비동기 블록
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "failed to install Ctrl+C handler");
        }
    };

    // Unix/Linux 계열 환경에서 SIGTERM 신호를 대기하는 비동기 블록
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};

        // 시스템 종료 신호 핸들러를 설치합니다.
        match signal(SignalKind::terminate()) {
            Ok(mut signal) => {
                // 실제로 종료 신호가 들어올 때까지 이곳에서 실행이 잠시 멈추고 대기합니다.
                signal.recv().await;
            }
            Err(error) => tracing::error!(%error, "failed to install SIGTERM handler"),
        }
    };

    // Windows 등 Unix 계열이 아닌 운영체제인 경우에는 SIGTERM 신호 감지를 지원하지 않으므로
    // 무한히 대기하는 더미(Pending) 비동기 타스크를 할당해 둡니다.
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    // `tokio::select!` 매크로는 여러 개의 비동기 작업 중 "가장 먼저 완료되는 하나"를 선택합니다.
    // 즉 Ctrl+C 신호가 오거나, SIGTERM 신호가 오면 즉시 대기 상태가 해제됩니다.
    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }

    // 신호가 감지되면 종료 로그를 찍고 함수를 마쳐, 호출한 메인 함수가 우아하게 종료되도록 합니다.
    info!("shutdown signal received");
}
