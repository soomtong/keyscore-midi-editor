//! 파일 열기·저장·닫기 흐름(S2-T7a, IO-1, 7.5).
//!
//! 대화상자와 시트는 GPUI 플랫폼 것(macOS 표준 시트)을 쓴다. 흐름은 창의 비동기 작업이고, 시트가 떠
//! 있는 동안 문서는 그대로 편집할 수 있다.

use std::path::{Path, PathBuf};

use gpui_kit::{AnyWindowHandle, App, Entity, Task};
use keyscore_core::Project;
use keyscore_core::ksq::KsqError;

use crate::OpenError;
use crate::window::ProjectWindow;

/// 최근 파일 목록의 길이.
pub const MAX_RECENT_FILES: usize = 10;

/// 파일 작업 오류.
#[derive(Debug, thiserror::Error)]
pub enum FileError {
    /// 읽지 못했다.
    #[error("{0}")]
    Read(KsqError),
    /// 쓰지 못했다.
    #[error("{0}")]
    Write(KsqError),
    /// 파일에 쓸 권한이 없다(결정 2026-10-21-s2-t7-save-policy-and-split 1 A).
    #[error("파일에 쓸 권한이 없다")]
    PermissionDenied,
    /// 다른 창이 이미 그 파일을 열고 있다.
    #[error("다른 창이 이미 이 파일을 열고 있다: {}", .0.display())]
    AlreadyOpen(PathBuf),
    /// 창을 열지 못했다.
    #[error("{0}")]
    Window(#[from] OpenError),
}

/// 파일 작업 Action을 건다(`init`이 부른다).
pub(crate) fn register(_cx: &mut App) {}

/// 프로젝트를 쓴다. `.ksqz`면 gzip, 그 밖은 평문이다. 쓰기는 임시 파일 + rename이라 도중에 죽어도 원본은
/// 옛 내용 그대로다.
///
/// # Errors
///
/// 쓸 권한이 없으면 [`FileError::PermissionDenied`], 그 밖의 실패는 [`FileError::Write`].
pub fn save_project(_path: &Path, _project: &Project) -> Result<(), FileError> {
    todo!()
}

/// 파일을 새 창으로 연다. 이미 열린 파일이면 그 창을 앞으로 가져온다.
///
/// # Errors
///
/// [`FileError`].
pub fn open_path(
    _path: &Path,
    _cx: &mut App,
) -> Result<(AnyWindowHandle, Entity<ProjectWindow>), FileError> {
    todo!()
}

/// 바뀐 창마다 차례로 닫기 시트를 띄운다. 모두 저장하거나 버렸으면 참, 하나라도 취소하면 거짓이다
/// (`app::Quit`이 참일 때만 끝낸다).
pub fn confirm_close_all(_cx: &mut App) -> Task<bool> {
    todo!()
}

/// 최근 파일(최근이 먼저).
#[must_use]
pub fn recent_files(_cx: &App) -> Vec<PathBuf> {
    todo!()
}
