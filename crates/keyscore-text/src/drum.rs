//! GM 퍼커션 라벨 표(REQ 5.13). Step 모드·계이름 텍스트·AI 응답·건반 뷰가 같은 표를 쓴다.

/// 라벨이 있는 노트 번호면 라벨을 돌려준다.
#[must_use]
pub fn label_of(note: u8) -> Option<&'static str> {
    let _ = note;
    todo!()
}

/// 라벨(대소문자 무시)의 노트 번호.
#[must_use]
pub fn note_of(label: &str) -> Option<u8> {
    let _ = label;
    todo!()
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
