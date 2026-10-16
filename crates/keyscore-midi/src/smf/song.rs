//! PPQ 960 중간 표현과 모델 변환.

use std::collections::BTreeMap;

use keyscore_core::{
    CcPoint, Channel, MeterMap, ModelError, Pitch, Program, Project, TempoMap, Tick, Velocity,
};

use super::raw::{SmfError, SmfFile, SmfFormat};

/// 곡 트랙의 노트. 시작은 곡의 절대 틱이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SongNote {
    start: Tick,
    length: u32,
    pitch: Pitch,
    velocity: Velocity,
}

impl SongNote {
    /// 길이 0은 1틱으로 둔다.
    #[must_use]
    pub fn new(start: Tick, length: u32, pitch: Pitch, velocity: Velocity) -> Self {
        SongNote {
            start,
            length: length.max(1),
            pitch,
            velocity,
        }
    }

    #[must_use]
    pub const fn start(&self) -> Tick {
        self.start
    }

    #[must_use]
    pub const fn length(&self) -> u32 {
        self.length
    }

    #[must_use]
    pub const fn pitch(&self) -> Pitch {
        self.pitch
    }

    #[must_use]
    pub const fn velocity(&self) -> Velocity {
        self.velocity
    }
}

/// 채널 하나에 묶인 곡 트랙. 열은 모두 틱 순이고, 같은 틱에서는 넣은 순서를 지킨다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongTrack {
    name: Option<String>,
    channel: Channel,
    notes: Vec<SongNote>,
    programs: Vec<(Tick, Program)>,
    cc: BTreeMap<u8, Vec<CcPoint>>,
    bend: Vec<CcPoint>,
    pressure: Vec<CcPoint>,
    lyrics: Vec<(Tick, String)>,
}

impl SongTrack {
    #[must_use]
    pub fn new(name: Option<String>, channel: Channel) -> Self {
        SongTrack {
            name,
            channel,
            notes: Vec::new(),
            programs: Vec::new(),
            cc: BTreeMap::new(),
            bend: Vec::new(),
            pressure: Vec::new(),
            lyrics: Vec::new(),
        }
    }

    pub fn add_note(&mut self, note: SongNote) {
        let _ = note;
        todo!()
    }

    pub fn add_program(&mut self, tick: Tick, program: Program) {
        let _ = (tick, program);
        todo!()
    }

    /// 컨트롤러 `controller`(0–127, 넘으면 무시)의 점. 값은 32비트다.
    pub fn add_cc(&mut self, controller: u8, point: CcPoint) {
        let _ = (controller, point);
        todo!()
    }

    /// 피치 벤드 점. 값은 부호 없는 32비트이고 가운데가 `0x8000_0000`이다.
    pub fn add_bend(&mut self, point: CcPoint) {
        let _ = point;
        todo!()
    }

    /// 채널 프레셔 점(32비트).
    pub fn add_pressure(&mut self, point: CcPoint) {
        let _ = point;
        todo!()
    }

    pub fn add_lyric(&mut self, tick: Tick, text: impl Into<String>) {
        let _ = (tick, text.into());
        todo!()
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    #[must_use]
    pub const fn channel(&self) -> Channel {
        self.channel
    }

    /// (시작, 음높이, …) 순으로 정렬된 노트.
    #[must_use]
    pub fn notes(&self) -> &[SongNote] {
        &self.notes
    }

    #[must_use]
    pub fn programs(&self) -> &[(Tick, Program)] {
        &self.programs
    }

    #[must_use]
    pub fn cc(&self) -> &BTreeMap<u8, Vec<CcPoint>> {
        &self.cc
    }

    #[must_use]
    pub fn bend(&self) -> &[CcPoint] {
        &self.bend
    }

    #[must_use]
    pub fn pressure(&self) -> &[CcPoint] {
        &self.pressure
    }

    #[must_use]
    pub fn lyrics(&self) -> &[(Tick, String)] {
        &self.lyrics
    }
}

/// PPQ 960 곡.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Song {
    tempo: TempoMap,
    meter: MeterMap,
    tracks: Vec<SongTrack>,
}

impl Song {
    #[must_use]
    pub fn new(tempo: TempoMap, meter: MeterMap) -> Self {
        Song {
            tempo,
            meter,
            tracks: Vec::new(),
        }
    }

    pub fn push_track(&mut self, track: SongTrack) {
        self.tracks.push(track);
    }

    #[must_use]
    pub fn tempo(&self) -> &TempoMap {
        &self.tempo
    }

    #[must_use]
    pub fn meter(&self) -> &MeterMap {
        &self.meter
    }

    #[must_use]
    pub fn tracks(&self) -> &[SongTrack] {
        &self.tracks
    }

    /// SMF에서 곡을 만든다(SEQ-6 재배율, SEQ-9 트랙 나누기).
    ///
    /// # Errors
    ///
    /// 재배율한 틱이 u64를 넘으면 [`SmfError::TooManyEvents`]가 아니라 그 이벤트를 버린다.
    /// 지금은 오류를 내지 않지만, 형식 검사를 늘릴 자리로 `Result`를 둔다.
    pub fn from_smf(file: &SmfFile) -> Result<Song, SmfError> {
        let _ = file;
        todo!()
    }

    /// SMF로 쓴다. Type 1은 첫 트랙에 템포·박자표를 두고 곡 트랙마다 하나씩, Type 0은 하나로 합친다.
    #[must_use]
    pub fn to_smf(&self, format: SmfFormat) -> SmfFile {
        let _ = format;
        todo!()
    }

    /// 모델 프로젝트로 옮긴다. 노트가 있는 마디를 이어 클립으로 묶는다(SEQ-9). 레인(CC·벤드·프레셔)과
    /// 노트에 붙지 않은 가사는 옮기지 않는다(S6).
    ///
    /// # Errors
    ///
    /// 모델 불변식을 어기는 값(틱 넘침 등)이면 [`ModelError`].
    pub fn to_project(&self) -> Result<Project, ModelError> {
        todo!()
    }

    /// 모델 프로젝트를 펼친다. 숨김 꼬리는 클립 끝에서 자른다.
    #[must_use]
    pub fn from_project(project: &Project) -> Song {
        let _ = project;
        todo!()
    }
}
