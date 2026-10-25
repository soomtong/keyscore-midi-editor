//! 선택한 노트에 대한 일괄 연산: 퀀타이즈(EDT-2), 조옮김·시간 이동·길이 스케일(EDT-3).
//!
//! 함수는 프로젝트를 바꾸지 않고 [`Edit`] 묶음을 돌려준다. 호출자가 [`crate::Document::apply`]로
//! 한 undo 단위로 적용한다. 규칙은 `docs/decisions/2026-10-12-quantize-and-transform-rules.md`를
//! 따른다: 퀀타이즈는 시작만 옮기고, 그리드는 곡의 마디 기준이며, 결과가 클립 끝을 넘으면 클립을
//! 마디 단위로 늘리고(다음 클립과 겹치면 거부), 클립 시작 앞으로 나가면 거부한다.

use std::collections::HashMap;

use crate::{Clip, ClipId, Edit, ModelError, Note, Pitch, Project, Tick, TrackId};

/// 선택 항목 하나. 모델 불변식을 담는 타입이 아니라 「무엇을 고를지」를 담는 값이다. 잘못된
/// 참조는 연산에서 거부된다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteRef {
    pub track: TrackId,
    pub clip: ClipId,
    pub note: Note,
}

/// 퀀타이즈 그리드(EDT-2: 1/4–1/64).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grid {
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
    SixtyFourth,
}

/// 퀀타이즈 설정. 강도 0–100%, 스윙 0–75%.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantizeSettings {
    grid: Grid,
    triplet: bool,
    strength: u8,
    swing: u8,
}

impl QuantizeSettings {
    /// # Errors
    ///
    /// 강도가 100을, 스윙이 75를 넘으면 [`ModelError::QuantizeSettingOutOfRange`].
    pub fn new(grid: Grid, triplet: bool, strength: u8, swing: u8) -> Result<Self, ModelError> {
        if strength > 100 || swing > 75 {
            return Err(ModelError::QuantizeSettingOutOfRange { strength, swing });
        }
        Ok(QuantizeSettings {
            grid,
            triplet,
            strength,
            swing,
        })
    }

    /// 그리드 한 칸의 틱 수. 셋잇단이면 2/3.
    #[must_use]
    pub fn grid_ticks(&self) -> u64 {
        let base = match self.grid {
            Grid::Quarter => 960,
            Grid::Eighth => 480,
            Grid::Sixteenth => 240,
            Grid::ThirtySecond => 120,
            Grid::SixtyFourth => 60,
        };
        // 1/64 셋잇단도 40틱으로 나누어떨어진다.
        if self.triplet { base * 2 / 3 } else { base }
    }
}

/// 조옮김 결과를 접어 넣을 음역. 최소 한 옥타브라서 언제나 접을 수 있다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PitchRange {
    low: Pitch,
    high: Pitch,
}

impl PitchRange {
    /// # Errors
    ///
    /// `high − low < 11`이면(한 옥타브 미만) [`ModelError::PitchRangeTooNarrow`].
    pub fn new(low: Pitch, high: Pitch) -> Result<Self, ModelError> {
        if high.get() < low.get().saturating_add(11) || low > high {
            return Err(ModelError::PitchRangeTooNarrow {
                low: low.get(),
                high: high.get(),
            });
        }
        Ok(PitchRange { low, high })
    }

    /// MIDI 전체 음역 0–127.
    #[must_use]
    pub fn full() -> Self {
        PitchRange {
            low: Pitch::MIN,
            high: Pitch::MAX,
        }
    }
}

/// 퀀타이즈한다(EDT-2). 시작만 옮기고 길이는 유지한다.
///
/// # Errors
///
/// 참조가 맞지 않거나, 결과가 클립 시작 앞으로 나가거나, 클립을 늘려야 하는데 다음 클립과 겹치면 `Err`.
pub fn quantize(
    project: &Project,
    notes: &[NoteRef],
    settings: &QuantizeSettings,
) -> Result<Vec<Edit>, ModelError> {
    let grid = settings.grid_ticks();
    plan(project, notes, |_, clip, note| {
        let abs = clip.start().get() + note.start().get();
        let target = snap(project, abs, grid, settings.swing);
        // 강도: 목표 쪽으로 강도만큼만 간다. 0으로 향해 잘라 강도 100%는 목표 그대로다.
        let moved = i128::from(abs)
            + (i128::from(target) - i128::from(abs)) * i128::from(settings.strength) / 100;
        relocate(clip, note, moved)
    })
}

