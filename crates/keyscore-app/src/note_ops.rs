//! 피아노롤의 분할·합치기·길이 코드·음높이 바꾸기·퀀타이즈·레가토·마지막 편집 반복(S3-T3b, REQ 5.8, 5.8.1,
//! 5.7, 5.6). 규칙은 `docs/reports/s3-t3b/plan.md` 「끝의 정의」에 있다.
//!
//! 편집 계산은 `pianoroll::edit`(순수), 음 키 해석은 `pianoroll::keys`, 여기는 창 처리기다. 대상 고르기와 적용은
//! S3-T3a `note_edit`의 도우미를 함께 쓴다.

use keyscore_core::{ClipId, Note, TrackId};

/// 창마다 두는 노트 편집 상태.
#[derive(Default)]
pub(crate) struct NoteOpsState {
    /// `e` 뒤 음 키를 기다리는 중.
    pub(crate) pitch_pending: bool,
    /// 마지막 레가토(토글 되돌리기용).
    pub(crate) legato: Option<LegatoMemory>,
}

/// 마지막 레가토: 그때의 문서 판과 (원래, 바뀐) 짝.
pub(crate) struct LegatoMemory {
    pub(crate) revision: u64,
    pub(crate) track: TrackId,
    pub(crate) clip: ClipId,
    pub(crate) pairs: Vec<(Note, Note)>,
}
