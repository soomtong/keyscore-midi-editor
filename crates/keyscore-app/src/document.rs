//! 창 하나가 가진 프로젝트 문서.
//!
//! 모델은 `core::Document`(undo 엔진)를 거쳐서만 바뀐다. 이 타입은 읽기(`project`)와
//! `apply`·`undo`·`redo`만 내주고 `&mut Project`를 내주지 않는다(S2-T1 리뷰 초점).

use std::path::{Path, PathBuf};

use keyscore_core::{Document, Edit, ModelError, Project};

/// undo 히스토리 메모리 상한(창마다). 넘으면 가장 오래된 단위부터 버리고 처음 한 번 알린다(EDT-1, 결정
/// `docs/decisions/2026-10-19-undo-history-memory-limit.md` A).
pub const HISTORY_LIMIT_BYTES: usize = 256 * 1024 * 1024;

/// 새 프로젝트의 이름.
pub const UNTITLED: &str = "제목 없음";

/// 파일을 덮어쓰지 않는 이유.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOnly {
    /// 이 앱보다 새 형식 버전의 파일(결정 `2026-10-12-ksq-newer-version-save` A). 「다른 이름으로 저장」만 된다.
    NewerFormat(u64),
}

/// 창의 문서. GPUI `Entity<ProjectDocument>`로 담긴다.
pub struct ProjectDocument {
    document: Document,
    /// 저장한 뒤 바뀌었는가(S2-T7의 닫기 시트·자동 저장이 쓴다).
    dirty: bool,
    /// 이 문서의 파일(정규화한 경로). 새 프로젝트는 없다.
    path: Option<PathBuf>,
    read_only: Option<ReadOnly>,
}

impl ProjectDocument {
    /// # Errors
    ///
    /// 프로젝트가 히스토리 상한보다 크면 [`ModelError`].
    pub fn new(project: Project) -> Result<Self, ModelError> {
        Self::with_limit(project, HISTORY_LIMIT_BYTES)
    }

    /// 히스토리 상한을 정해 만든다(시험이 작은 상한을 쓴다).
    ///
    /// # Errors
    ///
    /// 프로젝트가 히스토리 상한보다 크면 [`ModelError`].
    pub fn with_limit(project: Project, limit_bytes: usize) -> Result<Self, ModelError> {
        Ok(ProjectDocument {
            document: Document::new(project, limit_bytes)?,
            dirty: false,
            path: None,
            read_only: None,
        })
    }

    /// 상한 때문에 지금까지 버린 undo 단위 수.
    #[must_use]
    pub fn dropped_undo_units(&self) -> u64 {
        self.document.dropped_undo_units()
    }

    #[must_use]
    pub fn project(&self) -> &Project {
        self.document.project()
    }

    /// 편집 하나를 undo 단위 하나로 적용한다.
    ///
    /// # Errors
    ///
    /// 모델 불변식을 어기는 편집이면 [`ModelError`]. 이때 문서는 바뀌지 않는다.
    pub fn apply(&mut self, edits: Vec<Edit>) -> Result<(), ModelError> {
        // 빈 편집은 아무것도 바꾸지 않는다. 「바뀜」 표시를 남기지 않는다(리뷰 1 제안 6).
        if edits.is_empty() {
            return Ok(());
        }
        self.document.apply(edits, None)?;
        self.dirty = true;
        Ok(())
    }

    /// # Errors
    ///
    /// [`ModelError`].
    pub fn undo(&mut self) -> Result<bool, ModelError> {
        let done = self.document.undo()?;
        self.dirty |= done;
        Ok(done)
    }

    /// # Errors
    ///
    /// [`ModelError`].
    pub fn redo(&mut self) -> Result<bool, ModelError> {
        let done = self.document.redo()?;
        self.dirty |= done;
        Ok(done)
    }

    #[must_use]
    pub fn undo_len(&self) -> usize {
        self.document.undo_len()
    }

    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// 이 문서의 파일. 새 프로젝트는 없다.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    #[must_use]
    pub fn read_only(&self) -> Option<ReadOnly> {
        self.read_only
    }

    /// 창 제목과 시트에 쓰는 이름: 파일 이름, 새 프로젝트면 「제목 없음」.
    #[must_use]
    pub fn display_name(&self) -> String {
        self.path
            .as_deref()
            .and_then(Path::file_name)
            .map_or_else(|| UNTITLED.to_owned(), |n| n.to_string_lossy().into_owned())
    }

    /// 손대지 않은 새 프로젝트인가(열기가 이 창을 연 파일 창으로 바꾼다).
    #[must_use]
    pub fn is_pristine(&self) -> bool {
        self.path.is_none() && !self.dirty && self.document.undo_len() == 0
    }

    /// 파일에서 읽은 문서로 표시한다.
    pub(crate) fn set_file(&mut self, path: PathBuf, read_only: Option<ReadOnly>) {
        self.path = Some(path);
        self.read_only = read_only;
    }

    /// `path`에 저장했다. 바뀜을 끄고 읽기 전용을 푼다(새 파일은 이 앱의 형식 버전이다).
    pub(crate) fn mark_saved(&mut self, path: PathBuf) {
        self.path = Some(path);
        self.read_only = None;
        self.dirty = false;
    }
}