/// 조옮김한다(EDT-3). 음역 밖이면 옥타브 단위로 접는다.
///
/// # Errors
///
/// 참조가 맞지 않거나, 접어도 절대 피치(음높이 + 오프셋)가 MIDI 2.0 범위에 들지 않으면 `Err`.
pub fn transpose(
    project: &Project,
    notes: &[NoteRef],
    semitones: i32,
    range: PitchRange,
) -> Result<Vec<Edit>, ModelError> {
    plan(project, notes, |_, _, note| {
        // i64라 i32 끝의 반음 수도 넘치지 않는다. 접기는 반복 대신 옥타브 수를 한 번에 계산한다
        // (S0-T7 리뷰 1 지적 2: i32 덧셈 오버플로, 반음 수에 비례하는 반복).
        let (low, high) = (i64::from(range.low.get()), i64::from(range.high.get()));
        let raw = i64::from(note.pitch().get()) + i64::from(semitones);
        let pitch = if raw > high {
            raw - (raw - high + 11) / 12 * 12
        } else if raw < low {
            raw + (low - raw + 11) / 12 * 12
        } else {
            raw
        };
        // 피치 오프셋 때문에 절대 피치가 MIDI 2.0 범위를 넘으면 음역 안에서 한 옥타브 더 접는다.
        for candidate in [pitch, pitch - 12, pitch + 12] {
            if (low..=high).contains(&candidate)
                && let Ok(p) = u8::try_from(candidate)
                    .map_err(|_| ModelError::PitchOutOfRange(u8::MAX))
                    .and_then(Pitch::new)
                && let Ok(n) = note.with_pitch(p)
            {
                return Ok(n);
            }
        }
        Err(ModelError::AbsolutePitchOutOfRange)
    })
}

/// 시간 이동한다(EDT-3). 노트는 자기 클립 안에서만 움직인다.
///
/// # Errors
///
/// [`quantize`]와 같다.
pub fn shift(
    project: &Project,
    notes: &[NoteRef],
    delta_ticks: i64,
) -> Result<Vec<Edit>, ModelError> {
    plan(project, notes, |_, clip, note| {
        let abs = i128::from(clip.start().get()) + i128::from(note.start().get());
        relocate(clip, note, abs + i128::from(delta_ticks))
    })
}

/// 고른 노트마다 `f`가 준 노트로 바꾸는 편집 묶음(클립마다 `ReplaceNotes` 하나). 커서·선택 조작(S3-T2)이
/// 쓴다. 규칙은 [`quantize`]·[`shift`]와 같다: 결과가 클립 끝을 넘으면 클립을 마디 단위로 늘리고(다음 클립과
/// 겹치면 거부), 꼬리였던 노트는 시작만 클립 안이면 된다. `f`가 돌려준 오류는 그대로 전달한다.
///
/// # Errors
///
/// 참조가 맞지 않거나(같은 노트를 있는 개수보다 많이 골라도), `f`가 실패하거나, 클립을 늘려야 하는데 다음
/// 클립과 겹치면 `Err`.
pub fn map_notes(
    project: &Project,
    notes: &[NoteRef],
    f: impl FnMut(&NoteRef, &Clip, &Note) -> Result<Note, ModelError>,
) -> Result<Vec<Edit>, ModelError> {
    plan(project, notes, f)
}

/// 노트들을 클립 `clip`에 넣는 편집(붙여넣기·복제, S3-T3a). 노트 시작은 클립 기준이다. 끝이 클립을 넘는 노트가
/// 있으면 먼저 클립을 마디 단위로 늘린다(`ExtendClip`, REQ 5.10. 규칙은 [`quantize`]의 연장과 같다). 늘린 클립이
/// 다음 클립과 겹치면 거부한다. 빈 `notes`는 빈 묶음이다.
///
/// # Errors
///
/// 트랙·클립이 없으면 [`ModelError::UnknownTrack`]·[`ModelError::UnknownClip`], 다음 클립과 겹치면
/// [`ModelError::ClipOverlap`], 틱이 넘치면 [`ModelError::TickOverflow`].
pub fn insert_notes(
    project: &Project,
    track: TrackId,
    clip: ClipId,
    notes: &[Note],
) -> Result<Vec<Edit>, ModelError> {
    let t = project.track(track).ok_or(ModelError::UnknownTrack)?;
    let c = t.clip(clip).ok_or(ModelError::UnknownClip)?;
    let mut end = 0;
    for n in notes {
        end = end.max(n.end());
    }
    let mut edits = Vec::with_capacity(notes.len() + 1);
    if end > c.length() {
        let length = bar_aligned_length(project, c, end)?;
        let start = c.start().get();
        let stop = start.checked_add(length).ok_or(ModelError::TickOverflow)?;
        if t.clips()
            .any(|(other, o)| other != clip && o.start().get() < stop && start < o.end())
        {
            return Err(ModelError::ClipOverlap { start, end: stop });
        }
        edits.push(Edit::ExtendClip {
            track,
            id: clip,
            length,
        });
    }
    edits.extend(notes.iter().map(|n| Edit::InsertNote {
        track,
        clip,
        note: n.clone(),
    }));
    Ok(edits)
}

