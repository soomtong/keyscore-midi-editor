use std::path::Path;

use crate::CliError;

/// `ksq check` 한 파일의 결과. 모델 불변식은 `keyscore-core`가 읽을 때 지키므로, 읽혔다는 것이
/// 곧 불변식 통과다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckReport {
    version: Option<u64>,
    tracks: usize,
    clips: usize,
    notes: usize,
    warnings: Vec<String>,
}

impl CheckReport {
    /// 파일의 `format_version`. 없으면 `None`(경고 하나가 붙는다).
    #[must_use]
    pub const fn version(&self) -> Option<u64> {
        self.version
    }

    #[must_use]
    pub const fn tracks(&self) -> usize {
        self.tracks
    }

    #[must_use]
    pub const fn clips(&self) -> usize {
        self.clips
    }

    #[must_use]
    pub const fn notes(&self) -> usize {
        self.notes
    }

    /// 통과했지만 사람에게 알릴 것. 표준 오류에 쓴다.
    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

/// `.ksq`·`.ksqz` 파일을 읽어 불변식과 형식 버전을 검사한다.
///
/// # Errors
///
/// 읽을 수 없거나 모델 불변식을 어기면 [`CliError::Io`]·[`CliError::Ksq`], 이 도구보다 새 형식
/// 버전이면 [`CliError::NewerVersion`].
pub fn check(path: &Path) -> Result<CheckReport, CliError> {
    let _ = path;
    todo!()
}
