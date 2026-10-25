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

/// 설정 구역 이름(`:set 이름`). 설정 파일의 키와 같다.
const SECTIONS: [(&str, SettingsSection); 5] = [
    ("metronome", SettingsSection::Metronome),
    ("midi_output", SettingsSection::MidiOutput),
    ("input", SettingsSection::Input),
    ("ai", SettingsSection::Ai),
    ("keymap", SettingsSection::Keymap),
];

/// 질의가 명령줄인가(앞 공백 뒤 `:`).
#[must_use]
pub fn is_command_line(query: &str) -> bool {
    query.trim_start().starts_with(':')
}

/// `:w name` 같은 질의를 읽는다.
///
/// # Errors
///
/// [`ExError`].
pub fn parse(query: &str) -> Result<ExCommand, ExError> {
    let Some(rest) = query.trim_start().strip_prefix(':') else {
        return Err(ExError::NotCommandLine);
    };
    let rest = rest.trim();
    if rest.is_empty() {
        return Err(ExError::Empty);
    }
    let (name, arg) = match rest.split_once(char::is_whitespace) {
        Some((name, arg)) => (name, Some(arg.trim())),
        None => (rest, None),
    };
    let arg = arg.filter(|a| !a.is_empty());
    let no_arg = |cmd: ExCommand| match arg {
        Some(_) => Err(ExError::UnexpectedArgument(format!(":{name}"))),
        None => Ok(cmd),
    };
    match name {
        "w" => Ok(ExCommand::Write(arg.map(str::to_owned))),
        "q" => no_arg(ExCommand::Quit),
        "e" => Ok(ExCommand::Edit(arg.map(str::to_owned))),
        "new" => no_arg(ExCommand::New),
        "set" => match arg {
            None => Ok(ExCommand::Set(None)),
            Some(section) => SECTIONS
                .iter()
                .find(|(key, _)| *key == section)
                .map(|(_, s)| ExCommand::Set(Some(*s)))
                .ok_or_else(|| ExError::UnknownSection(section.to_owned())),
        },
        other => Err(ExError::Unknown(format!(":{other}"))),
    }
}

impl ExCommand {
    /// 팔레트 줄에 보이는 설명.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            ExCommand::Write(None) => "저장".to_owned(),
            ExCommand::Write(Some(name)) => format!("다른 이름으로 저장: {name}"),
            ExCommand::Quit => "창 닫기".to_owned(),
            ExCommand::Edit(None) => "열기".to_owned(),
            ExCommand::Edit(Some(path)) => format!("열기: {path}"),
            ExCommand::New => "새 프로젝트".to_owned(),
            ExCommand::Set(None) => "설정".to_owned(),
            ExCommand::Set(Some(section)) => {
                let name = SECTIONS
                    .iter()
                    .find(|(_, s)| s == section)
                    .map_or("", |(key, _)| *key);
                format!("설정: {name}")
            }
        }
    }
}
