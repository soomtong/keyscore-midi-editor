//! SMF·계이름 텍스트 가져오기·내보내기(S2-T7c, IO-2, IO-3, 결정 `2026-10-22-s2-t7c-import-export`).
//!
//! - SMF 가져오기: 백그라운드에서 읽고 바꿔 새 프로젝트 창에 둔다(손대지 않은 빈 창이면 바꾼다).
//! - 계이름 텍스트 가져오기: 지금 문서에 undo 하나로 넣는다. 위치는 재생 위치(S3에서 커서로 바꾼다).
//! - 내보내기 둘: 문서를 바꾸지 않고 쓰고, 잃은 정보를 상태줄에 요약한다.

use keyscore_core::{ModelError, Tick};
use keyscore_text::Parsed;

use crate::document::ProjectDocument;

/// 계이름 텍스트 가져오기의 결과.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextImportSummary {
    /// 넣은 블록(클립) 수.
    pub blocks: usize,
    /// 새로 만든 트랙 수.
    pub new_tracks: usize,
    /// 넣은 노트 수.
    pub notes: usize,
}

/// 읽은 텍스트를 문서에 undo 하나로 넣는다. `@track` 이름이 같은 트랙이 있고 그 트랙의 클립과 겹치지 않으면
/// 그 트랙에, 아니면 새 트랙(겹치면 「이름 2」처럼 비어 있는 이름)에 둔다. 클립은 `at`에서 시작한다.
/// 헤더는 반영하지 않는다.
///
/// # Errors
///
/// 모델 불변식을 어기면(틱 넘침 등) [`ModelError`]. 이때 문서는 바뀌지 않는다.
pub fn import_text_into(
    _document: &mut ProjectDocument,
    _parsed: &Parsed,
    _at: Tick,
) -> Result<TextImportSummary, ModelError> {
    todo!()
}
