use std::collections::VecDeque;

use crate::edit::Op;
use crate::{Edit, ModelError, Project};

/// 연속 같은 조작을 한 undo 단위로 묶기 위한 키(EDT-1, REQ 5.7 「되돌리기 묶음」).
///
/// 어떤 조작이 「같은 조작」인지는 앱의 Action이 정한다. 히스토리는 키가 같은지만 본다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoalesceKey(pub u64);

/// undo 한 단위. `ops`는 이 단위를 되돌리거나 다시 할 때 차례로 적용할 연산이다.
#[derive(Debug, Clone)]
struct Entry {
    ops: Vec<Op>,
    key: Option<CoalesceKey>,
    bytes: usize,
}

/// 무제한 되돌리기/다시 실행(EDT-1). 개수 제한은 없고, 추정 메모리가 상한을 넘으면
/// 가장 오래된 undo 단위부터 버린다.
///
/// 모든 연산은 원자적이다. 실패하면 프로젝트와 히스토리 모두 호출 전과 같다.
///
/// 역편집은 히스토리 안에만 있다. 역편집은 자기를 만든 상태에만 맞으므로, 히스토리는 crate
/// 안에서만 쓰고 공개 API는 프로젝트와 히스토리를 한 값에 묶은 [`crate::Document`]다.
#[derive(Debug, Clone)]
pub(crate) struct History {
    undo: VecDeque<Entry>,
    redo: Vec<Entry>,
    limit_bytes: usize,
    used_bytes: usize,
    /// 참이면 다음 `apply`가 직전 단위와 묶이지 않는다.
    sealed: bool,
    /// 상한 때문에 지금까지 버린 undo 단위 수
    dropped: u64,
}

impl History {
    #[must_use]
    pub(crate) fn new(limit_bytes: usize) -> Self {
        History {
            undo: VecDeque::new(),
            redo: Vec::new(),
            limit_bytes,
            used_bytes: 0,
            sealed: true,
            dropped: 0,
        }
    }

    /// 편집 묶음을 undo 한 단위로 적용한다. 새 편집은 redo를 비운다.
    ///
    /// `coalesce`가 직전 단위의 키와 같고, 그 사이에 다른 단위·undo·redo·[`History::seal`]이
    /// 없었으면 직전 단위에 합친다. 실패한 `apply`는 히스토리를 바꾸지 않으므로 묶기를 끊지
    /// 않는다. 실패한 키 입력이 이어지는 조작을 둘로 나누지 않게 하기 위해서다.
    ///
    /// # Errors
    ///
    /// 묶음의 편집 하나라도 실패하면 그 오류. 프로젝트와 히스토리는 바뀌지 않는다.
    pub(crate) fn apply(
        &mut self,
        project: &mut Project,
        edits: Vec<Edit>,
        coalesce: Option<CoalesceKey>,
    ) -> Result<(), ModelError> {
        if edits.is_empty() {
            return Ok(());
        }
        let inverses = apply_all(project, edits.into_iter().map(Op::from).collect())?;
        let bytes = total_bytes(&inverses);
        self.used_bytes -= self.redo.drain(..).map(|e| e.bytes).sum::<usize>();
        match self.undo.back_mut() {
            Some(top) if !self.sealed && coalesce.is_some() && top.key == coalesce => {
                // 새 편집을 먼저 되돌려야 하므로 새 역편집을 앞에 붙인다.
                let mut merged = inverses;
                merged.append(&mut top.ops);
                top.ops = merged;
                top.bytes += bytes;
            }
            _ => self.undo.push_back(Entry {
                ops: inverses,
                key: coalesce,
                bytes,
            }),
        }
        self.used_bytes += bytes;
        self.sealed = false;
        self.enforce_limit();
        Ok(())
    }

    /// 되돌린다. 되돌릴 것이 없으면 `Ok(false)`.
    ///
    /// # Errors
    ///
    /// 역편집이 맞지 않으면(히스토리 밖에서 프로젝트를 고친 경우) 그 오류. 프로젝트와 히스토리는
    /// 바뀌지 않는다.
    pub(crate) fn undo(&mut self, project: &mut Project) -> Result<bool, ModelError> {
        let Some(entry) = self.undo.pop_back() else {
            return Ok(false);
        };
        match apply_all(project, entry.ops.clone()) {
            Ok(forward) => {
                let bytes = total_bytes(&forward);
                self.used_bytes = self.used_bytes - entry.bytes + bytes;
                self.redo.push(Entry {
                    ops: forward,
                    key: None,
                    bytes,
                });
                self.sealed = true;
                self.enforce_limit();
                Ok(true)
            }
            Err(err) => {
                self.undo.push_back(entry);
                Err(err)
            }
        }
    }

