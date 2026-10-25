//! 삭제·붙여넣기·복제의 편집 계산(S3-T3a, REQ 5.8, 7.5, 5.10). GPUI와 무관한 순수 함수다.
//!
//! 클립 연장 규칙은 core `transform::insert_notes`·`clip_for_notes`(S0-T7과 같은 마디 단위 연장)를 따른다.

use std::collections::BTreeSet;

use keyscore_core::transform::{clip_for_notes, insert_notes};
use keyscore_core::{Clip, ClipId, Edit, ModelError, Note, Project, Tick, Track, TrackId};

use crate::clipboard::ClipNotes;

/// 노트를 넣은 클립.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacedClip {
    /// 있던 클립.
    Existing(ClipId),
    /// 틈에 새로 만든 클립(이 시작 틱). id는 적용한 뒤에 정해진다.
    New { start: Tick },
}

/// 넣기 계산 결과: 적용할 편집, 넣은 노트(그 클립 기준 시작), 넣은 클립.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placed {
    pub edits: Vec<Edit>,
    pub notes: Vec<Note>,
    pub clip: PlacedClip,
}

/// 선택 값마다 클립의 같은 노트 전부(같은 값의 노트는 화면에서 구별되지 않는다). 시작 순.
#[must_use]
pub fn targets(clip: &Clip, values: &BTreeSet<Note>) -> Vec<Note> {
    clip.notes()
        .iter()
        .filter(|n| values.contains(*n))
        .cloned()
        .collect()
}

/// `notes`(그 클립의 노트)를 지우는 편집.
#[must_use]
pub fn plan_delete(track: TrackId, clip: ClipId, notes: &[Note]) -> Vec<Edit> {
    notes
        .iter()
        .map(|n| Edit::RemoveNote {
            track,
            clip,
            note: n.clone(),
        })
        .collect()
}

/// 클립보드 내용의 틱 0을 절대 틱 `at`에 두는 편집. `at`을 품은 클립이 있으면 그 클립에(끝을 넘으면 마디 단위로
/// 늘림), 틈이면 새 클립에 넣는다.
///
/// # Errors
///
/// 트랙이 없거나, 늘린·새 클립이 다른 클립과 겹치거나, 틱이 넘치면 [`ModelError`].
pub fn plan_paste(
    project: &Project,
    track: TrackId,
    at: Tick,
    content: &ClipNotes,
) -> Result<Placed, ModelError> {
    let t = project.track(track).ok_or(ModelError::UnknownTrack)?;
    let host = t
        .clips()
        .find(|(_, c)| c.start() <= at && at.get() < c.end())
        .map(|(id, c)| (id, c.start().get()));
    if let Some((id, start)) = host {
        let notes = content.shifted(at.get() - start)?;
        let edits = insert_notes(project, track, id, &notes)?;
        return Ok(Placed {
            edits,
            notes,
            clip: PlacedClip::Existing(id),
        });
    }
    let absolute = content.shifted(at.get())?;
    let edit = clip_for_notes(project, track, &absolute, at)?;
    let Edit::InsertClip { clip, .. } = &edit else {
        return Err(ModelError::UnknownClip);
    };
    let start = clip.start();
    let notes = clip.notes().to_vec();
    Ok(Placed {
        edits: vec![edit],
        notes,
        clip: PlacedClip::New { start },
    })
}

/// 복제·반복 간격: `notes`의 폭(가장 늦은 끝 − 가장 이른 시작). REQ 7.5 「선택의 길이만큼 뒤에」, 5.8 「바로 뒤에」를
/// 글자대로 따른다(S3-T3a 리뷰 1 지적 1). 격자에 맞춰 올리는 규칙은 결정 요청
/// `docs/decisions/2026-10-26-duplicate-offset-grid.md`에 있다.
#[must_use]
pub fn duplicate_offset(notes: &[Note]) -> u64 {
    let first = notes.iter().map(|n| n.start().get()).min().unwrap_or(0);
    let last = notes.iter().map(Note::end).max().unwrap_or(first);
    (last - first).max(1)
}

/// `notes`(그 클립의 노트)를 같은 클립 안에 [`duplicate_offset`]만큼 뒤로 복사하는 편집.
///
/// # Errors
///
/// 늘린 클립이 다음 클립과 겹치거나 틱이 넘치면 [`ModelError`].
pub fn plan_duplicate(
    project: &Project,
    track: TrackId,
    clip: ClipId,
    notes: &[Note],
) -> Result<Placed, ModelError> {
    let offset = duplicate_offset(notes);
    let copies = notes
        .iter()
        .map(|n| n.with_start(n.start().checked_add(offset)?))
        .collect::<Result<Vec<_>, _>>()?;
    let edits = insert_notes(project, track, clip, &copies)?;
    Ok(Placed {
        edits,
        notes: copies,
        clip: PlacedClip::Existing(clip),
    })
}

/// 정렬된 두 노트 목록의 다중집합 차 `after − before`.
fn new_notes(before: &[Note], after: &[Note]) -> Vec<Note> {
    let mut out = Vec::new();
    let mut i = 0;
    for n in after {
        while i < before.len() && before[i] < *n {
            i += 1;
        }
        if i < before.len() && before[i] == *n {
            i += 1;
        } else {
            out.push(n.clone());
        }
    }
    out
}

/// undo·redo 뒤 선택: 트랙의 클립마다 앞뒤 노트를 비교해 새로 나타난 노트가 모두 한 클립에 있으면 (그 클립, 노트).
/// 나타난 노트가 없거나 여러 클립에 있으면 `None`.
#[must_use]
pub fn appeared(before: &Track, after: &Track) -> Option<(ClipId, BTreeSet<Note>)> {
    let mut found: Option<(ClipId, BTreeSet<Note>)> = None;
    for (id, clip) in after.clips() {
        let old = before.clip(id).map_or(&[][..], Clip::notes);
        let new = new_notes(old, clip.notes());
        if new.is_empty() {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some((id, new.into_iter().collect()));
    }
    found
}
