//! 음 키 해석(P3, KM-7). S3-T3b `e` 음높이 바꾸기가 쓰고, S3-T4 Step 입력도 같은 표를 쓴다.

use keyscore_core::Pitch;

use crate::actions::args::KeyLayout;

/// 음 키의 옥타브 안 반음 자리(C = 0). 건반형은 `a`–`l`(흰건반 C–D)과 `w e t y u o p`(검은건반), 계이름형은
/// `c d e f g a b`. 음 키가 아니면 `None`.
#[must_use]
pub fn semitone_of_key(layout: KeyLayout, key: &str) -> Option<u8> {
    // 입력 글자가 대문자로 올 수 있다(Caps Lock 등). 같은 음으로 읽는다.
    let mut chars = key.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return None;
    };
    let table: &[(char, u8)] = match layout {
        // P3 (a): 홈 로우 흰건반 C D E F G A B C D, 윗줄 검은건반.
        KeyLayout::Piano => &[
            ('a', 0),
            ('w', 1),
            ('s', 2),
            ('e', 3),
            ('d', 4),
            ('f', 5),
            ('t', 6),
            ('g', 7),
            ('y', 8),
            ('h', 9),
            ('u', 10),
            ('j', 11),
            ('k', 12),
            ('o', 13),
            ('l', 14),
            ('p', 15),
        ],
        // P3 (b): 음이름 글자.
        KeyLayout::Solfege => &[
            ('c', 0),
            ('d', 2),
            ('e', 4),
            ('f', 5),
            ('g', 7),
            ('a', 9),
            ('b', 11),
        ],
    };
    let c = c.to_ascii_lowercase();
    table.iter().find(|(k, _)| *k == c).map(|(_, s)| *s)
}

/// `cursor`가 든 옥타브의 C에 `semitone`을 더한 음높이. 0–127 밖이면 `None`.
#[must_use]
pub fn pitch_in_octave_of(cursor: Pitch, semitone: u8) -> Option<Pitch> {
    let base = cursor.get() - cursor.get() % 12;
    Pitch::new(base.checked_add(semitone)?).ok()
}
