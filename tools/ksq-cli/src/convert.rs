//! 텍스트↔프로젝트 변환은 `keyscore_text::project`로 옮겼다(S2-T7c). 도구의 공개 이름과
//! 오류 타입(`CliError`)을 지키려고 얇게 감싼다.

use keyscore_core::Project;
use keyscore_text::project as inner;
use keyscore_text::{Diagnostic, Mode, ParseOptions};

use crate::CliError;

pub use inner::{Loss, MAX_EXPANSION};

/// [`inner::project_to_text`]를 `CliError`로 감싼다.
///
/// # Errors
///
/// 안쪽 함수의 `ConvertError`를 같은 문구의 `CliError`로 바꾼다.
pub fn project_to_text(project: &Project) -> Result<(String, Vec<Loss>), CliError> {
    Ok(inner::project_to_text(project)?)
}

/// [`inner::project_to_text_limited`]를 `CliError`로 감싼다.
///
/// # Errors
///
/// 안쪽 함수의 `ConvertError`를 같은 문구의 `CliError`로 바꾼다.
pub fn project_to_text_limited(
    project: &Project,
    max_bytes: u64,
) -> Result<(String, Vec<Loss>), CliError> {
    Ok(inner::project_to_text_limited(project, max_bytes)?)
}

/// [`inner::text_to_project`]를 `CliError`로 감싼다.
///
/// # Errors
///
/// 안쪽 함수의 `ConvertError`를 같은 문구의 `CliError`로 바꾼다.
pub fn text_to_project(text: &str, mode: Mode) -> Result<(Project, Vec<Diagnostic>), CliError> {
    Ok(inner::text_to_project(text, mode)?)
}

/// [`inner::text_to_project_with`]를 `CliError`로 감싼다.
///
/// # Errors
///
/// 안쪽 함수의 `ConvertError`를 같은 문구의 `CliError`로 바꾼다.
pub fn text_to_project_with(
    text: &str,
    mode: Mode,
    options: ParseOptions,
) -> Result<(Project, Vec<Diagnostic>), CliError> {
    Ok(inner::text_to_project_with(text, mode, options)?)
}
