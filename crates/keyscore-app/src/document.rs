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
    /// 열거나 저장한 때의 파일 정체. 그 뒤 바깥에서 옮기거나 이름을 바꿔도 같은 파일임을 안다(S2-T7a 리뷰 2
    /// 지적 1·2).
    file_id: Option<crate::files::FileId>,
    read_only: Option<ReadOnly>,
    /// 편집·되돌리기·다시 실행마다 는다. 자동 저장본이 쓴 뒤 또 바뀌었는지 본다(S2-T7b).
    revision: u64,
    /// 크래시 뒤 자동 저장본에서 되살렸고 아직 저장하지 않았다.
    recovered: bool,
    /// 경로가 없을 때 다른 이름으로 저장이 제안할 이름(확장자 없음). 가져온 SMF의 파일 이름이다.
    suggested: Option<String>,
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
            file_id: None,
            read_only: None,
            revision: 0,
            recovered: false,
            suggested: None,
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
        self.revision += 1;
        Ok(())
    }

    /// 편집을 적용하되, 직전 단위가 같은 `key`로 적용된 것이면 한 undo 단위로 묶는다(REQ 5.7 「되돌리기 묶음」).
    /// 사이에 `seal`·다른 편집·undo가 있었으면 새 단위다.
    ///
    /// # Errors
    ///
    /// [`ProjectDocument::apply`]와 같다.
    pub fn apply_coalesced(&mut self, edits: Vec<Edit>, key: u64) -> Result<(), ModelError> {
        let _ = (edits, key);
        todo!("S3-T2")
    }

    /// 다음 `apply_coalesced`가 직전 단위와 묶이지 않게 한다(다른 키가 끼었을 때).
    pub fn seal(&mut self) {
        todo!("S3-T2")
    }

    /// # Errors
    ///
    /// [`ModelError`].
    pub fn undo(&mut self) -> Result<bool, ModelError> {
        let done = self.document.undo()?;
        self.dirty |= done;
        self.revision += u64::from(done);
        Ok(done)
    }

    /// # Errors
    ///
    /// [`ModelError`].
    pub fn redo(&mut self) -> Result<bool, ModelError> {
        let done = self.document.redo()?;
        self.dirty |= done;
        self.revision += u64::from(done);
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

    /// 다른 이름으로 저장이 제안할 이름(확장자 없음): 파일 이름 줄기, 가져온 파일 이름, 아니면 「제목 없음」.
    #[must_use]
    pub fn suggested_stem(&self) -> String {
        self.path
            .as_deref()
            .and_then(Path::file_stem)
            .map(|s| s.to_string_lossy().into_owned())
            .or_else(|| self.suggested.clone())
            .unwrap_or_else(|| UNTITLED.to_owned())
    }

    /// 가져온 문서로 표시한다: 저장 안 된 새 문서이고, 다른 이름으로 저장이 `stem`을 제안한다.
    pub(crate) fn mark_imported(&mut self, stem: String) {
        self.dirty = true;
        self.suggested = Some(stem);
    }

    /// 문서 내용의 판 번호.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// 자동 저장본에서 되살렸고 아직 저장하지 않았는가.
    #[must_use]
    pub const fn is_recovered(&self) -> bool {
        self.recovered
    }

    /// 자동 저장본에서 되살린 문서로 표시한다. 원래 파일과 다를 수 있어 「바뀜」이다.
    /// 원래 파일은 읽지 않는다(정체를 위한 메타데이터만 본다, 8.4).
    pub(crate) fn mark_recovered(&mut self, path: Option<PathBuf>, read_only: Option<ReadOnly>) {
        self.file_id = path.as_deref().and_then(crate::files::FileId::of);
        self.path = path;
        self.read_only = read_only;
        self.dirty = true;
        self.recovered = true;
    }

    /// 열거나 저장한 때의 파일 정체.
    pub(crate) fn file_id(&self) -> Option<crate::files::FileId> {
        self.file_id
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
        self.file_id = crate::files::FileId::of(&path);
        self.path = Some(path);
        self.read_only = read_only;
    }

    /// `path`에 저장했다. 바뀜을 끄고 읽기 전용을 푼다(새 파일은 이 앱의 형식 버전이다).
    pub(crate) fn mark_saved(&mut self, path: PathBuf) {
        // 저장은 임시 파일 + rename이라 정체(inode)가 바뀐다. 새 파일의 것을 남긴다.
        self.file_id = crate::files::FileId::of(&path);
        self.path = Some(path);
        self.read_only = None;
        self.dirty = false;
        self.recovered = false;
    }
}
