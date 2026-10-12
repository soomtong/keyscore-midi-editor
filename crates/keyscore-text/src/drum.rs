//! GM 퍼커션 라벨 표(REQ 5.13). Step 모드·계이름 텍스트·AI 응답·건반 뷰가 같은 표를 쓴다.
//!
//! 라벨은 모두 두 글자 소문자다. 파서는 이 성질로 드럼 트랙에서 라벨을 최장 일치로 읽는다(결정 Q4).
//! 설정에서 라벨을 고치는 기능(REQ 5.13 끝)은 이후 Task다.

/// (라벨, GM 노트 번호). REQ 5.13 표의 순서.
const LABELS: [(&str, u8); 16] = [
    ("bd", 36),
    ("sd", 38),
    ("rs", 37),
    ("cp", 39),
    ("ch", 42),
    ("ph", 44),
    ("oh", 46),
    ("lt", 45),
    ("mt", 47),
    ("ht", 50),
    ("cr", 49),
    ("rd", 51),
    ("rb", 53),
    ("tb", 54),
    ("cb", 56),
    ("sh", 70),
];

/// 라벨이 있는 노트 번호면 라벨을 돌려준다.
#[must_use]
pub fn label_of(note: u8) -> Option<&'static str> {
    LABELS.iter().find(|(_, n)| *n == note).map(|(l, _)| *l)
}

/// 라벨(대소문자 무시)의 노트 번호.
#[must_use]
pub fn note_of(label: &str) -> Option<u8> {
    LABELS
        .iter()
        .find(|(l, _)| l.eq_ignore_ascii_case(label))
        .map(|(_, n)| *n)
}

/// 두 문자(이미 소문자)가 라벨이면 노트 번호. 파서가 문자열을 만들지 않고 보게 한다.
pub(crate) fn note_of_chars(a: char, b: char) -> Option<u8> {
    LABELS
        .iter()
        .find(|(l, _)| {
            let mut cs = l.chars();
            cs.next() == Some(a) && cs.next() == Some(b)
        })
        .map(|(_, n)| *n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_table_matches_req_5_13() {
        let table = [
            ("bd", 36),
            ("sd", 38),
            ("rs", 37),
            ("cp", 39),
            ("ch", 42),
            ("ph", 44),
            ("oh", 46),
            ("lt", 45),
            ("mt", 47),
            ("ht", 50),
            ("cr", 49),
            ("rd", 51),
            ("rb", 53),
            ("tb", 54),
            ("cb", 56),
            ("sh", 70),
        ];
        for (label, note) in table {
            assert_eq!(note_of(label), Some(note), "{label}");
            assert_eq!(label_of(note), Some(label), "{note}");
        }
        assert_eq!(note_of("BD"), Some(36));
        assert_eq!(note_of("xx"), None);
        assert_eq!(note_of("bass drum 1"), None);
        let labelled = (0..=127u8).filter(|&n| label_of(n).is_some()).count();
        assert_eq!(labelled, table.len());
    }
}
