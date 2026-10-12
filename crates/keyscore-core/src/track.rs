use crate::{Clip, ClipId, ModelError, Note, Tick, Velocity};

/// MIDI 채널 1–16(REQ SEQ-7). 사람에게 보이는 번호 그대로 담는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Channel(u8);

impl Channel {
    /// GM 퍼커션 채널. 이 채널의 트랙은 드럼 트랙이다(REQ 5.13, SEQ-7).
    pub const DRUMS: Channel = Channel(10);

    /// # Errors
    ///
    /// 1–16 밖이면 [`ModelError::ChannelOutOfRange`].
    pub fn new(value: u8) -> Result<Self, ModelError> {
        if (1..=16).contains(&value) {
            Ok(Channel(value))
        } else {
            Err(ModelError::ChannelOutOfRange(value))
        }
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    #[must_use]
    pub fn is_drum(self) -> bool {
        self == Self::DRUMS
    }
}

/// 고정 12색 팔레트의 인덱스(REQ 2 「트랙 색」).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrackColor(u8);

impl TrackColor {
    pub const PALETTE_LEN: u8 = 12;

    /// # Errors
    ///
    /// 0–11 밖이면 [`ModelError::ColorOutOfRange`].
    pub fn new(index: u8) -> Result<Self, ModelError> {
        if index < Self::PALETTE_LEN {
            Ok(TrackColor(index))
        } else {
            Err(ModelError::ColorOutOfRange(index))
        }
    }