    /// 다시 실행한다. 다시 할 것이 없으면 `Ok(false)`.
    ///
    /// # Errors
    ///
    /// [`History::undo`]와 같다.
    pub(crate) fn redo(&mut self, project: &mut Project) -> Result<bool, ModelError> {
        let Some(entry) = self.redo.pop() else {
            return Ok(false);
        };
        match apply_all(project, entry.ops.clone()) {
            Ok(inverses) => {
                let bytes = total_bytes(&inverses);
                self.used_bytes = self.used_bytes - entry.bytes + bytes;
                self.undo.push_back(Entry {
                    ops: inverses,
                    key: None,
                    bytes,
                });
                self.sealed = true;
                self.enforce_limit();
                Ok(true)
            }
            Err(err) => {
                self.redo.push(entry);
                Err(err)
            }
        }
    }

    /// 상한을 넘으면 가장 오래된 undo 단위부터 버린다. 가장 최근 undo 단위 하나는 상한보다 커도
    /// 남긴다(방금 한 편집을 되돌릴 수 없으면 안 되기 때문이다). redo 단위는 지우지 않는다.
    fn enforce_limit(&mut self) {
        while self.used_bytes > self.limit_bytes && self.undo.len() > 1 {
            if let Some(oldest) = self.undo.pop_front() {
                self.used_bytes -= oldest.bytes;
                self.dropped = self.dropped.saturating_add(1);
            }
        }
    }

    /// 상한 때문에 지금까지 버린 undo 단위 수.
    pub(crate) const fn dropped(&self) -> u64 {
        self.dropped
    }

    /// 다음 `apply`가 직전 단위와 묶이지 않게 한다(모드 전환, 다른 대상 선택 등).
    pub(crate) fn seal(&mut self) {
        self.sealed = true;
    }

    #[must_use]
    pub(crate) fn undo_len(&self) -> usize {
        self.undo.len()
    }

    #[must_use]
    pub(crate) fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// undo·redo 스택이 차지하는 추정 바이트.
    #[must_use]
    pub(crate) fn used_bytes(&self) -> usize {
        self.used_bytes
    }
}

/// 연산을 차례로 적용하고, 되돌릴 때 적용할 순서의 역연산을 돌려준다. 하나라도 실패하면
/// 이미 적용한 것을 거꾸로 되돌려 프로젝트를 호출 전 상태로 둔다.
fn apply_all(project: &mut Project, ops: Vec<Op>) -> Result<Vec<Op>, ModelError> {
    let mut inverses = Vec::with_capacity(ops.len());
    for op in ops {
        match op.apply(project) {
            Ok(inverse) => inverses.push(inverse),
            Err(err) => {
                // 방금 적용한 연산의 역이라 실패하지 않아야 한다. 역연산은 히스토리 안에서만
                // 만들어지므로 위조된 역연산이 끼어들 수 없다.
                for inverse in inverses.into_iter().rev() {
                    if inverse.apply(project).is_err() {
                        return Err(ModelError::HistoryInconsistent);
                    }
                }
                return Err(err);
            }
        }
    }
    inverses.reverse();
    Ok(inverses)
}

