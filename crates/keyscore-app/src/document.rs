//! 창 하나가 가진 프로젝트 문서.
//!
//! 모델은 `core::Document`(undo 엔진)를 거쳐서만 바뀐다. 이 타입은 읽기(`project`)와
//! `apply`·`undo`·`redo`만 내주고 `&mut Project`를 내주지 않는다(S2-T1 리뷰 초점).

use keyscore_core::{Document, Edit, ModelError, Project};

/// undo 히스토리 메모리 상한의 임시값. 정책은 S2-T6에서 정한다(S0-T4 이월, EDT-1).
const HISTORY_LIMIT_BYTES: usize = 256 * 1024 * 1024;

/// 창의 문서. GPUI `Entity<ProjectDocument>`로 담긴다.
pub struct ProjectDocument {
    document: Document,
    /// 저장한 뒤 바뀌었는가(S2-T7의 닫기 시트·자동 저장이 쓴다).
    dirty: bool,
}

impl ProjectDocument {
    /// # Errors
    ///
    /// 프로젝트가 히스토리 상한보다 크면 [`ModelError`].
    pub fn new(project: Project) -> Result<Self, ModelError> {
        Ok(ProjectDocument {
            document: Document::new(project, HISTORY_LIMIT_BYTES)?,
            dirty: false,
        })
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
}
