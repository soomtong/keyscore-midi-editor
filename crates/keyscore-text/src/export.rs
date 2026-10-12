use keyscore_core::{Bank, Channel, Clip, Program};

use crate::Header;

/// 내보낼 트랙 하나. 입력 묶음이라 공개 필드로 둔다(`NoteRef`와 같은 이유).
#[derive(Debug, Clone, Copy)]
pub struct ExportTrack<'a> {
    pub name: &'a str,
    pub channel: Channel,
    pub program: Option<Program>,
    pub bank: Option<Bank>,
    pub clip: &'a Clip,
}

/// 헤더와 트랙들을 계이름 텍스트로 적는다.
#[must_use]
pub fn export(header: &Header, tracks: &[ExportTrack<'_>]) -> String {
    let _ = (header, tracks);
    todo!()
}