fn total_bytes(ops: &[Op]) -> usize {
    ops.iter().map(Op::approx_bytes).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Channel, Clip, ClipId, Note, Pitch, Tick, Track, TrackColor, TrackId, Velocity};

    const KEY: Option<CoalesceKey> = Some(CoalesceKey(1));

    fn project() -> (Project, TrackId, ClipId) {
        let mut p = Project::new();
        let track = Track::new(
            "Lead",
            Channel::new(1).unwrap(),
            TrackColor::new(0).unwrap(),
        );
        let t = track.id();
        p.push_track(track).unwrap();
        let id = p
            .track_by_id_mut(t)
            .unwrap()
            .insert_clip(Clip::new(Tick::ZERO, 96_000).unwrap())
            .unwrap();
        (p, t, id)
    }

    fn n(start: u64, length: u32) -> Note {
        Note::new(
            Tick::new(start),
            length,
            Pitch::new(60).unwrap(),
            Velocity::new(1),
        )
        .unwrap()
    }

    fn insert(t: TrackId, clip: ClipId, start: u64) -> Vec<Edit> {
        vec![Edit::InsertNote {
            track: t,
            clip,
            note: n(start, 10),
        }]
    }

    fn mv(t: TrackId, clip: ClipId, to: u64) -> Vec<Edit> {
        vec![Edit::MoveClip {
            track: t,
            id: clip,
            to: Tick::new(to),
        }]
    }

    #[test]
    fn history_new_edit_after_undo_clears_redo() {
        let (mut p, t, c) = project();
        let mut h = History::new(usize::MAX);
        h.apply(&mut p, insert(t, c, 0), None).unwrap();
        h.apply(&mut p, insert(t, c, 10), None).unwrap();
        assert!(h.undo(&mut p).unwrap());
        assert_eq!(h.redo_len(), 1);
        h.apply(&mut p, insert(t, c, 20), None).unwrap();
        assert_eq!(h.redo_len(), 0);
        assert!(!h.redo(&mut p).unwrap());
    }

    #[test]
    fn history_coalesced_edits_undo_at_once() {
        let (mut p, t, c) = project();
        let initial = p.clone();
        let mut h = History::new(usize::MAX);
        for i in 1..=5 {
            h.apply(&mut p, mv(t, c, i * 960), KEY).unwrap();
        }
        assert_eq!(h.undo_len(), 1);
        assert!(h.undo(&mut p).unwrap());
        assert_eq!(p, initial);
        assert!(h.redo(&mut p).unwrap());
        assert_eq!(
            p.track(t).unwrap().clip(c).unwrap().start(),
            Tick::new(4800)
        );
    }

    #[test]
    fn history_coalesce_breaks_on_other_key_seal_or_undo() {
        let (mut p, t, c) = project();
        let mut h = History::new(usize::MAX);
        h.apply(&mut p, mv(t, c, 960), KEY).unwrap();
        h.apply(&mut p, mv(t, c, 1920), Some(CoalesceKey(2)))
            .unwrap(); // 다른 키
        assert_eq!(h.undo_len(), 2);
        h.apply(&mut p, mv(t, c, 2880), Some(CoalesceKey(2)))
            .unwrap(); // 같은 키 → 묶임
        assert_eq!(h.undo_len(), 2);
        h.seal();
        h.apply(&mut p, mv(t, c, 3840), Some(CoalesceKey(2)))
            .unwrap(); // seal 뒤 → 안 묶임
        assert_eq!(h.undo_len(), 3);
        h.undo(&mut p).unwrap();
        h.apply(&mut p, mv(t, c, 4800), Some(CoalesceKey(2)))
            .unwrap(); // undo 뒤 → 안 묶임
        assert_eq!(h.undo_len(), 3);
        h.apply(&mut p, mv(t, c, 5760), None).unwrap(); // 키 없음 → 안 묶임
        h.apply(&mut p, mv(t, c, 6720), None).unwrap();
        assert_eq!(h.undo_len(), 5);
    }

    #[test]
    fn history_failed_apply_does_not_break_coalescing() {
        // 실패한 apply는 히스토리를 바꾸지 않으므로 묶기를 끊지 않는다(의도, 문서 참고).
        let (mut p, t, c) = project();
        let mut h = History::new(usize::MAX);
        h.apply(&mut p, mv(t, c, 960), KEY).unwrap();
        assert!(h.apply(&mut p, insert(t, c, 99_999), KEY).is_err());
        h.apply(&mut p, mv(t, c, 1920), KEY).unwrap();
        assert_eq!(h.undo_len(), 1);
    }

    #[test]
    fn history_split_redo_reuses_right_clip_id() {
        let (mut p, t, c) = project();
        let mut h = History::new(usize::MAX);
        h.apply(
            &mut p,
            vec![Edit::SplitClip {
                track: t,
                id: c,
                at: Tick::new(960),
            }],
            None,
        )
        .unwrap();
        let right = p
            .track(t)
            .unwrap()
            .clips()
            .map(|(id, _)| id)
            .find(|&id| id != c)
            .unwrap();
        h.apply(&mut p, insert(t, right, 5), None).unwrap();
        let after = p.clone();
        h.undo(&mut p).unwrap();
        h.undo(&mut p).unwrap();
        assert!(h.redo(&mut p).unwrap());
        assert!(
            h.redo(&mut p).unwrap(),
            "redo가 같은 오른쪽 id를 써야 노트 삽입이 이어진다"
        );
        assert_eq!(p, after);
    }

    #[test]
    fn history_memory_limit_drops_oldest() {
        let (mut p, t, c) = project();
        let one = Op::from(insert(t, c, 0).remove(0)).approx_bytes();
        let mut h = History::new(one * 3);
        for i in 0..10 {
            h.apply(&mut p, insert(t, c, i * 10), None).unwrap();
        }
        assert!(
            h.used_bytes() <= one * 3,
            "{} > {}",
            h.used_bytes(),
            one * 3
        );
        assert!(h.undo_len() >= 1 && h.undo_len() <= 3);
        // 가장 최근 단위는 남아 있다.
        h.undo(&mut p).unwrap();
        assert_eq!(p.track(t).unwrap().clip(c).unwrap().notes().len(), 9);
    }

    #[test]
    fn history_memory_limit_keeps_latest_even_if_too_large() {
        let (mut p, t, c) = project();
        let mut h = History::new(1);
        h.apply(&mut p, insert(t, c, 0), None).unwrap();
        h.apply(&mut p, insert(t, c, 10), None).unwrap();
        assert_eq!(h.undo_len(), 1);
        assert!(h.undo(&mut p).unwrap());
        assert_eq!(p.track(t).unwrap().clip(c).unwrap().notes().len(), 1);
    }

    #[test]
    fn history_undo_after_outside_mutation_fails_without_change() {
        let (mut p, t, c) = project();
        let mut h = History::new(usize::MAX);
        h.apply(&mut p, insert(t, c, 0), None).unwrap();
        // 히스토리 밖에서 그 노트를 지운다.
        let note = p.track(t).unwrap().clip(c).unwrap().notes()[0].clone();
        p.track_by_id_mut(t).unwrap().remove_note(c, &note).unwrap();
        let before = p.clone();
        assert!(h.undo(&mut p).is_err());
        assert_eq!(p, before);
        assert_eq!((h.undo_len(), h.redo_len()), (1, 0));
    }

    #[test]
    fn history_remove_tail_note_then_undo_restores_it() {
        // 분할로 생긴 숨김 꼬리 노트는 insert_note로 다시 넣을 수 없다. 삭제의 역은
        // 히스토리 안의 RestoreNote라서 undo가 되살릴 수 있어야 한다.
        let (mut p, t, c) = project();
        let mut h = History::new(usize::MAX);
        let tail = n(900, 200);
        h.apply(
            &mut p,
            vec![Edit::InsertNote {
                track: t,
                clip: c,
                note: tail.clone(),
            }],
            None,
        )
        .unwrap();
        h.apply(
            &mut p,
            vec![Edit::SplitClip {
                track: t,
                id: c,
                at: Tick::new(960),
            }],
            None,
        )
        .unwrap();
        let clip = p.track(t).unwrap().clip(c).unwrap();
        assert!(clip.notes()[0].end() > clip.length());
        let before = p.clone();
        h.apply(
            &mut p,
            vec![Edit::RemoveNote {
                track: t,
                clip: c,
                note: tail,
            }],
            None,
        )
        .unwrap();
        assert!(h.undo(&mut p).unwrap());
        assert_eq!(p, before);
    }

    #[test]
    fn history_empty_apply_adds_nothing() {
        let (mut p, ..) = project();
        let mut h = History::new(usize::MAX);
        h.apply(&mut p, Vec::new(), None).unwrap();
        assert_eq!(h.undo_len(), 0);
        assert!(!h.undo(&mut p).unwrap());
    }

    #[test]
    fn history_undo_enforces_limit() {
        // undo 뒤에도 상한을 적용한다(S0-T4 리뷰 1 제안 8).
        let (mut p, t, c) = project();
        let one = Op::from(insert(t, c, 0).remove(0)).approx_bytes();
        let mut h = History::new(one * 4);
        for i in 0..4 {
            h.apply(&mut p, insert(t, c, i * 10), None).unwrap();
        }
        h.undo(&mut p).unwrap();
        assert!(h.used_bytes() <= one * 4);
    }
}
