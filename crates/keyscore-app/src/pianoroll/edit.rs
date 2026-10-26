//! 노트 편집 계산(S3-T3a 삭제·붙여넣기·복제, S3-T3b 분할·합치기·길이·음높이·퀀타이즈·레가토, REQ 5.8, 7.5,
//! 5.10). GPUI와 무관한 순수 함수다. 규칙은 `docs/reports/s3-t3a/plan.md`·`docs/reports/s3-t3b/plan.md`
//! 「끝의 정의」에 있다.
//!
//! 클립 연장 규칙은 core `transform::insert_notes`·`clip_for_notes`·`map_notes`(S0-T7과 같은 마디 단위 연장)를
//! 따른다.

use std::collections::BTreeSet;

use keyscore_core::transform::{QuantizeSettings, clip_for_notes, insert_notes};
use keyscore_core::{Clip, ClipId, Edit, ModelError, Note, Pitch, Project, Tick, Track, TrackId};

use crate::actions::args::LengthCode;
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

// ---------- S3-T3b ----------

/// 기본 길이(4분음표, REQ 5.6 「`0`은 길이를 기본값으로」). 기본 길이 설정은 S3-T4가 만든다.
pub const DEFAULT_LENGTH: u32 = 960;

/// 길이 코드의 틱(REQ 5.6 표, PPQ 960). 점음표는 3/2.
#[must_use]
pub fn length_ticks(code: LengthCode, dotted: bool) -> u32 {
    let _ = (code, dotted);
    todo!()
}

/// 바꾸기 계산 결과: 적용할 편집과 (원래, 바뀐) 노트 짝. 짝은 커서가 따라갈 1:1 바꾸기(길이·음높이·퀀타이즈·
/// 레가토)에만 채운다. 분할·합치기는 비운다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Changed {
    pub edits: Vec<Edit>,
    pub moved: Vec<(Note, Note)>,
}

/// 분할(`/`): `notes` 가운데 절대 틱 `at`이 시작과 끝 사이에 있는 노트를 둘로 나눈다. 없으면 `Ok(None)`.
///
/// # Errors
///
/// 트랙·클립·노트가 없으면 [`ModelError`].
pub fn plan_split(
    project: &Project,
    track: TrackId,
    clip: ClipId,
    notes: &[Note],
    at: Tick,
) -> Result<Option<Changed>, ModelError> {
    let _ = (project, track, clip, notes, at);
    todo!()
}

/// 합치기(`Cmd+J`): 같은 음높이가 둘 이상인 묶음마다 노트 하나로. 묶음이 없으면 `Ok(None)`.
///
/// # Errors
///
/// 트랙·클립·노트가 없거나 합친 길이가 `u32`를 넘으면 [`ModelError`].
pub fn plan_join(
    project: &Project,
    track: TrackId,
    clip: ClipId,
    notes: &[Note],
) -> Result<Option<Changed>, ModelError> {
    let _ = (project, track, clip, notes);
    todo!()
}

/// 길이 직접 지정(`1`–`7`, `0`): 길이를 `length`로. 이미 그 길이인 노트는 뺀다. 바뀌는 노트가 없으면 `Ok(None)`.
///
/// # Errors
///
/// 클립을 늘려야 하는데 다음 클립과 겹치거나 참조가 맞지 않으면 [`ModelError`].
pub fn plan_set_length(
    project: &Project,
    track: TrackId,
    clip: ClipId,
    notes: &[Note],
    length: u32,
) -> Result<Option<Changed>, ModelError> {
    let _ = (project, track, clip, notes, length);
    todo!()
}

/// 음높이 바꾸기(`e`): 노트 하나의 음높이를 `pitch`로. 같으면 `Ok(None)`.
///
/// # Errors
///
/// 참조가 맞지 않으면 [`ModelError`].
pub fn plan_set_pitch(
    project: &Project,
    track: TrackId,
    clip: ClipId,
    note: &Note,
    pitch: Pitch,
) -> Result<Option<Changed>, ModelError> {
    let _ = (project, track, clip, note, pitch);
    todo!()
}

/// 레가토(`~`): 노트마다 끝을 같은 클립에서 시작이 더 늦은 노트의 가장 이른 시작에 맞춘다. 바뀌는 노트가 없으면
/// `Ok(None)`.
///
/// # Errors
///
/// 참조가 맞지 않으면 [`ModelError`].
pub fn plan_legato(
    project: &Project,
    track: TrackId,
    clip: ClipId,
    notes: &[Note],
) -> Result<Option<Changed>, ModelError> {
    let _ = (project, track, clip, notes);
    todo!()
}

/// 레가토 되돌리기: `pairs`(원래, 바뀐)의 바뀐 노트를 원래 노트로. 빈 짝이면 `Ok(None)`.
///
/// # Errors
///
/// 바뀐 노트가 클립에 없으면 [`ModelError`].
pub fn plan_restore(
    project: &Project,
    track: TrackId,
    clip: ClipId,
    pairs: &[(Note, Note)],
) -> Result<Option<Changed>, ModelError> {
    let _ = (project, track, clip, pairs);
    todo!()
}

/// 퀀타이즈(`q`): core `transform::quantize`. 바뀌는 노트가 없으면 `Ok(None)`.
///
/// # Errors
///
/// core와 같다(클립 시작 앞, 다음 클립과 겹침).
pub fn plan_quantize(
    project: &Project,
    track: TrackId,
    clip: ClipId,
    notes: &[Note],
    settings: &QuantizeSettings,
) -> Result<Option<Changed>, ModelError> {
    let _ = (project, track, clip, notes, settings);
    todo!()
}

/// 편집 뒤 선택: `targets` 가운데 편집 뒤에도 남은 노트와 편집으로 새로 나타난 노트.
#[must_use]
pub fn selection_after(before: &Clip, after: &Clip, targets: &[Note]) -> BTreeSet<Note> {
    let _ = (before, after, targets);
    todo!()
}
