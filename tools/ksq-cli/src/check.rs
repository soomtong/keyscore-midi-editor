use std::fs;
use std::io::Read;
use std::path::Path;

use keyscore_core::Track;
use keyscore_core::ksq::{self, FORMAT_VERSION};
use serde_json::Value;

use crate::CliError;

/// `format_version`을 보려고 gzip을 풀 때의 상한. `keyscore-core`의 읽기 상한(512 MiB)과 같다.
const MAX_UNPACKED_BYTES: u64 = 512 * 1024 * 1024;

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
    let bytes = fs::read(path).map_err(|source| CliError::Io {
        path: path.to_owned(),
        source,
    })?;
    // core는 새 버전 파일도 경고와 함께 읽는다(앱은 열어 보여야 하므로). 검사 도구는 이 코드가
    // 해석하지 못한 내용이 있을 수 있는 파일을 통과시키지 않는다. 읽기 오류보다 먼저 보는 것은,
    // 새 버전 파일이 읽기에 실패했을 때도 「버전이 새롭다」가 더 쓸모 있는 이유라서다.
    let version = format_version(&bytes);
    if let Some(v) = version
        && v > FORMAT_VERSION
    {
        return Err(CliError::NewerVersion {
            path: path.to_owned(),
            version: v,
            known: FORMAT_VERSION,
        });
    }
    let (project, read_warnings) = ksq::from_bytes(&bytes).map_err(|source| CliError::Ksq {
        path: path.to_owned(),
        source,
    })?;
    let mut warnings: Vec<String> = read_warnings.iter().map(ToString::to_string).collect();
    if version.is_none() {
        warnings.push("format_version이 없다. 이 도구가 아는 형식으로 읽었다".to_owned());
    }
    let tracks = project.tracks();
    Ok(CheckReport {
        version,
        tracks: tracks.len(),
        clips: tracks.iter().map(|t| t.clips().count()).sum(),
        notes: tracks
            .iter()
            .flat_map(Track::clips)
            .map(|(_, c)| c.notes().len())
            .sum(),
        warnings,
    })
}

/// 최상위 `format_version`(정수). gzip이면 풀어서 본다. 읽을 수 없으면 `None`이고, 그 경우의
/// 오류는 뒤이은 core 읽기가 보고한다.
fn format_version(bytes: &[u8]) -> Option<u64> {
    let root: Value = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut text = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .take(MAX_UNPACKED_BYTES)
            .read_to_end(&mut text)
            .ok()?;
        serde_json::from_slice(&text).ok()?
    } else {
        serde_json::from_slice(bytes).ok()?
    };
    root.get("format_version")?.as_u64()
}
