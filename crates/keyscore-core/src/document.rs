use std::collections::HashSet;

use crate::history::{CoalesceKey, History};
use crate::{Edit, ModelError, Project};

/// 편집 중인 곡 하나: 프로젝트와 그 undo 히스토리(EDT-1).
///
/// 프로젝트를 바꾸는 길은 [`Document::apply`]·[`Document::undo`]·[`Document::redo`]뿐이다.
/// `&mut Project`를 내주지 않는다. 히스토리의 역편집은 「자기를 만든 상태」에만 맞으므로,
/// 프로젝트를 히스토리 밖에서 고치거나 히스토리를 따로 복제·교체할 수 있으면 undo가 엉뚱한
/// 결과를 `Ok`로 낼 수 있다(S0-T4 리뷰 2 지적 1–3). 프로젝트와 히스토리를 한 값에 묶어
/// 그런 경로를 없앤다. `clone`은 둘을 함께 복제한다.
///
/// 밖에서 프로젝트를 고칠 수 없다:
///
/// ```compile_fail
/// let mut doc = keyscore_core::Document::new(keyscore_core::Project::new(), 1 << 20).unwrap();
/// let _: &mut keyscore_core::Project = doc.project_mut();
/// ```
///
/// 역편집 전용 연산은 공개 [`Edit`]의 변형이 아니다(있으면 아래 패턴이 컴파일된다):
///
/// ```compile_fail
/// fn is_unsplit(e: &keyscore_core::Edit) -> bool {
///     matches!(e, keyscore_core::Edit::UnsplitClip { .. })
/// }
/// ```
///
/// ```compile_fail
/// fn is_restore(e: &keyscore_core::Edit) -> bool {
///     matches!(e, keyscore_core::Edit::RestoreNote { .. })
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Document {
    project: Project,
    history: History,
}

impl Document {
    /// 프로젝트를 넘겨받아 편집을 시작한다. `limit_bytes`는 히스토리의 메모리 상한이다.
    ///
    /// # Errors
    ///
    /// 프로젝트 안에 같은 [`crate::TrackId`]의 트랙이 둘 이상이면 [`ModelError::DuplicateTrackId`],
    /// 같은 [`crate::ClipId`]의 클립이 둘 이상이면 [`ModelError::DuplicateClipId`]. `Project`는
    /// `track_mut`로 트랙을 통째로 바꿀 수 있어서(파일 읽기 등 구성 단계), 편집을 시작할 때 검사한다.
    pub fn new(project: Project, limit_bytes: usize) -> Result<Self, ModelError> {
        let mut tracks = HashSet::new();
        let mut clips = HashSet::new();
        for track in project.tracks() {
            if !tracks.insert(track.id()) {
                return Err(ModelError::DuplicateTrackId);
            }
            for (id, _) in track.clips() {
                if !clips.insert(id) {
                    return Err(ModelError::DuplicateClipId);
                }
            }
        }
        Ok(Document {
            project,
            history: History::new(limit_bytes),
        })
    }

    #[must_use]
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// 편집을 끝내고 프로젝트를 돌려받는다. 히스토리는 버려진다.
    #[must_use]
    pub fn into_project(self) -> Project {
        self.project
    }

    /// 편집 묶음을 undo 한 단위로 적용한다. 규칙은 히스토리와 같다: 원자적이고, 같은
    /// `coalesce` 키의 연속 편집은 한 단위로 묶이며, 새 편집은 redo를 비운다.
    ///
    /// # Errors
    ///
    /// 편집 하나라도 실패하면 그 오류. 문서는 바뀌지 않는다.
    pub fn apply(
        &mut self,
        edits: Vec<Edit>,
        coalesce: Option<CoalesceKey>,
    ) -> Result<(), ModelError> {
        self.history.apply(&mut self.project, edits, coalesce)
    }

    /// 되돌린다. 되돌릴 것이 없으면 `Ok(false)`.
    ///
    /// # Errors
    ///
    /// 정상 경로에서는 실패하지 않는다. 실패하면 [`ModelError::HistoryInconsistent`] 등이고
    /// 문서는 바뀌지 않는다.
    pub fn undo(&mut self) -> Result<bool, ModelError> {
        self.history.undo(&mut self.project)
    }

    /// 다시 실행한다. 다시 할 것이 없으면 `Ok(false)`.
    ///
    /// # Errors
    ///
    /// [`Document::undo`]와 같다.
    pub fn redo(&mut self) -> Result<bool, ModelError> {
        self.history.redo(&mut self.project)
    }

    /// 다음 `apply`가 직전 단위와 묶이지 않게 한다(모드 전환, 다른 대상 선택 등).
    pub fn seal(&mut self) {
        self.history.seal();
    }

    #[must_use]
    pub fn undo_len(&self) -> usize {
        self.history.undo_len()
    }

    #[must_use]
    pub fn redo_len(&self) -> usize {
        self.history.redo_len()
    }

    /// 히스토리가 차지하는 추정 바이트.
    #[must_use]
    pub fn history_bytes(&self) -> usize {
        self.history.used_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Channel, Clip, Tick, Track, TrackColor};

    fn track(name: &str) -> Track {
        Track::new(name, Channel::new(1).unwrap(), TrackColor::new(0).unwrap())
    }

    #[test]
    fn document_new_rejects_duplicate_track_id_made_through_track_mut() {
        // 리뷰 2 시도 1: track_mut로 복제본을 대입하면 Project 안에서는 중복이 생길 수 있다.
        let mut p = Project::new();
        let mut a = track("A");
        a.insert_clip(Clip::new(Tick::ZERO, 10).unwrap()).unwrap();
        p.push_track(a.clone()).unwrap();
        p.push_track(track("B")).unwrap();
        *p.track_mut(1).unwrap() = a;
        assert_eq!(
            Document::new(p, 1 << 20).unwrap_err(),
            ModelError::DuplicateTrackId
        );
    }

    #[test]
    fn document_apply_undo_redo_round_trip() {
        let mut p = Project::new();
        let t = track("A");
        let tid = t.id();
        p.push_track(t).unwrap();
        let mut doc = Document::new(p, 1 << 20).unwrap();
        let initial = doc.project().clone();
        doc.apply(
            vec![Edit::InsertClip {
                track: tid,
                clip: Box::new(Clip::new(Tick::ZERO, 10).unwrap()),
            }],
            None,
        )
        .unwrap();
        let after = doc.project().clone();
        assert!(doc.undo().unwrap());
        assert_eq!(doc.project(), &initial);
        assert!(doc.redo().unwrap());
        assert_eq!(doc.project(), &after);
        assert_eq!((doc.undo_len(), doc.redo_len()), (1, 0));
        assert!(doc.history_bytes() > 0);
        assert_eq!(doc.into_project(), after);
    }
}