    #[must_use]
    pub const fn index(self) -> u8 {
        self.0
    }
}

/// 프로그램 체인지 번호 0–127.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Program(u8);

impl Program {
    /// # Errors
    ///
    /// 127을 넘으면 [`ModelError::ProgramOutOfRange`].
    pub fn new(value: u8) -> Result<Self, ModelError> {
        if value <= 127 {
            Ok(Program(value))
        } else {
            Err(ModelError::ProgramOutOfRange(value))
        }
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// 뱅크 셀렉트 번호. CC 0(MSB)·CC 32(LSB)로 보내는 14비트 값이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bank(u16);

impl Bank {
    pub const MAX: u16 = 16383;

    /// # Errors
    ///
    /// 16383을 넘으면 [`ModelError::BankOutOfRange`].
    pub fn new(value: u16) -> Result<Self, ModelError> {
        if value <= Self::MAX {
            Ok(Bank(value))
        } else {
            Err(ModelError::BankOutOfRange(value))
        }
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// 트랙의 출력 목적지(REQ SEQ-7).
///
/// CoreMIDI 포트를 어떻게 식별할지(이름, uniqueID)는 S1에서 정한다. 지금은 이름만 담는다.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum OutputPort {
    /// 앱이 만드는 가상 MIDI 포트.
    #[default]
    Virtual,
    /// 이름으로 가리키는 CoreMIDI 목적지.
    Named(String),
}

/// 트랙 식별자. 프로세스 안에서 유일하게 발급한다.
///
/// 편집·undo는 트랙을 인덱스가 아니라 이 id로 가리킨다. 인덱스로 가리키면 히스토리 밖에서 트랙
/// 순서가 바뀐 뒤 undo가 엉뚱한 트랙에 적용된다(S0-T4 리뷰 1 지적 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrackId(u64);

impl TrackId {
    fn allocate() -> TrackId {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        // 유일성만 필요하고 다른 메모리와의 순서는 필요 없다.
        TrackId(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

/// 하나의 MIDI 목적지(포트·채널)에 묶인 선형 타임라인(REQ 3, SEQ-2, SEQ-7).
///
/// 불변식: 클립 구간 `[start, end)`는 서로 겹치지 않는다. 맞닿는 것(`end == start`)은 허용한다.
/// `&mut Clip`을 내주지 않으므로 이 불변식은 이 타입의 메서드로만 바뀐다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    id: TrackId,
    name: String,
    color: TrackColor,
    port: OutputPort,
    channel: Channel,
    bank: Option<Bank>,
    program: Option<Program>,
    default_velocity: Velocity,
    mute: bool,
    solo: bool,
    lyric_lang: Option<String>,
    /// 시작 틱 순으로 정렬. 겹침이 없으므로 끝 틱 순서도 같다.
    clips: Vec<(ClipId, Clip)>,
    /// `.ksq` 트랙 객체의 모르는 키. 다시 쓸 때 보존한다(REQ IO-1).
    extra: serde_json::Map<String, serde_json::Value>,
}

impl Track {
    /// 기본 벨로시티 100, 가상 포트, 뮤트·솔로 꺼짐, 뱅크·프로그램 없음.
    #[must_use]
    pub fn new(name: impl Into<String>, channel: Channel, color: TrackColor) -> Self {
        Track {
            id: TrackId::allocate(),
            name: name.into(),
            color,
            port: OutputPort::Virtual,
            channel,
            bank: None,
            program: None,
            default_velocity: Velocity::TRACK_DEFAULT,
            mute: false,
            solo: false,
            lyric_lang: None,
            clips: Vec::new(),
            extra: serde_json::Map::new(),
        }
    }

    /// 트랙 식별자. `clone`은 같은 id를 복사하므로, 복제본은 같은 프로젝트에 넣을 수 없다.
    #[must_use]
    pub const fn id(&self) -> TrackId {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    #[must_use]
    pub const fn color(&self) -> TrackColor {
        self.color
    }

    pub fn set_color(&mut self, color: TrackColor) {
        self.color = color;
    }

    #[must_use]
    pub fn port(&self) -> &OutputPort {
        &self.port
    }

    pub fn set_port(&mut self, port: OutputPort) {
        self.port = port;
    }

    #[must_use]
    pub const fn channel(&self) -> Channel {
        self.channel
    }

    pub fn set_channel(&mut self, channel: Channel) {
        self.channel = channel;
    }

    /// 채널 10 트랙은 드럼 트랙이다(SEQ-7).
    #[must_use]
    pub fn is_drum(&self) -> bool {
        self.channel.is_drum()
    }

    #[must_use]
    pub const fn bank(&self) -> Option<Bank> {
        self.bank
    }

    pub fn set_bank(&mut self, bank: Option<Bank>) {
        self.bank = bank;
    }

    #[must_use]
    pub const fn program(&self) -> Option<Program> {
        self.program
    }

    pub fn set_program(&mut self, program: Option<Program>) {
        self.program = program;
    }

    #[must_use]
    pub const fn default_velocity(&self) -> Velocity {
        self.default_velocity
    }

    pub fn set_default_velocity(&mut self, velocity: Velocity) {
        self.default_velocity = velocity;
    }

    #[must_use]
    pub const fn mute(&self) -> bool {
        self.mute
    }

    pub fn set_mute(&mut self, mute: bool) {
        self.mute = mute;
    }

    #[must_use]
    pub const fn solo(&self) -> bool {
        self.solo
    }

    pub fn set_solo(&mut self, solo: bool) {
        self.solo = solo;
    }

    /// 가사 언어 코드(ko·en·ja…, LYR-1). 분배 규칙과 AI 프롬프트에 쓴다.
    #[must_use]
    pub fn lyric_lang(&self) -> Option<&str> {
        self.lyric_lang.as_deref()
    }

    /// 빈 문자열은 「지정 안 함」이다.
    pub fn set_lyric_lang(&mut self, lang: Option<String>) {
        self.lyric_lang = lang.filter(|l| !l.is_empty());
    }

    pub(crate) fn extra(&self) -> &serde_json::Map<String, serde_json::Value> {
        &self.extra
    }

    pub(crate) fn set_extra(&mut self, extra: serde_json::Map<String, serde_json::Value>) {
        self.extra = extra;
    }

    /// 시작 틱 순서의 클립.
    pub fn clips(&self) -> impl Iterator<Item = (ClipId, &Clip)> {
        self.clips.iter().map(|(id, clip)| (*id, clip))
    }

    #[must_use]
    pub fn clip(&self, id: ClipId) -> Option<&Clip> {
        self.index_of(id).map(|i| &self.clips[i].1)
    }

    fn index_of(&self, id: ClipId) -> Option<usize> {
        self.clips.iter().position(|(cid, _)| *cid == id)
    }

    /// `[start, end)`가 `ignore`를 뺀 다른 클립과 겹치는가.
    fn overlaps(&self, start: u64, end: u64, ignore: Option<ClipId>) -> bool {
        self.clips
            .iter()
            .filter(|(id, _)| Some(*id) != ignore)
            .any(|(_, c)| c.start().get() < end && start < c.end())
    }

    /// 시작 순서를 지키는 위치에 넣는다. 겹침 검사는 호출자가 끝냈다.
    fn place(&mut self, id: ClipId, clip: Clip) {
        let at = self
            .clips
            .partition_point(|(_, c)| c.start() < clip.start());
        self.clips.insert(at, (id, clip));
    }

    /// # Errors
    ///
    /// 다른 클립과 겹치면 [`ModelError::ClipOverlap`]. 이때 트랙은 바뀌지 않는다.
    pub fn insert_clip(&mut self, clip: Clip) -> Result<ClipId, ModelError> {
        self.check_free(&clip, &[])?;
        let id = ClipId::allocate();
        self.place(id, clip);
        Ok(id)
    }

    /// 정해진 id로 클립을 넣는다. undo·redo가 원래 id를 되살릴 때 쓴다(S0-T4).
    pub(crate) fn insert_clip_with_id(&mut self, id: ClipId, clip: Clip) -> Result<(), ModelError> {
        if self.index_of(id).is_some() {
            return Err(ModelError::DuplicateClipId);
        }
        self.check_free(&clip, &[])?;
        self.place(id, clip);
        Ok(())
    }

    /// `clip`의 구간이 `ignore`를 뺀 다른 클립과 겹치지 않는지.
    fn check_free(&self, clip: &Clip, ignore: &[ClipId]) -> Result<(), ModelError> {
        let (start, end) = (clip.start().get(), clip.end());
        let hit = self
            .clips
            .iter()
            .filter(|(id, _)| !ignore.contains(id))
            .any(|(_, c)| c.start().get() < end && start < c.end());
        if hit {
            Err(ModelError::ClipOverlap { start, end })
        } else {
            Ok(())
        }
    }

    /// 클립 이름을 바꾼다. 빈 문자열은 「이름 없음」이다. 식별자는 그대로다.
    ///
    /// # Errors
    ///
    /// 없는 클립이면 [`ModelError::UnknownClip`].
    pub fn set_clip_name(&mut self, id: ClipId, name: impl Into<String>) -> Result<(), ModelError> {
        let index = self.index_of(id).ok_or(ModelError::UnknownClip)?;
        self.clips[index].1.set_name(name);
        Ok(())
    }

    /// 클립 색을 바꾼다. `None`이면 트랙 색을 따른다(REQ 5.10).
    ///
    /// # Errors
    ///
    /// 없는 클립이면 [`ModelError::UnknownClip`].
    pub fn set_clip_color(
        &mut self,
        id: ClipId,
        color: Option<TrackColor>,
    ) -> Result<(), ModelError> {
        let index = self.index_of(id).ok_or(ModelError::UnknownClip)?;
        self.clips[index].1.set_color(color);
        Ok(())
    }

    pub fn remove_clip(&mut self, id: ClipId) -> Option<Clip> {
        self.index_of(id).map(|i| self.clips.remove(i).1)
    }

    /// 클립을 새 시작 틱으로 옮긴다. 노트는 클립 기준이라 함께 움직인다.
    ///
    /// # Errors
    ///
    /// 없는 클립이면 [`ModelError::UnknownClip`], 다른 클립과 겹치면 [`ModelError::ClipOverlap`],
    /// 끝이 u64를 넘으면 [`ModelError::TickOverflow`]. 실패하면 트랙은 바뀌지 않는다.
    pub fn move_clip(&mut self, id: ClipId, new_start: Tick) -> Result<(), ModelError> {
        let index = self.index_of(id).ok_or(ModelError::UnknownClip)?;
        let clip = &self.clips[index].1;
        let end = new_start.checked_add(clip.length())?.get();
        // 숨김 꼬리까지 u64 안인지 클립을 빼내기 전에 검사한다(실패 시 상태 불변).
        new_start.checked_add(clip.extent())?;
        if self.overlaps(new_start.get(), end, Some(id)) {
            return Err(ModelError::ClipOverlap {
                start: new_start.get(),
                end,
            });
        }
        // 제자리에서 먼저 바꾼다. set_start가 실패해도 클립을 빼낸 상태로 남지 않는다
        // (S0-T2 리뷰 2 제안 2). 위의 검사 때문에 지금은 실패하지 않는다.
        self.clips[index].1.set_start(new_start)?;
        let (_, clip) = self.clips.remove(index);
        self.place(id, clip);
        Ok(())
    }

    /// 트랙 기준 `at`에서 클립을 둘로 나누고 오른쪽 클립의 식별자를 돌려준다.
    /// 분할점을 가로지르는 노트는 왼쪽 클립에 숨김 꼬리로 남는다(결정 Q1).
    ///
    /// # Errors
    ///
    /// 없는 클립이면 [`ModelError::UnknownClip`], `at`이 클립 안쪽(`start < at < end`)이 아니면
    /// [`ModelError::SplitOutsideClip`]. 실패하면 트랙은 바뀌지 않는다.
    pub fn split_clip(&mut self, id: ClipId, at: Tick) -> Result<ClipId, ModelError> {
        self.split_clip_with_id(id, at, None)
    }

    /// 오른쪽 클립 id를 정해 분할한다. redo가 처음 분할 때와 같은 id를 쓰게 한다(S0-T4).
    pub(crate) fn split_clip_with_id(
        &mut self,
        id: ClipId,
        at: Tick,
        right_id: Option<ClipId>,
    ) -> Result<ClipId, ModelError> {
        let index = self.index_of(id).ok_or(ModelError::UnknownClip)?;
        if let Some(right) = right_id
            && self.index_of(right).is_some()
        {
            return Err(ModelError::DuplicateClipId);
        }
        let clip = &self.clips[index].1;
        let (start, end) = (clip.start().get(), clip.end());
        if at.get() <= start || at.get() >= end {
            return Err(ModelError::SplitOutsideClip {
                at: at.get(),
                start,
                end,
            });
        }
        let right = self.clips[index].1.split_at(at.get() - start)?;
        let right_id = right_id.unwrap_or_else(ClipId::allocate);
        // 오른쪽 클립은 왼쪽 바로 뒤에 놓인다. 원래 구간 안이므로 겹침이 생기지 않는다.
        self.clips.insert(index + 1, (right_id, right));
        Ok(right_id)
    }

    /// 분할의 역(undo 전용). 오른쪽 클립을 지우고 왼쪽 클립을 `original`로 바꾼다.
    /// 검사를 모두 끝낸 뒤에 바꾼다(실패 시 상태 불변).
    pub(crate) fn unsplit_clip(
        &mut self,
        left: ClipId,
        right: ClipId,
        original: Clip,
    ) -> Result<(), ModelError> {
        let li = self.index_of(left).ok_or(ModelError::UnknownClip)?;
        let ri = self.index_of(right).ok_or(ModelError::UnknownClip)?;
        if li == ri {
            return Err(ModelError::DuplicateClipId);
        }
        // 지금의 두 클립이 정말 `original`을 한 경계에서 나눈 것인지 확인한다. 아니면 되돌리면
        // 두 클립 사이의 다른 내용을 덮거나 분할 없이 꼬리를 만들 수 있다(S0-T4 리뷰 1 지적 3).
        let (l, r) = (&self.clips[li].1, &self.clips[ri].1);
        let fits = l.end() == r.start().get()
            && original.start() == l.start()
            && original.end() == r.end();
        if !fits {
            return Err(ModelError::SplitOutsideClip {
                at: l.end(),
                start: original.start().get(),
                end: original.end(),
            });
        }
        self.check_free(&original, &[left, right])?;
        let (first, second) = if li > ri { (li, ri) } else { (ri, li) };
        self.clips.remove(first);
        self.clips.remove(second);
        self.place(left, original);
        Ok(())
    }

    /// 지웠던 노트를 되살린다(undo 전용). 숨김 꼬리 노트도 받는다.
    pub(crate) fn restore_note(&mut self, id: ClipId, note: Note) -> Result<(), ModelError> {
        let index = self.index_of(id).ok_or(ModelError::UnknownClip)?;
        self.clips[index].1.restore_note(note)
    }

    /// # Errors
    ///
    /// 없는 클립이면 [`ModelError::UnknownClip`], 노트가 클립 밖이면 [`ModelError::NoteOutsideClip`].
    pub fn insert_note(&mut self, id: ClipId, note: Note) -> Result<(), ModelError> {
        let index = self.index_of(id).ok_or(ModelError::UnknownClip)?;
        self.clips[index].1.insert_note(note)
    }

    /// 같은 값의 노트 하나를 지운다. 지웠으면 `Ok(true)`.
    ///
    /// # Errors
    ///
    /// 없는 클립이면 [`ModelError::UnknownClip`].
    pub fn remove_note(&mut self, id: ClipId, note: &Note) -> Result<bool, ModelError> {
        let index = self.index_of(id).ok_or(ModelError::UnknownClip)?;
        Ok(self.clips[index].1.remove_note(note))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Pitch;

    fn ch(v: u8) -> Channel {
        Channel::new(v).unwrap()
    }

    fn track() -> Track {
        Track::new("Lead", ch(1), TrackColor::new(0).unwrap())
    }

    fn clip(start: u64, length: u64) -> Clip {
        Clip::new(Tick::new(start), length).unwrap()
    }

    fn note(start: u64, length: u32) -> Note {
        Note::new(
            Tick::new(start),
            length,
            Pitch::new(60).unwrap(),
            Velocity::from_midi1(100).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn channel_new_zero_or_seventeen_is_rejected() {
        assert_eq!(Channel::new(0), Err(ModelError::ChannelOutOfRange(0)));
        assert_eq!(Channel::new(17), Err(ModelError::ChannelOutOfRange(17)));
        assert_eq!(Channel::new(1).map(Channel::get), Ok(1));
        assert_eq!(Channel::new(16).map(Channel::get), Ok(16));
    }

    #[test]
    fn channel_ten_is_drum() {
        for c in 1..=16 {
            assert_eq!(ch(c).is_drum(), c == 10, "채널 {c}");
        }
        assert_eq!(Channel::DRUMS, ch(10));
    }

    #[test]
    fn small_value_types_reject_out_of_range() {
        assert_eq!(TrackColor::new(12), Err(ModelError::ColorOutOfRange(12)));
        assert_eq!(TrackColor::new(11).map(TrackColor::index), Ok(11));
        assert_eq!(Program::new(128), Err(ModelError::ProgramOutOfRange(128)));
        assert_eq!(Program::new(127).map(Program::get), Ok(127));
        assert_eq!(Bank::new(16384), Err(ModelError::BankOutOfRange(16384)));
        assert_eq!(Bank::new(16383).map(Bank::get), Ok(16383));
    }

    #[test]
    fn track_new_has_documented_defaults() {
        let t = track();
        assert_eq!(t.name(), "Lead");
        assert_eq!(t.port(), &OutputPort::Virtual);
        assert_eq!(t.default_velocity().to_midi1(), 100);
        assert!(!t.mute() && !t.solo());
        assert_eq!((t.bank(), t.program(), t.lyric_lang()), (None, None, None));
        assert_eq!(t.clips().count(), 0);
    }

    #[test]
    fn track_insert_overlapping_clip_is_rejected() {
        let mut t = track();
        t.insert_clip(clip(960, 960)).unwrap();
        let before = t.clone();
        assert_eq!(
            t.insert_clip(clip(1919, 10)),
            Err(ModelError::ClipOverlap {
                start: 1919,
                end: 1929
            })
        );
        assert!(t.insert_clip(clip(0, 961)).is_err());
        assert!(t.insert_clip(clip(1000, 10)).is_err());
        assert!(t.insert_clip(clip(0, 5000)).is_err());
        assert_eq!(t, before);
        // 맞닿는 클립은 겹침이 아니다.
        t.insert_clip(clip(0, 960)).unwrap();
        t.insert_clip(clip(1920, 960)).unwrap();
        let starts: Vec<u64> = t.clips().map(|(_, c)| c.start().get()).collect();
        assert_eq!(starts, vec![0, 960, 1920]);
    }

    #[test]
    fn track_move_clip_onto_neighbor_is_rejected_and_state_unchanged() {
        let mut t = track();
        let a = t.insert_clip(clip(0, 960)).unwrap();
        let b = t.insert_clip(clip(1920, 960)).unwrap();
        let before = t.clone();
        assert!(matches!(
            t.move_clip(a, Tick::new(1000)),
            Err(ModelError::ClipOverlap { .. })
        ));
        assert_eq!(
            t.move_clip(a, Tick::new(u64::MAX - 10)),
            Err(ModelError::TickOverflow)
        );
        assert_eq!(
            t.move_clip(ClipId(u64::MAX), Tick::ZERO),
            Err(ModelError::UnknownClip)
        );
        assert_eq!(t, before);
        // 자기 자신과는 겹침으로 보지 않는다.
        t.move_clip(b, Tick::new(1900)).unwrap();
        t.move_clip(a, Tick::new(2880)).unwrap();
        let starts: Vec<(ClipId, u64)> = t.clips().map(|(id, c)| (id, c.start().get())).collect();
        assert_eq!(starts, vec![(b, 1900), (a, 2880)]);
    }

    #[test]
    fn track_split_clip_keeps_crossing_note_in_left_as_tail() {
        let mut t = track();
        let id = t.insert_clip(clip(960, 1920)).unwrap();
        t.insert_note(id, note(0, 100)).unwrap();
        t.insert_note(id, note(900, 200)).unwrap(); // 960을 가로지른다
        t.insert_note(id, note(960, 100)).unwrap();
        t.insert_note(id, note(1500, 420)).unwrap();
        let right = t.split_clip(id, Tick::new(1920)).unwrap();

        let left = t.clip(id).unwrap();
        assert_eq!((left.start().get(), left.length()), (960, 960));
        let left_notes: Vec<(u64, u32)> = left
            .notes()
            .iter()
            .map(|n| (n.start().get(), n.length()))
            .collect();
        assert_eq!(left_notes, vec![(0, 100), (900, 200)]);
        assert!(
            left.notes()[1].end() > left.length(),
            "꼬리가 클립 끝을 넘는다"
        );

        let right = t.clip(right).unwrap();
        assert_eq!((right.start().get(), right.length()), (1920, 960));
        let right_notes: Vec<(u64, u32)> = right
            .notes()
            .iter()
            .map(|n| (n.start().get(), n.length()))
            .collect();
        assert_eq!(right_notes, vec![(0, 100), (540, 420)]);
    }

    #[test]
    fn track_split_clip_at_boundary_is_rejected() {
        let mut t = track();
        let id = t.insert_clip(clip(960, 960)).unwrap();
        let before = t.clone();
        for at in [0, 960, 1920, 5000] {
            assert_eq!(
                t.split_clip(id, Tick::new(at)),
                Err(ModelError::SplitOutsideClip {
                    at,
                    start: 960,
                    end: 1920
                })
            );
        }
        assert_eq!(
            t.split_clip(ClipId(u64::MAX), Tick::new(1000)),
            Err(ModelError::UnknownClip)
        );
        assert_eq!(t, before);
    }

    #[test]
    fn track_set_clip_name_and_color_keep_clip_id() {
        let mut t = track();
        let id = t.insert_clip(clip(0, 960)).unwrap();
        t.set_clip_name(id, "후렴").unwrap();
        t.set_clip_color(id, Some(TrackColor::new(3).unwrap()))
            .unwrap();
        let c = t.clip(id).unwrap();
        assert_eq!(
            (c.name(), c.color()),
            (Some("후렴"), Some(TrackColor::new(3).unwrap()))
        );
        t.set_clip_name(id, "").unwrap();
        assert_eq!(t.clip(id).unwrap().name(), None);
        assert_eq!(
            t.set_clip_name(ClipId(u64::MAX), "x"),
            Err(ModelError::UnknownClip)
        );
        assert_eq!(
            t.set_clip_color(ClipId(u64::MAX), None),
            Err(ModelError::UnknownClip)
        );
    }

    #[test]
    fn track_clip_id_from_another_track_is_unknown() {
        let mut a = track();
        let mut b = track();
        let id_a = a.insert_clip(clip(0, 10)).unwrap();
        b.insert_clip(clip(1000, 10)).unwrap();
        assert!(b.clip(id_a).is_none());
        assert!(b.remove_clip(id_a).is_none());
    }

    #[test]
    fn track_note_ops_on_unknown_clip_are_rejected() {
        let mut t = track();
        assert_eq!(
            t.insert_note(ClipId(u64::MAX), note(0, 1)),
            Err(ModelError::UnknownClip)
        );
        assert_eq!(
            t.remove_note(ClipId(u64::MAX), &note(0, 1)),
            Err(ModelError::UnknownClip)
        );
    }

    #[test]
    fn track_clip_ids_are_not_reused_after_remove() {
        let mut t = track();
        let a = t.insert_clip(clip(0, 10)).unwrap();
        assert!(t.remove_clip(a).is_some());
        let b = t.insert_clip(clip(0, 10)).unwrap();
        assert_ne!(a, b);
        assert!(t.remove_clip(a).is_none());
    }
}
