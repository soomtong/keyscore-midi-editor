//! vim 오퍼레이터 대기·모션·텍스트 객체·비주얼 선택의 창 처리기(S3-T3c, REQ 5.8.1, 5.7). 규칙은
//! `docs/reports/s3-t3c/plan.md` 「끝의 정의」.
//!
//! 계산은 `pianoroll::vim`(순수), 대상 고르기·삭제·복사는 S3-T3a `note_edit`의 도우미를 쓴다.

use crate::pianoroll::vim::Operator;
use crate::window::ProjectWindow;

/// 오퍼레이터 대기: 오퍼레이터와 그 앞의 카운트.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Pending {
    pub(crate) op: Operator,
    pub(crate) count: Option<u32>,
}

/// 창마다 두는 vim 상태.
#[derive(Default)]
pub(crate) struct VimState {
    pub(crate) pending: Option<Pending>,
}

impl ProjectWindow {
    /// 대기 중인 오퍼레이터와 그 앞의 카운트(S3-T3c).
    #[must_use]
    pub fn pending_operator(&self) -> Option<(Operator, Option<u32>)> {
        self.vim.pending.map(|p| (p.op, p.count))
    }
}
