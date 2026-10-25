//! 명령줄(결정 `docs/decisions/2026-10-17-keymap-conflicts.md` A12): 팔레트 질의가 `:`로 시작하면 ex 명령으로
//! 읽는다. REQ 7.5의 `:new` `:e` `:w` `:w name` `:q` `:set`. `:ai`(S7)·`:test`(S8)는 아직 모르는 명령이다.

use crate::actions::args::SettingsSection;

/// 읽힌 ex 명령.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExCommand {
    /// `:w` 저장, `:w 이름` 다른 이름으로 저장(이름 제안)
    Write(Option<String>),
    /// `:q` 창 닫기
    Quit,
    /// `:e` 열기 대화상자, `:e 경로` 그 파일 열기
    Edit(Option<String>),
    /// `:new` 새 프로젝트
    New,
    /// `:set` 설정, `:set 구역` 그 구역
    Set(Option<SettingsSection>),
}

/// 읽기 실패.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExError {
    #[error("명령이 없다")]
    Empty,
    #[error("`:`로 시작하지 않는다")]
    NotCommandLine,
    #[error("모르는 명령: {0}")]
    Unknown(String),
    #[error("{0}에는 인자가 없다")]
    UnexpectedArgument(String),
    #[error("모르는 설정 구역: {0}")]
    UnknownSection(String),
}

/// 질의가 명령줄인가(앞 공백 뒤 `:`).
#[must_use]
pub fn is_command_line(query: &str) -> bool {
    let _ = query;
    todo!("S3-T2")
}

/// `:w name` 같은 질의를 읽는다.
///
/// # Errors
///
/// [`ExError`].
pub fn parse(query: &str) -> Result<ExCommand, ExError> {
    let _ = query;
    todo!("S3-T2")
}

impl ExCommand {
    /// 팔레트 줄에 보이는 설명.
    #[must_use]
    pub fn label(&self) -> String {
        todo!("S3-T2")
    }
}
