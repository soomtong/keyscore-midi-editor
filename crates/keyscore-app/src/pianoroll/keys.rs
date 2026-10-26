//! 음 키 해석(P3, KM-7). S3-T3b `e` 음높이 바꾸기가 쓰고, S3-T4 Step 입력도 같은 표를 쓴다.

use keyscore_core::Pitch;

use crate::actions::args::KeyLayout;

/// 음 키의 옥타브 안 반음 자리(C = 0). 건반형은 `a`–`l`(흰건반 C–D)과 `w e t y u o p`(검은건반), 계이름형은
/// `c d e f g a b`. 음 키가 아니면 `None`.
#[must_use]
pub fn semitone_of_key(layout: KeyLayout, key: &str) -> Option<u8> {
    let _ = (layout, key);
    todo!()
}

/// `cursor`가 든 옥타브의 C에 `semitone`을 더한 음높이. 0–127 밖이면 `None`.
#[must_use]
pub fn pitch_in_octave_of(cursor: Pitch, semitone: u8) -> Option<Pitch> {
    let _ = (cursor, semitone);
    todo!()
}
