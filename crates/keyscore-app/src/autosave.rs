//! 자동 저장본과 크래시 후 복구(S2-T7b, IO-5, 8.4).
//!
//! 저장본은 설정 파일 옆 `autosave/`에만 쓴다(앱에서는 `~/Library/Application Support/KeyScore/autosave/`).
//! 프로젝트 폴더와 원래 파일은 건드리지 않는다. 창마다 파일 둘이다.
//!
//! - `<키>.ksq`: 문서 내용(평문 `.ksq`).
//! - `<키>.json`: 메타 `{"path": 원래 경로 또는 null, "read_only": 상위 형식 버전 또는 null,
//!   "session": 세션 id, "pid": 프로세스 id}`.
//!
//! 실행 중인 앱은 `session-<세션 id>.lock`을 배타 잠금으로 쥔다. 잠금을 잡을 수 있는 세션(파일이 없어도)은
//! 끝난 세션이고, 그 저장본이 복구 대상이다.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui_kit::{App, Task};

use crate::document::ReadOnly;

/// 문서가 바뀐 뒤 저장본이 생기기까지(IO-5).
pub const AUTOSAVE_DELAY: Duration = Duration::from_secs(30);

/// 복구할 수 있는 저장본 하나.
#[derive(Debug, Clone)]
pub struct Recoverable {
    path: Option<PathBuf>,
    read_only: Option<ReadOnly>,
}

impl Recoverable {
    /// 원래 파일 경로. 새 프로젝트였으면 없다.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    #[must_use]
    pub fn read_only(&self) -> Option<ReadOnly> {
        self.read_only
    }
}

/// 세션 잠금을 잡는다(`init`이 부른다).
pub(crate) fn init(_cx: &mut App) {}

/// 저장본 폴더.
#[must_use]
pub fn autosave_dir(_cx: &App) -> PathBuf {
    todo!()
}

/// 끝난 세션의 저장본(내용과 메타를 읽을 수 있는 것만).
#[must_use]
pub fn find_recoverable(_cx: &App) -> Vec<Recoverable> {
    todo!()
}

/// 복구 대상이 있으면 시트로 묻는다(`run`이 첫 창을 연 뒤 부른다).
pub fn offer_recovery(_cx: &mut App) -> Task<()> {
    todo!()
}

/// 바뀐 창의 저장본을 지금 쓴다(취소할 수 없는 종료 훅).
pub fn flush_all(_cx: &mut App) {
    todo!()
}
