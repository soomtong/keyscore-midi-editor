//! `.ksq` 프로젝트와 계이름 텍스트를 오가는 명령줄 도구(REQ 5.14, 9.5)와 실험실 픽스처
//! 생성기(AI-18).
//!
//! 앱 없이 픽스처를 만들고 검사하려는 도구라 `keyscore-core`와 `keyscore-text`에만 의존한다.
//! 바이너리 셋(`ksq`, `ksq2txt`, `txt2ksq`)은 [`cli`]의 함수를 부르는 껍데기이고, 통합 테스트는
//! 이 crate의 함수를 직접 부른다.
#![deny(unsafe_code)]

mod check;
pub mod cli;
mod convert;
mod fixtures;

use std::path::PathBuf;

use keyscore_core::ModelError;
use keyscore_core::ksq::KsqError;
use keyscore_text::ParseError;
use thiserror::Error;

pub use check::{CheckReport, check};
pub use convert::{Loss, project_to_text, text_to_project};
pub use fixtures::{fixtures, write_fixtures};

/// 도구의 오류. 파일을 다루는 오류는 사람이 어느 파일인지 알 수 있게 경로를 담는다.
#[derive(Debug, Error)]
pub enum CliError {
    #[error("{}: 입출력 오류: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{}: {source}", path.display())]
    Ksq { path: PathBuf, source: KsqError },
    #[error(
        "{}: 파일 형식 버전 {version}은 이 도구가 아는 버전({known})보다 새롭다",
        path.display()
    )]
    NewerVersion {
        path: PathBuf,
        version: u64,
        known: u64,
    },
    #[error("텍스트 오류: {0}")]
    Parse(#[from] ParseError),
    #[error("모델 오류: {0}")]
    Model(#[from] ModelError),
    /// 내보낸 텍스트를 다시 읽지 못했다. `keyscore-text`의 내보내기·파서가 어긋난 것이다.
    #[error("내보낸 텍스트를 다시 읽지 못했다(keyscore-text 버그): {0}")]
    ExportUnreadable(ParseError),
    #[error("{}: 출력 파일이 입력 파일과 같다. 입력을 덮어쓰지 않는다", path.display())]
    SameInputOutput { path: PathBuf },
}