/// 틈에 노트를 넣을 새 클립 편집(`InsertClip`, REQ 5.10 「클립 밖 입력은 마디 단위로 새 클립」). 노트 시작은 절대
/// 틱이다. 클립 시작은 `at`이 든 마디의 시작(앞 클립이 그 마디 안 `at` 앞에서 끝나면 그 끝), 끝은 마지막 노트 끝을
/// 품는 마디의 끝(다음 클립이 그 앞에서 시작하면 그 시작)이다. 노트가 다른 클립과 겹치면 거부한다.
///
/// # Errors
///
/// `notes`가 비었으면 [`ModelError::NoteNotFound`], 어떤 노트가 클립 시작 앞이면
/// [`ModelError::NoteBeforeClipStart`], 트랙이 없으면
/// [`ModelError::UnknownTrack`], 겹치면 [`ModelError::ClipOverlap`], 틱이 넘치면 [`ModelError::TickOverflow`].
pub fn clip_for_notes(
    project: &Project,
    track: TrackId,
    notes: &[Note],
    at: Tick,
) -> Result<Edit, ModelError> {
    let t = project.track(track).ok_or(ModelError::UnknownTrack)?;
    if notes.is_empty() {
        return Err(ModelError::NoteNotFound);
    }
    let meter = project.meter_map();
    let bar_start = meter
        .bar_start(meter.position(at).bar())
        .unwrap_or(Tick::ZERO)
        .get();
    // 앞 클립이 그 마디 안에서 끝나면(SMF 가져오기·줄이기가 만든 마디 중간 끝) 그 끝에서 시작한다(S3-T3a 리뷰 1
    // 지적 2). 마디 시작으로 잡으면 틈인데도 앞 클립과 겹쳐 거부된다.
    let start = t
        .clips()
        .map(|(_, o)| o.end())
        .filter(|&e| e > bar_start && e <= at.get())
        .max()
        .unwrap_or(bar_start);
    let mut end = start;
    let mut relative = Vec::with_capacity(notes.len());
    for n in notes {
        let rel = n
            .start()
            .get()
            .checked_sub(start)
            .ok_or(ModelError::NoteBeforeClipStart)?;
        end = end.max(n.end());
        relative.push(n.with_start(Tick::new(rel))?);
    }
    // 끝은 마지막 노트 끝을 품는 마디의 끝. 다음 클립이 그 사이(노트 끝 뒤)에서 시작하면 그 시작까지만 늘린다.
    // 노트가 다음 클립에 닿으면 거부한다.
    let probe = Clip::new(Tick::new(start), 1)?;
    let bar_stop = start
        .checked_add(bar_aligned_length(project, &probe, (end - start).max(1))?)
        .ok_or(ModelError::TickOverflow)?;
    let stop = t
        .clips()
        .map(|(_, o)| o.start().get())
        .filter(|&s| s >= end && s < bar_stop)
        .min()
        .unwrap_or(bar_stop);
    if t.clips()
        .any(|(_, o)| o.start().get() < stop && start < o.end())
    {
        return Err(ModelError::ClipOverlap { start, end: stop });
    }
    let length = stop - start;
    let mut clip = Clip::new(Tick::new(start), length)?;
    for n in relative {
        clip.insert_note(n)?;
    }
    Ok(Edit::InsertClip {
        track,
        clip: Box::new(clip),
    })
}

/// 길이 스케일(EDT-3). 클립마다 그 클립 안의 가장 이른 선택 노트를 기준으로 시작 간격과 길이를
/// 함께 `numerator / denominator`배 한다(결정 개정). 길이는 최소 1틱이다.
///
/// # Errors
///
/// 비율이 0이면 [`ModelError::ScaleRatioInvalid`]. 그 밖은 [`quantize`]와 같다.
pub fn scale(
    project: &Project,
    notes: &[NoteRef],
    numerator: u32,
    denominator: u32,
) -> Result<Vec<Edit>, ModelError> {
    if numerator == 0 || denominator == 0 {
        return Err(ModelError::ScaleRatioInvalid {
            numerator,
            denominator,
        });
    }
    let (num, den) = (u128::from(numerator), u128::from(denominator));
    // 반올림(0.5는 올림): (x × 분자 × 2 + 분모) / (2 × 분모).
    let ratio = |x: u128| (x * num * 2 + den) / (2 * den);
    // 기준점은 클립마다 그 클립 안의 가장 이른 선택 노트다(결정 개정). 선택 전체 기준이면 여러
    // 클립에 걸친 반속이 언제나 「클립 시작 앞」으로 거부된다.
    let mut anchors: HashMap<(TrackId, ClipId), u64> = HashMap::new();
    for r in notes {
        let start = r.note.start().get();
        anchors
            .entry((r.track, r.clip))
            .and_modify(|a| *a = (*a).min(start))
            .or_insert(start);
    }
    plan(project, notes, |r, clip, note| {
        let anchor = clip.start().get() + anchors.get(&(r.track, r.clip)).copied().unwrap_or(0);
        let abs = clip.start().get() + note.start().get();
        let moved = u128::from(anchor) + ratio(u128::from(abs - anchor));
        let length = u32::try_from(ratio(u128::from(note.length())).max(1))
            .map_err(|_| ModelError::TickOverflow)?;
        relocate(
            clip,
            &note.with_length(length)?,
            i128::try_from(moved).map_err(|_| ModelError::TickOverflow)?,
        )
    })
}

