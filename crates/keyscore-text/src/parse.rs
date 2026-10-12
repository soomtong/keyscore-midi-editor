use crate::{Mode, ParseError, Parsed};

/// 계이름 텍스트를 읽는다.
///
/// # Errors
///
/// 문법 오류(엄격 모드에서는 경고가 될 것도)의 첫 위치를 담은 [`ParseError`].
pub fn parse(text: &str, mode: Mode) -> Result<Parsed, ParseError> {
    let _ = (text, mode);
    todo!()
}
