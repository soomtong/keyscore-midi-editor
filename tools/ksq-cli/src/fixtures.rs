use std::fs;
use std::path::{Path, PathBuf};

use keyscore_core::Project;
use keyscore_core::ksq::{self, Compression};
use keyscore_text::Mode;

use crate::{CliError, text_to_project};

/// (파일 이름, 원본 텍스트). 픽스처는 사람이 읽고 고칠 수 있게 계이름 텍스트로 쓰고, 엄격 모드로
/// 읽어 만든다. 엄격 모드로 읽히는 텍스트만 쓰므로 텍스트 왕복이 보장된다.
const SOURCES: [(&str, &str); 8] = [
    (
        "01-melody.ksq",
        include_str!("../fixtures-src/01-melody.txt"),
    ),
    (
        "02-melody-chords.ksq",
        include_str!("../fixtures-src/02-melody-chords.txt"),
    ),
    ("03-drums.ksq", include_str!("../fixtures-src/03-drums.txt")),
    ("04-band.ksq", include_str!("../fixtures-src/04-band.txt")),
    (
        "05-lyrics.ksq",
        include_str!("../fixtures-src/05-lyrics.txt"),
    ),
    ("06-waltz.ksq", include_str!("../fixtures-src/06-waltz.txt")),
    (
        "07-triplets.ksq",
        include_str!("../fixtures-src/07-triplets.txt"),
    ),
    (
        "08-bass-drums.ksq",
        include_str!("../fixtures-src/08-bass-drums.txt"),
    ),
];

/// 실험실 번들 픽스처 8곡(AI-18). 같은 빌드면 늘 같은 프로젝트다.
///
/// # Panics
///
/// 원본 텍스트가 엄격 모드로 읽히지 않으면. 원본은 빌드에 박혀 있고 테스트가 지킨다.
#[must_use]
pub fn fixtures() -> Vec<(&'static str, Project)> {
    SOURCES
        .iter()
        .map(|&(name, text)| {
            let (project, warnings) = text_to_project(text, Mode::Strict)
                .unwrap_or_else(|e| panic!("픽스처 {name}의 원본 텍스트가 읽히지 않는다: {e}"));
            // 엄격 모드는 경고를 오류로 내므로 여기는 늘 비어 있다.
            debug_assert!(warnings.is_empty());
            (name, project)
        })
        .collect()
}

/// 픽스처를 `dir`에 `.ksq`로 쓴다. 디렉터리가 없으면 만든다. 쓴 파일 경로를 돌려준다.
///
/// # Errors
///
/// 디렉터리를 만들거나 파일을 쓰지 못하면 [`CliError::Io`]·[`CliError::Ksq`].
pub fn write_fixtures(dir: &Path) -> Result<Vec<PathBuf>, CliError> {
    fs::create_dir_all(dir).map_err(|source| CliError::Io {
        path: dir.to_owned(),
        source,
    })?;
    let mut written = Vec::with_capacity(SOURCES.len());
    for (name, project) in fixtures() {
        let path = dir.join(name);
        // core의 결정적 쓰기라서 같은 생성기면 같은 바이트다.
        ksq::write_file(&path, &project, Compression::None).map_err(|source| CliError::Ksq {
            path: path.clone(),
            source,
        })?;
        written.push(path);
    }
    Ok(written)
}