/// 참조가 가리키는 클립과, 그 클립에 노트가 정말 있는지 확인한다.
fn find<'a>(project: &'a Project, r: &NoteRef) -> Result<(&'a Clip, ()), ModelError> {
    let clip = project
        .track(r.track)
        .ok_or(ModelError::UnknownTrack)?
        .clip(r.clip)
        .ok_or(ModelError::UnknownClip)?;
    if !clip.notes().contains(&r.note) {
        return Err(ModelError::NoteNotFound);
    }
    Ok((clip, ()))
}

/// 노트를 절대 위치 `abs`로 옮긴 노트. 클립 시작 앞이면 거부한다(결정: 앞은 거부).
fn relocate(clip: &Clip, note: &Note, abs: i128) -> Result<Note, ModelError> {
    let relative = abs - i128::from(clip.start().get());
    // 음수만 「클립 시작 앞」이다. u64를 넘는 것은 오버플로라 원인을 가리지 않게 따로 알린다
    // (S0-T7 리뷰 1 지적 4).
    if relative < 0 {
        return Err(ModelError::NoteBeforeClipStart);
    }
    let relative = u64::try_from(relative).map_err(|_| ModelError::TickOverflow)?;
    note.with_start(Tick::new(relative))
}

/// 절대 위치 `abs`에서 가장 가까운 그리드 점. 그리드는 `abs`가 속한 마디의 시작부터 잰다. 홀수 칸은
/// `그리드 × 스윙%`만큼 뒤로 민다. 마디 길이가 그리드의 배수가 아니면 다음 마디 시작도 후보다.
/// 거리가 같으면 앞쪽을 고른다.
fn snap(project: &Project, abs: u64, grid: u64, swing: u8) -> u64 {
    let meter = project.meter_map();
    let bar = meter.position(Tick::new(abs)).bar();
    let bar_start = meter.bar_start(bar).map_or(abs, Tick::get);
    let next = meter.bar_start(bar + 1).map_or(u64::MAX, Tick::get);
    let k = (abs - bar_start) / grid;
    let point = |k: u64| {
        let plain = bar_start.saturating_add(k.saturating_mul(grid));
        if k % 2 == 1 {
            plain.saturating_add(grid * u64::from(swing) / 100)
        } else {
            plain
        }
    };
    let mut candidates: Vec<u64> = [k.checked_sub(1), Some(k), Some(k + 1)]
        .into_iter()
        .flatten()
        // 마디 안의 칸만. 그 칸의 스윙 점은 75% 이하라 다음 칸 앞에 있다.
        .filter(|&k| bar_start.saturating_add(k.saturating_mul(grid)) < next)
        .map(point)
        .collect();
    candidates.push(next);
    candidates
        .into_iter()
        .min_by_key(|&c| (c.abs_diff(abs), c))
        .unwrap_or(abs)
}

