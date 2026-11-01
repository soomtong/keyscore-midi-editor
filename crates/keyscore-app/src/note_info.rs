//! 노트 정보 팝업(S3-T7, REQ 5.8 `i` 「커서 노트 정보 팝업(틱·길이·채널)」, 5.8.1 vim `K`).
//!
//! 규칙은 결정 `docs/decisions/2026-11-01-s3-t7-search-and-note-info.md` 6절이다.

use keyscore_core::{ClipId, Note, Project, TrackId};

/// 팝업의 한 줄.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoLine {
    pub label: &'static str,
    pub value: String,
}

/// `track`의 `clip`에 든 `note`의 정보 줄(결정 6절 2). 트랙·클립이 없으면 빈 목록.
#[must_use]
pub fn info_lines(project: &Project, track: TrackId, clip: ClipId, note: &Note) -> Vec<InfoLine> {
    let _ = (project, track, clip, note);
    todo!()
}
