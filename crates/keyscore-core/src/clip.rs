use crate::{ModelError, Note, Tick, TrackColor};

use std::sync::atomic::{AtomicU64, Ordering};

/// 클립 식별자. 프로세스 안에서 유일하게 발급한다.
///
/// 트랙마다 0부터 세면 다른 트랙의 식별자로 무관한 클립을 지울 수 있고(S0-T2 리뷰 1 제안 4),
/// 트랙이 카운터를 가지면 undo로 되돌린 트랙이 카운터 때문에 원본과 달라진다(제안 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClipId(pub(crate) u64);

impl ClipId {
    pub(crate) fn allocate() -> ClipId {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        // 유일성만 필요하고 다른 메모리와의 순서는 필요 없다.
        ClipId(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// 트랙 타임라인 위의 연속 구간 `[start, start + length)`. 노트를 담는 유일한 그릇이다(REQ 3, 5.10).
///
/// 불변식(공개 API로 깨뜨릴 수 없다):
/// - 길이 ≥ 1틱이고 `start + length`가 u64 안이다.
/// - 모든 노트의 시작은 `0 ≤ start < length`다. 끝이 길이를 넘는 노트(숨김 꼬리)는
///   분할로만 생긴다(`docs/decisions/2026-10-12-clip-split-and-velocity.md` Q1).
/// - 노트는 [`Note`]의 전체 순서(시작, 음높이, 그다음 나머지 필드)로 정렬되어 있다.
///
/// 시작·길이를 바꾸는 공개 메서드는 없다. 트랙 안의 겹침 검사를 우회하지 못하게
/// [`crate::Track`]의 메서드로만 바뀐다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clip {
    start: Tick,
    length: u64,
    name: Option<String>,
    color: Option<TrackColor>,
    notes: Vec<Note>,
    /// `.ksq` 클립 객체의 모르는 키. 다시 쓸 때 보존한다(REQ IO-1).
    extra: serde_json::Map<String, serde_json::Value>,
}

impl Clip {
    /// # Errors
    ///
    /// 길이가 0이면 [`ModelError::ZeroClipLength`], 끝이 u64를 넘으면 [`ModelError::TickOverflow`].
    pub fn new(start: Tick, length: u64) -> Result<Self, ModelError> {
        if length == 0 {
            return Err(ModelError::ZeroClipLength);
        }
        // end()가 덧셈을 검사 없이 하므로 여기서 오버플로를 막는다.
        start.checked_add(length)?;
        Ok(Clip {
            start,
            length,
            name: None,
            color: None,
            notes: Vec::new(),
            extra: serde_json::Map::new(),
        })
    }

    #[must_use]
    pub const fn start(&self) -> Tick {
        self.start
    }

    #[must_use]
    pub const fn length(&self) -> u64 {
        self.length
    }

    /// 트랙 기준 끝 틱(배타).
    #[must_use]
    pub const fn end(&self) -> u64 {
        self.start.get() + self.length
    }

    #[must_use]
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// 빈 문자열은 「이름 없음」이다.
    pub fn set_name(&mut self, name: impl Into<String>) {
        let name = name.into();
        self.name = if name.is_empty() { None } else { Some(name) };
    }

    /// `None`이면 트랙 색을 따른다(REQ 5.10).
    #[must_use]
    pub const fn color(&self) -> Option<TrackColor> {
        self.color
    }

    pub fn set_color(&mut self, color: Option<TrackColor>) {
        self.color = color;
    }

    /// 노트를 정렬 위치에 넣는다.
    ///
    /// # Errors
    ///
    /// 노트 끝이 클립 길이를 넘으면 [`ModelError::NoteOutsideClip`]. 이때 클립은 바뀌지 않는다.
    pub fn insert_note(&mut self, note: Note) -> Result<(), ModelError> {
        let end = note.end();
        if end > self.length {
            return Err(ModelError::NoteOutsideClip {
                start: note.start().get(),
                end,
                clip_length: self.length,
            });
        }
        // 전체 순서로 넣는다. 삽입 순서와 상관없이 같은 내용이면 같은 클립이 되고, 지웠다
        // 되살린 노트도 원래 자리로 돌아간다.
        let at = self.notes.partition_point(|n| n <= &note);
        self.notes.insert(at, note);
        Ok(())
    }

    /// 지웠던 노트를 되살린다(undo 전용). 숨김 꼬리 노트도 되살려야 하므로 `insert_note`와 달리
    /// 시작만 클립 안이면 받는다. 꼬리를 새로 만드는 공개 경로가 되지 않도록 crate 안에서만 쓴다.
    pub(crate) fn restore_note(&mut self, note: Note) -> Result<(), ModelError> {
        if note.start().get() >= self.length {
            return Err(ModelError::NoteOutsideClip {
                start: note.start().get(),
                end: note.end(),
                clip_length: self.length,
            });
        }
        self.start.checked_add(note.end())?;
        // 전체 순서로 넣는다. 삽입 순서와 상관없이 같은 내용이면 같은 클립이 되고, 지웠다
        // 되살린 노트도 원래 자리로 돌아간다.
        let at = self.notes.partition_point(|n| n <= &note);
        self.notes.insert(at, note);
        Ok(())
    }

    /// 지웠던·저장된 노트를 한꺼번에 되살린다(파일 읽기 전용). 노트마다 `restore_note`로 넣으면
    /// 역순 입력에서 매번 앞쪽에 끼워 넣어 O(n²)이 된다(S0-T6 리뷰 1 지적 4). 모두 검사한 뒤
    /// 붙이고 한 번 정렬한다. 실패하면 클립은 바뀌지 않고, 어긋난 노트의 번호를 돌려준다.
    pub(crate) fn restore_notes(&mut self, notes: Vec<Note>) -> Result<(), (usize, ModelError)> {
        for (i, note) in notes.iter().enumerate() {
            if note.start().get() >= self.length {
                return Err((
                    i,
                    ModelError::NoteOutsideClip {
                        start: note.start().get(),
                        end: note.end(),
                        clip_length: self.length,
                    },
                ));
            }
            self.start.checked_add(note.end()).map_err(|e| (i, e))?;
        }
        self.notes.extend(notes);
        self.notes.sort();
        Ok(())
    }

    /// 노트 하나를 바꾼다. 검사를 모두 끝낸 뒤 바꾼다(실패 시 상태 불변).
    pub(crate) fn replace_note(
        &mut self,
        from: &Note,
        to: Note,
        restoring: bool,
    ) -> Result<(), ModelError> {
        let index = self
            .notes
            .iter()
            .position(|n| n == from)
            .ok_or(ModelError::NoteNotFound)?;
        let tail_allowed = restoring || from.end() > self.length;
        if to.start().get() >= self.length || (!tail_allowed && to.end() > self.length) {
            return Err(ModelError::NoteOutsideClip {
                start: to.start().get(),
                end: to.end(),
                clip_length: self.length,
            });
        }
        self.start.checked_add(to.end())?;
        self.notes.remove(index);
        let at = self.notes.partition_point(|n| n <= &to);
        self.notes.insert(at, to);
        Ok(())
    }

    /// 길이를 `length`로 정하고 노트들을 차례로 바꾼다(`Edit::ReplaceNotes`). 꼬리 여부는 바꾸기 전
    /// 길이로 판정한다. `restoring`(undo)이면 줄이기와 꼬리를 허용한다. 복사본에서 모두 검사한 뒤
    /// 반영하므로 실패하면 클립은 그대로다.
    pub(crate) fn replace_notes(
        &mut self,
        length: u64,
        pairs: &[(Note, Note)],
        restoring: bool,
    ) -> Result<(), ModelError> {
        if length == 0 {
            return Err(ModelError::ZeroClipLength);
        }
        if !restoring && length < self.length {
            return Err(ModelError::ClipWouldShrink {
                current: self.length,
                requested: length,
            });
        }
        self.start.checked_add(length)?;
        let before = self.length;
        let mut notes = self.notes.clone();
        for (from, to) in pairs {
            let index = notes
                .iter()
                .position(|n| n == from)
                .ok_or(ModelError::NoteNotFound)?;
            let tail_allowed = restoring || from.end() > before;
            if to.start().get() >= length || (!tail_allowed && to.end() > length) {
                return Err(ModelError::NoteOutsideClip {
                    start: to.start().get(),
                    end: to.end(),
                    clip_length: length,
                });
            }
            self.start.checked_add(to.end())?;
            notes.remove(index);
            let at = notes.partition_point(|n| n <= to);
            notes.insert(at, to.clone());
        }
        if let Some(n) = notes.iter().find(|n| n.start().get() >= length) {
            return Err(ModelError::NoteOutsideClip {
                start: n.start().get(),
                end: n.end(),
                clip_length: length,
            });
        }
        self.notes = notes;
        self.length = length;
        Ok(())
    }

    /// 같은 값의 노트 하나를 지운다. 지웠으면 `true`.
    pub fn remove_note(&mut self, note: &Note) -> bool {
        match self.notes.iter().position(|n| n == note) {
            Some(i) => {
                self.notes.remove(i);
                true
            }
            None => false,
        }
    }

    /// 클립 기준으로 내용이 차지하는 끝: 길이와 가장 늦은 노트 끝(숨김 꼬리 포함) 중 큰 값.
    /// 클립 시작에 더해도 u64를 넘지 않는다는 것이 이 타입의 불변식이다.
    pub(crate) fn extent(&self) -> u64 {
        self.notes.iter().map(Note::end).fold(self.length, u64::max)
    }

    /// 꼬리까지 포함해 오버플로를 검사한다. 길이만 보면 숨김 꼬리의 절대 끝이 u64를
    /// 넘을 수 있다(S0-T2 리뷰 1 지적 1).
    pub(crate) fn extra(&self) -> &serde_json::Map<String, serde_json::Value> {
        &self.extra
    }

    pub(crate) fn set_extra(&mut self, extra: serde_json::Map<String, serde_json::Value>) {
        self.extra = extra;
    }

    /// 길이를 바꾼다(늘리기는 `ExtendClip`, 줄이기는 그 undo). 줄일 때도 노트 시작은 클립 안에
    /// 남아야 한다. 겹침 검사는 트랙이 한다.
    pub(crate) fn set_length(&mut self, length: u64) -> Result<(), ModelError> {
        if length == 0 {
            return Err(ModelError::ZeroClipLength);
        }
        self.start.checked_add(length)?;
        if let Some(n) = self.notes.iter().find(|n| n.start().get() >= length) {
            return Err(ModelError::NoteOutsideClip {
                start: n.start().get(),
                end: n.end(),
                clip_length: length,
            });
        }
        self.length = length;
        Ok(())
    }

    pub(crate) fn set_start(&mut self, start: Tick) -> Result<(), ModelError> {
        start.checked_add(self.extent())?;
        self.start = start;
        Ok(())
    }

    /// 클립 기준 `offset`에서 둘로 나눈다. `self`는 왼쪽이 되고 오른쪽을 돌려준다.
    /// 분할점을 가로지르는 노트는 왼쪽에 숨김 꼬리로 남는다(결정 Q1).
    pub(crate) fn split_at(&mut self, offset: u64) -> Result<Clip, ModelError> {
        if offset == 0 || offset >= self.length {
            let start = self.start.get();
            return Err(ModelError::SplitOutsideClip {
                at: start.saturating_add(offset),
                start,
                end: self.end(),
            });
        }
        // 노트가 시작 순으로 정렬되어 있으므로 경계 하나로 나뉜다.
        let cut = self.notes.partition_point(|n| n.start().get() < offset);
        // 실패할 수 있는 계산을 모두 끝낸 뒤에 self를 바꾼다(실패 시 상태 불변).
        let right_notes = self.notes[cut..]
            .iter()
            .map(|n| n.with_start(Tick::new(n.start().get() - offset)))
            .collect::<Result<Vec<_>, _>>()?;
        let right = Clip {
            start: self.start.checked_add(offset)?,
            length: self.length - offset,
            name: self.name.clone(),
            color: self.color,
            notes: right_notes,
            // 이름·색처럼 모르는 키도 두 클립에 그대로 둔다.
            extra: self.extra.clone(),
        };
        self.notes.truncate(cut);
        self.length = offset;
        Ok(right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Pitch, Velocity};

    fn note(start: u64, length: u32, pitch: u8) -> Note {
        Note::new(
            Tick::new(start),
            length,
            Pitch::new(pitch).unwrap(),
            Velocity::from_midi1(100).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn clip_new_zero_length_or_overflowing_end_is_rejected() {
        assert_eq!(Clip::new(Tick::ZERO, 0), Err(ModelError::ZeroClipLength));
        assert_eq!(
            Clip::new(Tick::new(u64::MAX), 1),
            Err(ModelError::TickOverflow)
        );
        let clip = Clip::new(Tick::new(u64::MAX - 1), 1).unwrap();
        assert_eq!(clip.end(), u64::MAX);
    }

    #[test]
    fn clip_insert_note_past_end_is_rejected() {
        let mut clip = Clip::new(Tick::new(1000), 960).unwrap();
        let before = clip.clone();
        assert_eq!(
            clip.insert_note(note(900, 61, 60)),
            Err(ModelError::NoteOutsideClip {
                start: 900,
                end: 961,
                clip_length: 960
            })
        );
        assert!(clip.insert_note(note(960, 1, 60)).is_err());
        assert_eq!(clip, before);
        assert_eq!(clip.insert_note(note(900, 60, 60)), Ok(()));
    }

    #[test]
    fn clip_insert_note_keeps_notes_sorted() {
        let mut clip = Clip::new(Tick::ZERO, 960).unwrap();
        for n in [
            note(480, 10, 64),
            note(0, 10, 67),
            note(480, 10, 60),
            note(240, 10, 62),
        ] {
            clip.insert_note(n).unwrap();
        }
        let order: Vec<(u64, u8)> = clip
            .notes()
            .iter()
            .map(|n| (n.start().get(), n.pitch().get()))
            .collect();
        assert_eq!(order, vec![(0, 67), (240, 62), (480, 60), (480, 64)]);
    }

    #[test]
    fn clip_note_order_is_total_so_remove_and_restore_round_trip() {
        // 같은 (시작, 음높이)에 길이만 다른 노트: 어떤 순서로 넣어도 같은 순서로 정렬되고,
        // 지웠다가 되살리면 원래 자리로 돌아온다(S0-T4 리뷰 1 지적 4).
        let long = note(0, 50, 60);
        let short = note(0, 10, 60);
        let mut a = Clip::new(Tick::ZERO, 960).unwrap();
        a.insert_note(long.clone()).unwrap();
        a.insert_note(short.clone()).unwrap();
        let mut b = Clip::new(Tick::ZERO, 960).unwrap();
        b.insert_note(short.clone()).unwrap();
        b.insert_note(long.clone()).unwrap();
        assert_eq!(a, b);
        let before = a.clone();
        for n in [&short, &long] {
            assert!(a.remove_note(n));
            a.restore_note(n.clone()).unwrap();
            assert_eq!(a, before);
        }
    }

    #[test]
    fn clip_remove_note_removes_only_one_equal_note() {
        let mut clip = Clip::new(Tick::ZERO, 960).unwrap();
        clip.insert_note(note(0, 10, 60)).unwrap();
        clip.insert_note(note(0, 10, 60)).unwrap();
        assert!(clip.remove_note(&note(0, 10, 60)));
        assert_eq!(clip.notes().len(), 1);
        assert!(!clip.remove_note(&note(1, 10, 60)));
    }

    #[test]
    fn clip_set_empty_name_has_no_name() {
        let mut clip = Clip::new(Tick::ZERO, 1).unwrap();
        clip.set_name("후렴");
        assert_eq!(clip.name(), Some("후렴"));
        clip.set_name("");
        assert_eq!(clip.name(), None);
    }
}