/// 노트마다 새 노트를 계산하고, 클립 경계 규칙을 적용한 편집 묶음을 만든다.
/// 결과 끝이 클립 길이를 넘으면 클립을 마디 단위로 늘리고, 다음 클립과 겹치면 거부한다.
fn plan(
    project: &Project,
    notes: &[NoteRef],
    mut f: impl FnMut(&NoteRef, &Clip, &Note) -> Result<Note, ModelError>,
) -> Result<Vec<Edit>, ModelError> {
    // 같은 노트를 클립에 있는 개수보다 많이 고르면 적용 단계에서 실패한다. 함수 문서대로 계산
    // 단계에서 거부한다(S0-T7 리뷰 1 지적 3).
    // 선택 크기에 선형이도록 개수를 해시로 센다.
    let mut picked: HashMap<(TrackId, ClipId, &Note), usize> = HashMap::new();
    for r in notes {
        *picked.entry((r.track, r.clip, &r.note)).or_default() += 1;
    }
    for ((track, clip, note), count) in picked {
        let r = NoteRef {
            track,
            clip,
            note: note.clone(),
        };
        let (clip, ()) = find(project, &r)?;
        if clip.notes().iter().filter(|n| *n == note).count() < count {
            return Err(ModelError::NoteNotFound);
        }
    }
    let mut moved: Vec<(&NoteRef, Note)> = Vec::with_capacity(notes.len());
    // (트랙, 클립) → 필요한 길이.
    let mut needed: Vec<(TrackId, ClipId, u64)> = Vec::new();
    for r in notes {
        let (clip, ()) = find(project, r)?;
        let new = f(r, clip, &r.note)?;
        // 원래 꼬리였던 노트는 꼬리로 둔다: 끝이 아니라 시작만 클립 안에 있으면 된다
        // (결정 개정, S0-T7 리뷰 1 지적 1). 그 밖의 노트는 끝까지 클립 안이어야 한다.
        let needed_end = if r.note.end() > clip.length() {
            new.start().get() + 1
        } else {
            new.end()
        };
        if needed_end > clip.length() {
            match needed
                .iter_mut()
                .find(|(t, c, _)| *t == r.track && *c == r.clip)
            {
                Some(entry) => entry.2 = entry.2.max(needed_end),
                None => needed.push((r.track, r.clip, needed_end)),
            }
        }
        moved.push((r, new));
    }
    // 클립마다 편집 하나: 새 길이(늘리기만)와 노트 바꾸기를 한꺼번에. 꼬리 여부를 편집 직전 길이로
    // 판정해야 위의 계산(원래 길이 기준)과 어긋나지 않는다(S0-T7 리뷰 2 지적 1).
    let mut clips: Vec<(TrackId, ClipId)> = Vec::new();
    for (r, _) in &moved {
        if !clips.contains(&(r.track, r.clip)) {
            clips.push((r.track, r.clip));
        }
    }
    let mut edits = Vec::with_capacity(clips.len());
    for (track, id) in clips {
        let t = project.track(track).ok_or(ModelError::UnknownTrack)?;
        let clip = t.clip(id).ok_or(ModelError::UnknownClip)?;
        let length = match needed.iter().find(|(nt, nc, _)| *nt == track && *nc == id) {
            Some(&(_, _, end)) => bar_aligned_length(project, clip, end)?,
            None => clip.length(),
        };
        if length > clip.length() {
            let (start, stop) = (clip.start().get(), clip.start().get() + length);
            if t.clips()
                .any(|(other, c)| other != id && c.start().get() < stop && start < c.end())
            {
                return Err(ModelError::ClipOverlap { start, end: stop });
            }
        }
        let pairs = moved
            .iter()
            .filter(|(r, _)| r.track == track && r.clip == id)
            .map(|(r, to)| (r.note.clone(), to.clone()))
            .collect();
        edits.push(Edit::ReplaceNotes {
            track,
            clip: id,
            length,
            pairs,
        });
    }
    Ok(edits)
}

