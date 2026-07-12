use std::{env, net::SocketAddr};

use crate::error::AppError;

/// 애플리케이션 시작 시 최초에 한 번 로드하고 검증을 거친 전역 설정 정보 구조체입니다.
///
/// ## 설정값 형식 변환(Type Validation)의 중요성
/// 환경 변수(String)를 매 HTTP 요청마다 매번 읽지 않고, 앱 시작 단계에서 타입 검증을 거쳐
/// 구조체(`SocketAddr` 등 실제 필요한 타입) 형태로 메모리에 보관(Typed Value)합니다.
/// 이를 통해 잘못 구성된 인프라 및 환경 변수 설정을 실제 클라이언트 요청이 들어왔을 때가 아니라,
/// 서버 프로세스가 뜨는 시점(Bootstrap Phase)에 즉시 감지하여 Fail-Fast하게 처리할 수 있습니다.
///
/// `#[derive(Clone, Debug)]`는 컴파일러가 이 구조체에 대한 `Clone`(복제 가능) 기능과
/// `Debug`(디버깅을 위한 포맷 출력) 트레이트 구현체를 자동으로 작성하도록 유도하는 매크로입니다.
#[derive(Clone, Debug)]
pub struct AppConfig {
    /// 서버가 요청을 받기 위해 바인딩할 네트워크 소켓 주소 (IP + Port)
    pub bind_address: SocketAddr,
    /// 마크다운 문서들이 보관되는 로컬 파일 시스템 상의 루트 디렉터리 경로
    pub knowledge_root: String,
    /// 로깅 시스템(`tracing`)에서 적용할 로그 범위와 필터 옵션
    pub log_filter: String,
}

impl AppConfig {
    // 환경 변수가 지정되지 않았을 때 기본값으로 사용될 상수를 선언합니다.

    /// 기본 바인드 주소 (로컬 호스트의 3000번 포트)
    const DEFAULT_BIND_ADDRESS: &'static str = "127.0.0.1:3000";
    /// 기본 지식 저장소 경로 (현재 작업 디렉터리 상위의 knowledge 폴더)
    const DEFAULT_KNOWLEDGE_ROOT: &'static str = "../knowledge";
    /// 기본 로깅 필터 (`knowledgeos_backend` 모듈의 로그 수준을 `info`로 제한)
    const DEFAULT_LOG_FILTER: &'static str = "knowledgeos_backend=info";

    /// `KNOWLEDGEOS_` 접두사를 가지는 시스템 환경 변수에서 설정값을 읽어들이고,
    /// 알맞은 타입으로 변환 및 검증한 뒤 `AppConfig` 인스턴스를 생성해 반환합니다.
    ///
    /// # Errors
    ///
    /// 만약 환경 변수 `KNOWLEDGEOS_BIND_ADDRESS`에 저장된 문자열이 올바른 IP 주소 및 포트 형식(예: 127.0.0.1:3000)이
    /// 아닌 경우, 파싱에 실패하여 [`AppError::InvalidSocketAddress`] 에러를 반환합니다.
    pub fn from_env() -> Result<Self, AppError> {
        // KNOWLEDGEOS_BIND_ADDRESS 환경 변수를 조회하고, 없으면 기본 주소값을 사용합니다.
        // `.parse()?`를 호출하여 String을 SocketAddr 타입으로 변환합니다.
        // 변환 실패 시 `?` 연산자에 의해 이 함수는 즉시 에러를 호출자에게 던집니다.
        let bind_address = env::var("KNOWLEDGEOS_BIND_ADDRESS")
            .unwrap_or_else(|_| Self::DEFAULT_BIND_ADDRESS.to_owned())
            .parse()?;

        // KNOWLEDGEOS_KNOWLEDGE_ROOT 환경 변수를 조회하며, 값이 없으면 기본 경로를 지정합니다.
        let knowledge_root = env::var("KNOWLEDGEOS_KNOWLEDGE_ROOT")
            .unwrap_or_else(|_| Self::DEFAULT_KNOWLEDGE_ROOT.to_owned());

        // 생성된 값을 가진 설정 구조체를 감싸서 반환(Ok)합니다.
        Ok(Self {
            bind_address,
            knowledge_root,
            // KNOWLEDGEOS_LOG 환경 변수를 조회하여 로깅 범위 필터를 구성하며, 없을 시 기본 필터를 사용합니다.
            log_filter: env::var("KNOWLEDGEOS_LOG")
                .unwrap_or_else(|_| Self::DEFAULT_LOG_FILTER.to_owned()),
        })
    }

    /// 단위 테스트나 통합 테스트에서 사용할 수 있도록 정적인 모의(Mock) 설정값을 간편하게 만듭니다.
    ///
    /// `#[must_use]`는 이 함수의 반환값을 변수에 대입하거나 사용하지 않고 무시할 때
    /// 컴파일러가 경고(Warning)를 내보내도록 하는 애트리뷰트입니다.
    #[must_use]
    pub fn for_test() -> Self {
        Self {
            // 테스트 시에는 항상 고정된 로컬 IP `127.0.0.1:3000` 주소를 주입합니다.
            bind_address: SocketAddr::from(([127, 0, 0, 1], 3000)),
            // 테스트용 지식 저장소 루트 폴더를 "knowledge"로 지정합니다.
            knowledge_root: "knowledge".to_owned(),
            // 디버그 출력 로그 필터를 설정합니다.
            log_filter: Self::DEFAULT_LOG_FILTER.to_owned(),
        }
    }
}
