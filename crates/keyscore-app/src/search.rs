//! 검색의 순수 계산(S3-T7, REQ 5.8 `Cmd+F`, 5.8.1 `/`·`n`·`N`). GPUI와 무관하다.
//!
//! 규칙은 결정 `docs/decisions/2026-11-01-s3-t7-search-and-note-info.md` 2·3절이다. 창 쪽(막대·이동)은
//! `search_bar.rs`.

use keyscore_core::{Pitch, Project, Tick, TrackId};

/// 읽은 검색어.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Query {
    /// 옥타브가 있는 음높이(`c#4`).
    Pitch(Pitch),
    /// 옥타브가 없는 음이름(`c#`). 0–11, C = 0.
    PitchClass(u8),
    /// 트랙 이름·가사의 부분 문자열(소문자로 바꾼 것).
    Text(String),
}

/// 검색어를 읽지 못한 까닭.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryError {
    /// 빈 검색어.
    Empty,
    /// 음높이 꼴이지만 0–127 밖.
    PitchOutOfRange,
}

impl QueryError {
    /// 막대·상태줄에 보일 글.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            QueryError::Empty => "검색어가 비었다",
            QueryError::PitchOutOfRange => "음높이 범위 밖",
        }
    }
}

impl Query {
    /// 검색어를 읽는다(결정 2절).
    ///
    /// # Errors
    ///
    /// 비었으면 [`QueryError::Empty`], 음높이 꼴인데 0–127 밖이면 [`QueryError::PitchOutOfRange`].
    pub fn parse(text: &str) -> Result<Query, QueryError> {
        let _ = text;
        todo!()
    }
}

/// 결과 하나: 트랙과 그 안의 자리.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub track: TrackId,
    /// 트랙 기준 틱(노트 결과는 클립 시작 + 노트 시작).
    pub tick: Tick,
    pub pitch: Pitch,
    /// 자리에 노트가 있는가. 노트 없는 트랙 결과는 거짓이고, 이동할 때 커서 음높이를 그대로 둔다.
    pub has_note: bool,
}

/// 프로젝트에서 `query`의 결과를 (틱, 트랙 순서, 음높이) 순서로 찾는다. 같은 자리는 하나다(결정 3절).
#[must_use]
pub fn find_hits(project: &Project, query: &Query) -> Vec<Hit> {
    let _ = (project, query);
    todo!()
}

/// 지금 자리: 현재 트랙 순서, 커서 틱·음높이.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub track_index: usize,
    pub tick: Tick,
    pub pitch: Pitch,
}

/// 찾는 방향.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

/// 고른 결과.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found {
    /// `hits` 안의 순번.
    pub index: usize,
    /// 끝을 넘어 반대 끝에서 골랐다(결정 3절 5).
    pub wrapped: bool,
}

/// `at` 다음(또는 이전) 결과. 결과가 없으면 `None`.
#[must_use]
pub fn step(project: &Project, hits: &[Hit], at: Position, direction: Direction) -> Option<Found> {
    let _ = (project, hits, at, direction);
    todo!()
}
