use crate::{ModelError, Note, Tick, TrackColor};

/// 트랙 안에서 클립을 가리키는 식별자. 트랙이 발급하며 같은 트랙 안에서만 의미가 있다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClipId(pub(crate) u64);

/// 트랙 타임라인 위의 연속 구간 `[start, start + length)`. 노트를 담는 유일한 그릇이다(REQ 3, 5.10).
///
/// 불변식(공개 API로 깨뜨릴 수 없다):
/// - 길이 ≥ 1틱이고 `start + length`가 u64 안이다.
/// - 모든 노트의 시작은 `0 ≤ start < length`다. 끝이 길이를 넘는 노트(숨김 꼬리)는
///   분할로만 생긴다(`docs/decisions/2026-10-12-clip-split-and-velocity.md` Q1).
/// - 노트는 `(시작, 음높이)` 순으로 정렬되어 있다.
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
}

impl Clip {
    /// # Errors
    ///
    /// 길이가 0이면 [`ModelError::ZeroClipLength`], 끝이 u64를 넘으면 [`ModelError::TickOverflow`].
    pub fn new(start: Tick, length: u64) -> Result<Self, ModelError> {
        todo!("{start:?} {length}")
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
        todo!("{note:?}")
    }

    /// 같은 값의 노트 하나를 지운다. 지웠으면 `true`.
    pub fn remove_note(&mut self, note: &Note) -> bool {
        todo!("{note:?}")
    }

    pub(crate) fn set_start(&mut self, start: Tick) -> Result<(), ModelError> {
        todo!("{start:?}")
    }

    /// 클립 기준 `offset`에서 둘로 나눈다. `self`는 왼쪽이 되고 오른쪽을 돌려준다.
    /// 분할점을 가로지르는 노트는 왼쪽에 숨김 꼬리로 남는다(결정 Q1).
    pub(crate) fn split_at(&mut self, offset: u64) -> Result<Clip, ModelError> {
        todo!("{offset}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Pitch, Velocity};

    fn note(start: u64, length: u32, pitch: u8) -> Note {
        Note::new(Tick::new(start), length, Pitch::new(pitch).unwrap(), Velocity::new(100).unwrap())
            .unwrap()
    }

    #[test]
    fn clip_new_zero_length_or_overflowing_end_is_rejected() {
        assert_eq!(Clip::new(Tick::ZERO, 0), Err(ModelError::ZeroClipLength));
        assert_eq!(Clip::new(Tick::new(u64::MAX), 1), Err(ModelError::TickOverflow));
        let clip = Clip::new(Tick::new(u64::MAX - 1), 1).unwrap();
        assert_eq!(clip.end(), u64::MAX);
    }

    #[test]
    fn clip_insert_note_past_end_is_rejected() {
        let mut clip = Clip::new(Tick::new(1000), 960).unwrap();
        let before = clip.clone();
        assert_eq!(
            clip.insert_note(note(900, 61, 60)),
            Err(ModelError::NoteOutsideClip { start: 900, end: 961, clip_length: 960 })
        );
        assert_eq!(clip.insert_note(note(960, 1, 60)).is_err(), true);
        assert_eq!(clip, before);
        assert_eq!(clip.insert_note(note(900, 60, 60)), Ok(()));
    }

    #[test]
    fn clip_insert_note_keeps_notes_sorted() {
        let mut clip = Clip::new(Tick::ZERO, 960).unwrap();
        for n in [note(480, 10, 64), note(0, 10, 67), note(480, 10, 60), note(240, 10, 62)] {
            clip.insert_note(n).unwrap();
        }
        let order: Vec<(u64, u8)> =
            clip.notes().iter().map(|n| (n.start().get(), n.pitch().get())).collect();
        assert_eq!(order, vec![(0, 67), (240, 62), (480, 60), (480, 64)]);
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
