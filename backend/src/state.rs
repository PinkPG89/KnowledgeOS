use crate::{
    config::AppConfig,
    infrastructure::{
        markdown::MarkdownReader,
        markdown_writer::MarkdownWriter,
        vault::{VaultError, VaultRoot},
    },
};

/// 모든 HTTP API 라우터 핸들러가 안전하게 공유하여 접근하는 검증 완료된 전역 애플리케이션 상태(AppState) 구조체입니다.
///
/// ## 웹 프레임워크에서의 공유 상태(Shared State)
/// 비동기 웹 서버인 Axum은 멀티스레드 환경에서 동작하므로 여러 사용자의 HTTP 요청이 동시에 유입됩니다.
/// 이때 각 요청을 처리하는 독립된 작업 스레드들이 서버의 설정(`config`)과 안전하게 정규화된 지식 저장소 루트(`vault`)를
/// 동시적으로 읽고 다룰 수 있도록 이 구조체를 핸들러의 매개변수로 전달(의존성 주입)받아 사용합니다.
///
/// `#[derive(Clone, Debug)]` 매크로는 이 공유 상태 구조체가 안전하게 복제(Clone)되고 포맷 출력(Debug)될 수 있음을 증명합니다.
#[derive(Clone, Debug)]
pub struct AppState {
    /// 검증 완료된 읽기 전용 전역 설정 정보
    pub config: AppConfig,
    /// 물리적 파일 시스템의 경계와 보안 격리를 관리하는 Vault 리소스를 저장
    pub vault: VaultRoot,
    pub markdown_reader: MarkdownReader,
    /// 기존 파일을 덮어쓰지 않는 Markdown 생성 서비스
    pub markdown_writer: MarkdownWriter,
}

impl AppState {
    /// 제공받은 설정을 기반으로 검증 및 초기화 단계를 거쳐 단일 활성 Vault를 생성하고 `AppState` 인스턴스를 확보합니다.
    ///
    /// ## 초기화 프로세스 흐름
    /// 1. `config.knowledge_root`에 명시된 지식 원본 마크다운 저장소 경로를 확인합니다.
    /// 2. `VaultRoot::open`을 실행하여 실제 물리 폴더가 맞는지, 읽기 권한이 올바른지 등 철저한 가드레일 유효성 검증을 거칩니다.
    /// 3. 검증 성공 시 구조화된 `AppState` 객체를 안전하게 조립해 돌려줍니다.
    ///
    /// # Errors
    ///
    /// 만약 설정에 적힌 경로를 사용할 수 없거나(부재, 파일 속성 불일치 등) 검증에 실패하면,
    /// 하위 계층에서 발생한 구체적인 에러인 [`VaultError`]를 호출자에게 상향 반환합니다.
    pub fn initialize(config: AppConfig) -> Result<Self, VaultError> {
        // VaultRoot를 개방하여 물리 경로를 획득하고 내부 검증을 진행합니다.
        let vault = VaultRoot::open(&config.knowledge_root)?;
        let markdown_reader = MarkdownReader::new(vault.clone(), config.max_markdown_bytes);
        let markdown_writer = MarkdownWriter::new(vault.clone(), config.max_markdown_bytes);

        // 서버 기동 로그를 Tracing 시스템에 기록합니다.
        // `%` 접두사는 해당 인스턴스의 Display 포맷을 사용해 구조화된 로깅 필드로 치환 출력하라는 지시어입니다.
        // `.display()`는 Rust의 `Path` 타입이 유니코드가 깨진 잘못된 바이트 배열일 수도 있기 때문에,
        // 이를 안전하게 출력 가능한 문자열로 임시 변환해서 보여주는 안전 헬퍼 기능입니다.
        tracing::info!(
            configured_path = %vault.configured_path().display(),
            canonical_path = %vault.canonical_path().display(),
            "active Vault initialized"
        );

        Ok(Self {
            config,
            vault,
            markdown_reader,
            markdown_writer,
        })
    }
}