/// 클립 기준 끝 `end`를 덮도록 마디 단위로 늘린 클립 길이(REQ 5.10).
fn bar_aligned_length(project: &Project, clip: &Clip, end: u64) -> Result<u64, ModelError> {
    let abs_end = clip
        .start()
        .get()
        .checked_add(end)
        .ok_or(ModelError::TickOverflow)?;
    let meter = project.meter_map();
    let pos = meter.position(Tick::new(abs_end));
    let boundary = if pos.beat() == 0 && pos.tick() == 0 {
        abs_end
    } else {
        meter
            .bar_start(pos.bar() + 1)
            .ok_or(ModelError::TickOverflow)?
            .get()
    };
    Ok(boundary - clip.start().get())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Channel, Clip, Document, PitchOffset, Tick, TimeSignature, Track, TrackColor, Velocity,
    };

    fn note(start: u64, length: u32, pitch: u8) -> Note {
        Note::new(
            Tick::new(start),
            length,
            Pitch::new(pitch).unwrap(),
            Velocity::new(1000),
        )
        .unwrap()
    }

    /// 트랙 하나에 클립 둘: a(시작 `a_start`, 길이 3840), b(10 마디 뒤).
    fn doc_with(a_start: u64, notes: &[Note]) -> (Document, TrackId, ClipId, ClipId) {
        let mut p = Project::new();
        let mut t = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
        let tid = t.id();
        let mut clip = Clip::new(Tick::new(a_start), 3840).unwrap();
        for n in notes {
            clip.insert_note(n.clone()).unwrap();
        }
        let a = t.insert_clip(clip).unwrap();
        let b = t
            .insert_clip(Clip::new(Tick::new(10 * 3840), 3840).unwrap())
            .unwrap();
        p.push_track(t).unwrap();
        (Document::new(p, usize::MAX).unwrap(), tid, a, b)
    }

    fn refs(d: &Document, t: TrackId, c: ClipId) -> Vec<NoteRef> {
        d.project()
            .track(t)
            .unwrap()
            .clip(c)
            .unwrap()
            .notes()
            .iter()
            .map(|n| NoteRef {
                track: t,
                clip: c,
                note: n.clone(),
            })
            .collect()
    }

    fn starts(d: &Document, t: TrackId, c: ClipId) -> Vec<u64> {
        d.project()
            .track(t)
            .unwrap()
            .clip(c)
            .unwrap()
            .notes()
            .iter()
            .map(|n| n.start().get())
            .collect()
    }

    fn q(grid: Grid, triplet: bool, strength: u8, swing: u8) -> QuantizeSettings {
        QuantizeSettings::new(grid, triplet, strength, swing).unwrap()
    }

    #[test]
    fn quantize_settings_validate_and_grid_ticks() {
        assert_eq!(q(Grid::Quarter, false, 100, 0).grid_ticks(), 960);
        assert_eq!(q(Grid::Eighth, true, 100, 0).grid_ticks(), 320);
        assert_eq!(q(Grid::SixtyFourth, true, 100, 0).grid_ticks(), 40);
        assert!(QuantizeSettings::new(Grid::Quarter, false, 101, 0).is_err());
        assert!(QuantizeSettings::new(Grid::Quarter, false, 100, 76).is_err());
        assert!(QuantizeSettings::new(Grid::Quarter, false, 0, 75).is_ok());
    }

    #[test]
    fn quantize_snaps_to_bar_relative_grid() {
        // 클립이 박에서 어긋난 자리(100틱)에 있어도 곡의 마디 그리드에 맞는다.
        let (d, t, a, _) = doc_with(
            100,
            &[note(0, 10, 60), note(400, 10, 62), note(900, 10, 64)],
        );
        let edits = quantize(
            d.project(),
            &refs(&d, t, a),
            &q(Grid::Quarter, false, 100, 0),
        )
        .unwrap_err();
        // 절대 100 → 0은 클립 시작 앞이라 거부된다.
        assert_eq!(edits, ModelError::NoteBeforeClipStart);
        let (mut d, t, a, _) = doc_with(100, &[note(400, 10, 62), note(900, 10, 64)]);
        let edits = quantize(
            d.project(),
            &refs(&d, t, a),
            &q(Grid::Quarter, false, 100, 0),
        )
        .unwrap();
        d.apply(edits, None).unwrap();
        // 절대 500 → 960, 1000 → 960 (클립 기준 860).
        assert_eq!(starts(&d, t, a), vec![860, 860]);
    }

    #[test]
    fn quantize_bar_end_uses_next_bar_start() {
        // 7/8 마디(3360틱)에 4분음표 그리드: 마디 안 점은 0·960·1920·2880, 그다음은 다음 마디 시작 3360.
        let (d, t, a, _) = doc_with(0, &[note(3200, 10, 60)]);
        let mut p = d.into_project();
        p.meter_map_mut()
            .set(Tick::ZERO, TimeSignature::new(7, 8).unwrap());
        let mut d = Document::new(p, usize::MAX).unwrap();
        let edits = quantize(
            d.project(),
            &refs(&d, t, a),
            &q(Grid::Quarter, false, 100, 0),
        )
        .unwrap();
        d.apply(edits, None).unwrap();
        assert_eq!(starts(&d, t, a), vec![3360]);
    }

    #[test]
    fn quantize_strength_and_swing_values() {
        let (mut d, t, a, _) = doc_with(0, &[note(100, 10, 60)]);
        let edits = quantize(
            d.project(),
            &refs(&d, t, a),
            &q(Grid::Quarter, false, 50, 0),
        )
        .unwrap();
        d.apply(edits, None).unwrap();
        assert_eq!(starts(&d, t, a), vec![50], "강도 50%: 100 → 0의 절반");

        // 8분음표 그리드에 스윙 50%: 홀수 칸(480)은 480 + 240 = 720.
        let (mut d, t, a, _) = doc_with(0, &[note(700, 10, 60), note(1000, 10, 62)]);
        let edits = quantize(
            d.project(),
            &refs(&d, t, a),
            &q(Grid::Eighth, false, 100, 50),
        )
        .unwrap();
        d.apply(edits, None).unwrap();
        assert_eq!(starts(&d, t, a), vec![720, 960]);

        // 스윙 75%: 홀수 칸 480 → 840.
        let (mut d, t, a, _) = doc_with(0, &[note(800, 10, 60)]);
        let edits = quantize(
            d.project(),
            &refs(&d, t, a),
            &q(Grid::Eighth, false, 100, 75),
        )
        .unwrap();
        d.apply(edits, None).unwrap();
        assert_eq!(starts(&d, t, a), vec![840]);
    }

    #[test]
    fn quantize_triplet_grid() {
        // 8분음표 셋잇단 그리드 320틱: 650 → 640.
        let (mut d, t, a, _) = doc_with(0, &[note(650, 10, 60)]);
        let edits = quantize(d.project(), &refs(&d, t, a), &q(Grid::Eighth, true, 100, 0)).unwrap();
        d.apply(edits, None).unwrap();
        assert_eq!(starts(&d, t, a), vec![640]);
    }

    #[test]
    fn transpose_folds_into_range() {
        let (mut d, t, a, _) = doc_with(0, &[note(0, 10, 120), note(10, 10, 5)]);
        let edits = transpose(d.project(), &refs(&d, t, a), 12, PitchRange::full()).unwrap();
        d.apply(edits, None).unwrap();
        let pitches: Vec<u8> = d
            .project()
            .track(t)
            .unwrap()
            .clip(a)
            .unwrap()
            .notes()
            .iter()
            .map(|n| n.pitch().get())
            .collect();
        // 120 + 12 = 132 → 120(한 옥타브 접음), 5 + 12 = 17.
        assert_eq!(pitches, vec![120, 17]);

        let range = PitchRange::new(Pitch::new(60).unwrap(), Pitch::new(71).unwrap()).unwrap();
        let (mut d, t, a, _) = doc_with(0, &[note(0, 10, 60)]);
        let edits = transpose(d.project(), &refs(&d, t, a), 25, range).unwrap();
        d.apply(edits, None).unwrap();
        // 60 + 25 = 85 → 73 → 61.
        assert_eq!(
            d.project().track(t).unwrap().clip(a).unwrap().notes()[0]
                .pitch()
                .get(),
            61
        );

        assert!(PitchRange::new(Pitch::new(60).unwrap(), Pitch::new(70).unwrap()).is_err());

        // 오프셋 +반음 1개를 가진 127은 128반음이라 범위 밖. 접어서 들어가야 한다.
        let bent = note(0, 10, 115)
            .with_offset(PitchOffset::new(PitchOffset::SEMITONE).unwrap())
            .unwrap();
        let (mut d, t, a, _) = doc_with(0, &[bent]);
        let edits = transpose(d.project(), &refs(&d, t, a), 12, PitchRange::full()).unwrap();
        d.apply(edits, None).unwrap();
        assert_eq!(
            d.project().track(t).unwrap().clip(a).unwrap().notes()[0]
                .pitch()
                .get(),
            115
        );
    }

    #[test]
    fn shift_past_end_extends_clip_by_bars() {
        let (mut d, t, a, _) = doc_with(0, &[note(3000, 480, 60)]);
        let edits = shift(d.project(), &refs(&d, t, a), 1000).unwrap();
        d.apply(edits, None).unwrap();
        let clip = d.project().track(t).unwrap().clip(a).unwrap();
        // 끝 4480 > 3840 → 다음 마디 끝 7680까지 늘린다.
        assert_eq!((clip.length(), starts(&d, t, a)), (7680, vec![4000]));
        assert!(d.undo().unwrap());
        assert_eq!(
            d.project().track(t).unwrap().clip(a).unwrap().length(),
            3840
        );
    }

    #[test]
    fn shift_before_start_is_rejected() {
        let (d, t, a, _) = doc_with(0, &[note(100, 10, 60)]);
        assert_eq!(
            shift(d.project(), &refs(&d, t, a), -101).unwrap_err(),
            ModelError::NoteBeforeClipStart
        );
        assert!(shift(d.project(), &refs(&d, t, a), -100).is_ok());
    }

    #[test]
    fn extend_into_next_clip_is_rejected() {
        // 클립 b는 10마디 뒤. 노트를 9마디 넘게 밀면 a가 b와 겹친다.
        let (d, t, a, _) = doc_with(0, &[note(0, 10, 60)]);
        assert!(matches!(
            shift(d.project(), &refs(&d, t, a), 10 * 3840).unwrap_err(),
            ModelError::ClipOverlap { .. }
        ));
    }

    #[test]
    fn scale_doubles_spacing_and_length() {
        let (mut d, t, a, _) = doc_with(0, &[note(480, 240, 60), note(960, 240, 62)]);
        let edits = scale(d.project(), &refs(&d, t, a), 2, 1).unwrap();
        d.apply(edits, None).unwrap();
        let notes = d
            .project()
            .track(t)
            .unwrap()
            .clip(a)
            .unwrap()
            .notes()
            .to_vec();
        // 기준은 가장 이른 시작 480: 480 → 480, 960 → 1440. 길이 240 → 480.
        let got: Vec<(u64, u32)> = notes
            .iter()
            .map(|n| (n.start().get(), n.length()))
            .collect();
        assert_eq!(got, vec![(480, 480), (1440, 480)]);

        let (mut d, t, a, _) = doc_with(0, &[note(0, 1, 60), note(1001, 3, 62)]);
        let edits = scale(d.project(), &refs(&d, t, a), 1, 2).unwrap();
        d.apply(edits, None).unwrap();
        let got: Vec<(u64, u32)> = d
            .project()
            .track(t)
            .unwrap()
            .clip(a)
            .unwrap()
            .notes()
            .iter()
            .map(|n| (n.start().get(), n.length()))
            .collect();
        // 반올림(0.5는 올림): 1001/2 = 500.5 → 501, 3/2 = 1.5 → 2, 1/2 = 0.5 → 1(최소 1).
        assert_eq!(got, vec![(0, 1), (501, 2)]);
        assert!(scale(d.project(), &[], 0, 1).is_err());
        assert!(scale(d.project(), &[], 1, 0).is_err());
    }

    /// 왼쪽 클립(0..3840)에 숨김 꼬리 노트 `tail`을 남긴 문서. 오른쪽 클립은 지운다.
    fn doc_with_tail(tail: &Note, others: &[Note]) -> (Document, TrackId, ClipId) {
        let mut p = Project::new();
        let mut t = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
        let tid = t.id();
        let c = t
            .insert_clip(Clip::new(Tick::ZERO, 2 * 3840).unwrap())
            .unwrap();
        t.insert_note(c, tail.clone()).unwrap();
        for n in others {
            t.insert_note(c, n.clone()).unwrap();
        }
        let right = t.split_clip(c, Tick::new(3840)).unwrap();
        t.remove_clip(right);
        p.push_track(t).unwrap();
        (Document::new(p, usize::MAX).unwrap(), tid, c)
    }

    #[test]
    fn tail_note_keeps_clip_length_when_start_stays_inside() {
        // 결정 개정: 원래 꼬리였던 노트는 끝으로 클립을 늘리지 않는다.
        let tail = note(3000, 2000, 60);
        let (mut d, t, c) = doc_with_tail(&tail, &[]);
        let refs = refs(&d, t, c);
        for edits in [
            transpose(d.project(), &refs, 5, PitchRange::full()).unwrap(),
            shift(d.project(), &refs, 0).unwrap(),
            shift(d.project(), &refs, 500).unwrap(),
        ] {
            let mut copy = d.clone();
            copy.apply(edits, None).unwrap();
            assert_eq!(
                copy.project().track(t).unwrap().clip(c).unwrap().length(),
                3840
            );
        }
        // 시작이 밖으로 나가면 시작을 덮는 마디까지만 늘리고, 노트는 꼬리로 남는다.
        let edits = shift(d.project(), &refs, 1000).unwrap();
        d.apply(edits, None).unwrap();
        let clip = d.project().track(t).unwrap().clip(c).unwrap();
        assert_eq!((clip.length(), clip.notes()[0].start().get()), (7680, 4000));
        assert_eq!(clip.notes()[0].end(), 6000);
    }

    #[test]
    fn tail_start_beyond_extended_length_still_applies() {
        // 리뷰 2 지적 1: 늘린 길이가 원래 꼬리 끝 이상이어도, 묶음은 적용되어야 한다.
        let tail = note(1920, 3840, 64);
        let (mut d, t, c) = doc_with_tail(&tail, &[]);
        let edits = shift(d.project(), &refs(&d, t, c), 4000).unwrap();
        d.apply(edits, None).unwrap();
        let clip = d.project().track(t).unwrap().clip(c).unwrap();
        assert_eq!(
            (
                clip.length(),
                clip.notes()[0].start().get(),
                clip.notes()[0].end()
            ),
            (7680, 5920, 9760)
        );
    }

    #[test]
    fn scale_anchors_each_clip_at_its_first_selected_note() {
        // 결정 개정: 클립마다 그 클립 안의 가장 이른 선택 노트가 기준이다. 클립 시작이 아니다.
        let mut project = Project::new();
        let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
        let tid = track.id();
        let a = track
            .insert_clip(Clip::new(Tick::ZERO, 3840).unwrap())
            .unwrap();
        let b = track
            .insert_clip(Clip::new(Tick::new(7680), 3840).unwrap())
            .unwrap();
        for (clip, start) in [(a, 480), (a, 960), (b, 1000), (b, 1600)] {
            track.insert_note(clip, note(start, 100, 60)).unwrap();
        }
        project.push_track(track).unwrap();
        let mut d = Document::new(project, usize::MAX).unwrap();
        let mut sel = refs(&d, tid, a);
        sel.extend(refs(&d, tid, b));
        d.apply(scale(d.project(), &sel, 1, 2).unwrap(), None)
            .unwrap();
        assert_eq!(starts(&d, tid, a), vec![480, 720]);
        assert_eq!(starts(&d, tid, b), vec![1000, 1300]);
    }

    #[test]
    fn stale_reference_is_rejected_on_apply() {
        let (mut d, t, a, _) = doc_with(0, &[note(100, 10, 60)]);
        let stale = vec![NoteRef {
            track: t,
            clip: a,
            note: note(5, 10, 60),
        }];
        let edits = shift(d.project(), &stale, 10);
        // 계산 단계나 적용 단계 중 하나에서 거부되고 문서는 그대로다.
        let before = d.project().clone();
        if let Ok(edits) = edits {
            assert!(d.apply(edits, None).is_err());
        }
        assert_eq!(d.project(), &before);
    }
}
