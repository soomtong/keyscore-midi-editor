use std::fmt;

use thiserror::Error;

/// 파싱 모드. 같은 문법을 입력의 출처에 따라 다르게 다룬다(결정 Q6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// 파일·클립보드 가져오기. 모르는 토큰과 마디 길이 불일치는 경고하고 계속한다.
    Lenient,
    /// AI 응답. 느슨 모드의 경고를 모두 오류로 내고 `~` 보정도 거부한다. 오류는 AI-6
    /// 재요청의 사유가 되므로, 건너뛴 토큰 때문에 마디가 어긋난 결과를 받지 않는다.
    Strict,
}

/// 입력 위치가 붙은 경고·오류 한 건.
///
/// 줄·열은 1부터 센다. 열은 줄 안의 문자(Unicode scalar) 순번이다. 사람에게 보이고
/// AI 재요청에 그대로 넣을 수 있게 바이트가 아니라 문자로 센다.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Diagnostic {
    line: usize,
    column: usize,
    message: String,
}

impl Diagnostic {
    pub(crate) fn new(line: usize, column: usize, message: impl Into<String>) -> Self {
        Diagnostic {
            line,
            column,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn line(&self) -> usize {
        self.line
    }

    #[must_use]
    pub const fn column(&self) -> usize {
        self.column
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "줄 {}, 열 {}: {}", self.line, self.column, self.message)
    }
}

/// 파싱을 멈춘 첫 오류.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Error)]
#[error("{0}")]
pub struct ParseError(Diagnostic);

impl ParseError {
    pub(crate) fn new(diagnostic: Diagnostic) -> Self {
        ParseError(diagnostic)
    }

    #[must_use]
    pub fn diagnostic(&self) -> &Diagnostic {
        &self.0
    }
}
