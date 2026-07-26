use crate::{
    domain::{document::MarkdownDocument, path::MarkdownPath},
    infrastructure::{
        index_sync::SearchIndexSynchronizer,
        markdown_writer::{MarkdownUpdateError, MarkdownWriteError, MarkdownWriter},
    },
};

/// 원본 Markdown 쓰기 성공 후 검색 projection을 best-effort로 갱신합니다.
///
/// 검색 index는 재생성 가능한 cache이므로 갱신 실패가 이미 성공한 원본 파일 쓰기를
/// 실패로 바꾸지 않습니다. 실패한 projection은 다음 reconciliation에서 복구합니다.
#[derive(Clone, Debug)]
pub struct IndexedMarkdownWriter {
    writer: MarkdownWriter,
    index_sync: Option<SearchIndexSynchronizer>,
}

impl IndexedMarkdownWriter {
    #[must_use]
    pub const fn new(writer: MarkdownWriter, index_sync: Option<SearchIndexSynchronizer>) -> Self {
        Self { writer, index_sync }
    }

    /// 새 Markdown 파일을 배타적으로 생성하고 검색 projection을 갱신합니다.
    ///
    /// # Errors
    ///
    /// 원본 Markdown 생성이 실패하면 [`MarkdownWriteError`]를 반환합니다.
    pub fn create(
        &self,
        path: &MarkdownPath,
        content: String,
    ) -> Result<MarkdownDocument, MarkdownWriteError> {
        let document = self.writer.create(path, content)?;
        self.sync_best_effort(&document);
        Ok(document)
    }

    /// 기존 Markdown 파일을 원자적으로 교체하고 검색 projection을 갱신합니다.
    ///
    /// # Errors
    ///
    /// 원본 Markdown 갱신이 실패하면 [`MarkdownUpdateError`]를 반환합니다.
    pub fn update(
        &self,
        path: &MarkdownPath,
        content: String,
        base_hash: &str,
    ) -> Result<MarkdownDocument, MarkdownUpdateError> {
        let document = self.writer.update(path, content, base_hash)?;
        self.sync_best_effort(&document);
        Ok(document)
    }

    fn sync_best_effort(&self, document: &MarkdownDocument) {
        let Some(index_sync) = &self.index_sync else {
            return;
        };
        if let Err(error) = index_sync.upsert_document(document) {
            tracing::error!(
                %error,
                path = %document.path,
                "search projection update failed; source Markdown write remains committed"
            );
        }
    }
}
