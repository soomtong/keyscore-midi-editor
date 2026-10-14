/// UMP 메시지 타입(첫 워드의 상위 4비트)이 정하는 메시지 길이(워드 수).
///
/// MIDI 2.0 UMP 명세의 표를 따른다. 송신 전에 길이를 검사해, 잘린 메시지를 CoreMIDI 호출로
/// 넘기지 않게 한다.
#[must_use]
pub fn ump_word_count(first_word: u32) -> usize {
    match first_word >> 28 {
        0x0..=0x2 | 0x6 | 0x7 => 1,
        0x3 | 0x4 | 0x8..=0xA => 2,
        0xB | 0xC => 3,
        // 0x5, 0xD–0xF. 시프트 결과는 4비트라 다른 값은 없다.
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::ump_word_count;

    #[test]
    fn ump_word_count_matches_message_type() {
        // UMP 명세 표: 타입 0x0–0x2 1워드, 0x3–0x4 2워드, 0x5 4워드, 0x6–0x7 1워드,
        // 0x8–0xA 2워드, 0xB–0xC 3워드, 0xD–0xF 4워드.
        let expected = [1, 1, 1, 2, 2, 4, 1, 1, 2, 2, 2, 3, 3, 4, 4, 4];
        for (mt, &n) in expected.iter().enumerate() {
            let word = u32::try_from(mt).unwrap() << 28;
            assert_eq!(ump_word_count(word), n, "message type {mt:#x}");
            assert_eq!(
                ump_word_count(word | 0x0fff_ffff),
                n,
                "message type {mt:#x} low bits"
            );
        }
    }
}
