//! PPQ 960 중간 표현과 모델 변환.

use std::collections::{BTreeMap, VecDeque};

use keyscore_core::{
    Bank, CcPoint, Channel, Clip, MeterMap, ModelError, Note, Pitch, Program, Project, Tempo,
    TempoMap, Tick, TimeSignature, Track, TrackColor, Velocity,
};

use super::raw::{SmfError, SmfEvent, SmfEventKind, SmfFile, SmfFormat, SmfTrack};
use crate::{Midi1Message, note_on, pitch_bend, program_change};

/// 곡의 최대 길이(PPQ 960 틱). 4분음표 약 447만 개로, 가장 느린 템포에서도 몇 년이다. 노트 길이(u32)가
/// 잘리지 않고, SMF로 쓸 때 가변 길이 수(28비트)를 넘는 델타를 채우는 메타가 트랙마다 16개를 넘지
/// 않는다. 이보다 긴 곡은 읽기·쓰기에서 `SmfError::TooLong`이다(S1-T6 리뷰 1 지적 4).
pub const MAX_SONG_TICKS: u64 = u32::MAX as u64;

/// SMF 트랙 수 상한(헤더 u16). Type 1은 템포 트랙 하나를 더 쓴다.
const MAX_SMF_TRACKS: usize = u16::MAX as usize;

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
        let at = self.notes.partition_point(|n| n <= &note);
        self.notes.insert(at, note);
    }

    pub fn add_program(&mut self, tick: Tick, program: Program) {
        let at = self.programs.partition_point(|p| p.0 <= tick);
        self.programs.insert(at, (tick, program));
    }

    /// 컨트롤러 `controller`(0–127, 넘으면 무시)의 점. 값은 32비트다.
    pub fn add_cc(&mut self, controller: u8, point: CcPoint) {
        if controller < 128 {
            insert_point(self.cc.entry(controller).or_default(), point);
        }
    }

    /// 피치 벤드 점. 값은 부호 없는 32비트이고 가운데가 `0x8000_0000`이다.
    pub fn add_bend(&mut self, point: CcPoint) {
        insert_point(&mut self.bend, point);
    }

    /// 채널 프레셔 점(32비트).
    pub fn add_pressure(&mut self, point: CcPoint) {
        insert_point(&mut self.pressure, point);
    }

    pub fn add_lyric(&mut self, tick: Tick, text: impl Into<String>) {
        let at = self.lyrics.partition_point(|l| l.0 <= tick);
        self.lyrics.insert(at, (tick, text.into()));
    }

    fn is_empty(&self) -> bool {
        self.notes.is_empty()
            && self.programs.is_empty()
            && self.cc.is_empty()
            && self.bend.is_empty()
            && self.pressure.is_empty()
            && self.lyrics.is_empty()
    }

    fn has_note_at(&self, tick: Tick) -> bool {
        let i = self.notes.partition_point(|n| n.start < tick);
        self.notes.get(i).is_some_and(|n| n.start == tick)
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
    /// 노트는 (채널, 음높이)마다 먼저 켠 것을 먼저 끈다. 벨로시티 0 Note On은 Note Off다. 끝나지 않은
    /// 노트는 트랙 끝에서 끝난다. 폴리 프레셔, `SysEx`, 이름·템포·박자표·가사 밖의 메타는 버린다.
    /// 채널 메시지가 없는 트랙의 가사(LilyPond·노래방 파일)는 가사 틱에 노트가 가장 많이 시작하는
    /// 곡 트랙에 붙인다.
    ///
    /// # Errors
    ///
    /// 재배율한 곡이 [`MAX_SONG_TICKS`]보다 길면 [`SmfError::TooLong`].
    pub fn from_smf(file: &SmfFile) -> Result<Song, SmfError> {
        let scale = Rescale(u128::from(file.ppq().max(1)));
        // 끝 틱은 트랙의 모든 이벤트 이상이다. 재배율한 끝이 상한 안이면 모든 틱이 상한 안이다.
        if let Some(tick) = file
            .tracks()
            .iter()
            .map(|t| scale.tick(t.end()))
            .find(|&t| t > MAX_SONG_TICKS)
        {
            return Err(SmfError::TooLong { tick });
        }
        let mut tempos = Vec::new();
        let mut meters = Vec::new();
        for track in file.tracks() {
            for e in track.events() {
                let SmfEventKind::Meta { kind, data } = e.kind() else {
                    continue;
                };
                let at = Tick::new(scale.tick(e.tick()));
                match (*kind, data.as_slice()) {
                    (0x51, &[a, b, c]) => {
                        // 잘못된 값(0 등)은 건너뛴다.
                        if let Ok(t) =
                            Tempo::from_micros_per_quarter(u32::from_be_bytes([0, a, b, c]))
                        {
                            tempos.push((at, t));
                        }
                    }
                    (0x58, &[nn, dd, ..]) if dd < 16 => {
                        if let Ok(sig) = TimeSignature::new(nn, 1u16 << dd) {
                            meters.push((at, sig));
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut song = Song::new(
            TempoMap::from_changes(Tempo::DEFAULT, tempos),
            MeterMap::from_changes(TimeSignature::COMMON, meters),
        );
        let mut orphan_lyrics = Vec::new();
        for track in file.tracks() {
            let name = match file.format() {
                SmfFormat::Parallel => track_name(track),
                // Type 0의 이름은 대개 곡 제목이라 채널 트랙에 붙이지 않는다.
                SmfFormat::Single => None,
            };
            // 곡 트랙이 남지 않는 SMF 트랙의 가사(가사만 담은 트랙, 폴리 프레셔·짝 없는 Note Off만 있는
            // 트랙)는 따로 모아 맞는 곡 트랙에 붙인다(S1-T6 리뷰 3 지적 3).
            let (tracks, orphans) = if has_channel_messages(track) {
                split_track(track, name.as_deref(), scale)
            } else {
                (Vec::new(), lyrics_of(track, scale))
            };
            song.tracks.extend(tracks);
            orphan_lyrics.push(orphans);
        }
        song.attach_lyrics(orphan_lyrics);
        Ok(song)
    }

    /// 가사 묶음마다, 가사 틱에 노트가 가장 많이 시작하는 트랙(같으면 앞 트랙)에 붙인다. 맞는 트랙이
    /// 없으면 첫 트랙에, 트랙이 없으면 버린다.
    ///
    /// 모든 묶음을 한 번에 센다. 곡 트랙의 노트 시작을 한 번씩만 보고, 그 틱에 가사가 있는 묶음에만
    /// 더하므로 비용은 (노트 + 가사 + 맞은 수)다. 묶음마다 노트 전체를 다시 훑으면 (묶음 × 노트)가
    /// 된다(S1-T6 리뷰 2 지적 2, 리뷰 3 지적 1).
    fn attach_lyrics(&mut self, groups: Vec<Vec<(Tick, String)>>) {
        let groups: Vec<_> = groups.into_iter().filter(|g| !g.is_empty()).collect();
        if groups.is_empty() || self.tracks.is_empty() {
            return;
        }
        // 틱 → (묶음 번호, 그 틱의 가사 수). 묶음 안의 가사는 틱 순이다.
        let mut per_tick: BTreeMap<Tick, Vec<(usize, usize)>> = BTreeMap::new();
        for (g, lyrics) in groups.iter().enumerate() {
            for (tick, _) in lyrics {
                let list = per_tick.entry(*tick).or_default();
                match list.last_mut() {
                    Some((group, count)) if *group == g => *count += 1,
                    _ => list.push((g, 1)),
                }
            }
        }
        let mut best = vec![(0usize, 0usize); groups.len()];
        let mut hits = vec![0usize; groups.len()];
        let mut touched = Vec::new();
        for (i, t) in self.tracks.iter().enumerate() {
            let mut last = None;
            for n in &t.notes {
                if last == Some(n.start) {
                    continue;
                }
                last = Some(n.start);
                for &(g, count) in per_tick.get(&n.start).into_iter().flatten() {
                    if hits[g] == 0 {
                        touched.push(g);
                    }
                    hits[g] += count;
                }
            }
            for g in touched.drain(..) {
                if hits[g] > best[g].1 {
                    best[g] = (i, hits[g]);
                }
                hits[g] = 0;
            }
        }
        // 한 번에 붙이고 바뀐 트랙만 한 번씩 안정 정렬한다. 같은 틱에서는 먼저 있던 가사가 앞이다.
        let mut changed = vec![false; self.tracks.len()];
        for (g, lyrics) in groups.into_iter().enumerate() {
            let target = best[g].0;
            self.tracks[target].lyrics.extend(lyrics);
            changed[target] = true;
        }
        for (t, changed) in self.tracks.iter_mut().zip(changed) {
            if changed {
                t.lyrics.sort_by_key(|l| l.0);
            }
        }
    }

    /// SMF로 쓴다. Type 1은 첫 트랙에 템포·박자표를 두고 곡 트랙마다 하나씩, Type 0은 하나로 합친다.
    ///
    /// # Errors
    ///
    /// 어떤 이벤트가 [`MAX_SONG_TICKS`] 뒤면 [`SmfError::TooLong`], 트랙이 SMF 헤더에 담기지 않으면
    /// [`SmfError::TooManyTracks`]. 조용히 자르거나 버리지 않는다.
    pub fn to_smf(&self, format: SmfFormat) -> Result<SmfFile, SmfError> {
        if format == SmfFormat::Parallel && self.tracks.len() >= MAX_SMF_TRACKS {
            return Err(SmfError::TooManyTracks {
                max: MAX_SMF_TRACKS - 1,
            });
        }
        let mut conductor = Vec::new();
        for (tick, tempo) in self.tempo.changes() {
            let [_, a, b, c] = tempo.micros_per_quarter().to_be_bytes();
            conductor.push(Keyed::meta(tick, Class::Conductor, 0x51, vec![a, b, c]));
        }
        for (tick, sig) in self.meter.changes() {
            let dd = u8::try_from(sig.denominator().trailing_zeros()).unwrap_or(2);
            // 클록 24개/메트로놈, 32분음표 8개/4분음표(흔한 기본값).
            let data = vec![sig.numerator(), dd, 24, 8];
            conductor.push(Keyed::meta(tick, Class::Conductor, 0x58, data));
        }
        let per_track: Vec<Vec<Keyed>> = self
            .tracks
            .iter()
            .map(|t| track_events(t, format == SmfFormat::Parallel))
            .collect();
        if let Some(tick) = conductor
            .iter()
            .chain(per_track.iter().flatten())
            .map(|e| e.tick)
            .find(|&t| t > MAX_SONG_TICKS)
        {
            return Err(SmfError::TooLong { tick });
        }
        let tracks = match format {
            SmfFormat::Parallel => std::iter::once(conductor)
                .chain(per_track)
                .map(finish_track)
                .collect(),
            SmfFormat::Single => {
                let mut all = conductor;
                all.extend(per_track.into_iter().flatten());
                vec![finish_track(all)]
            }
        };
        Ok(SmfFile::from_parts(format, 960, tracks))
    }

    /// 모델 프로젝트로 옮긴다. 노트가 있는 마디를 이어 클립으로 묶는다(SEQ-9). 레인(CC·벤드·프레셔)과
    /// 노트에 붙지 않은 가사는 옮기지 않는다(S6).
    ///
    /// # Errors
    ///
    /// 모델 불변식을 어기는 값(틱 넘침 등)이면 [`ModelError`].
    pub fn to_project(&self) -> Result<Project, ModelError> {
        let mut project = Project::new();
        *project.tempo_map_mut() = self.tempo.clone();
        *project.meter_map_mut() = self.meter.clone();
        for (i, st) in self.tracks.iter().enumerate() {
            let name = st
                .name
                .clone()
                .unwrap_or_else(|| format!("Track {}", i + 1));
            let color = TrackColor::new(
                u8::try_from(i % usize::from(TrackColor::PALETTE_LEN)).unwrap_or(0),
            )?;
            let mut track = Track::new(name, st.channel, color);
            let first_program = st.programs.first().copied();
            track.set_program(first_program.map(|p| p.1));
            track.set_bank(bank_before(st, first_program.map_or(Tick::ZERO, |p| p.0)));
            for (start, length, notes) in bar_groups(&self.meter, st) {
                let mut clip = Clip::new(start, length)?;
                let mut model_notes = Vec::with_capacity(notes.len());
                for (note, lyric) in notes {
                    let rel = Tick::new(note.start.get() - start.get());
                    let mut n = Note::new(rel, note.length, note.pitch, note.velocity)?;
                    if let Some(text) = lyric {
                        n = n.with_lyric(text);
                    }
                    model_notes.push(n);
                }
                // 클립의 정렬 순서(시작, 음높이, …)로 미리 정렬해 넣으면 늘 끝에 붙는다. 곡의 순서
                // (시작, 길이, …)로 넣으면 매번 앞쪽에 끼워 O(n²)이 된다(S1-T6 리뷰 2 지적 1).
                model_notes.sort();
                for n in model_notes {
                    clip.insert_note(n)?;
                }
                track.insert_clip(clip)?;
            }
            project.push_track(track)?;
        }
        Ok(project)
    }

    /// 모델 프로젝트를 펼친다. 숨김 꼬리는 클립 끝에서 자른다.
    #[must_use]
    pub fn from_project(project: &Project) -> Song {
        let mut song = Song::new(project.tempo_map().clone(), project.meter_map().clone());
        for track in project.tracks() {
            let mut st = SongTrack::new(Some(track.name().to_string()), track.channel());
            if let Some(bank) = track.bank() {
                let [msb, lsb] = [bank.get() >> 7, bank.get() & 0x7F]
                    .map(|v| u8::try_from(v & 0x7F).unwrap_or(0));
                st.add_cc(0, CcPoint::new(Tick::ZERO, upscale7(msb)));
                st.add_cc(32, CcPoint::new(Tick::ZERO, upscale7(lsb)));
            }
            if let Some(program) = track.program() {
                st.add_program(Tick::ZERO, program);
            }
            let mut lyrics = Vec::new();
            for (_, clip) in track.clips() {
                for note in clip.notes() {
                    let start = clip.start().get().saturating_add(note.start().get());
                    // 숨김 꼬리는 클립 끝에서 자른다(결정 2026-10-12 Q1).
                    let end = note.end().min(clip.length());
                    let length = u32::try_from(end - note.start().get()).unwrap_or(u32::MAX);
                    st.notes.push(SongNote::new(
                        Tick::new(start),
                        length,
                        note.pitch(),
                        note.velocity(),
                    ));
                    lyrics.push((
                        Tick::new(start),
                        std::cmp::Reverse(note.pitch()),
                        length,
                        note.velocity(),
                        note.lyric(),
                    ));
                }
            }
            // 노트는 한 번에 정렬한다. 같은 틱의 가사는 높은 음부터 둔다(`bar_groups`가 같은 순서로 붙인다).
            // SMF에는 가사가 어느 노트의 것인지 적을 자리가 없다. 그래서 가사가 있는 가장 낮은 음까지,
            // 가사 없는 위 음의 자리를 빈 가사로 채운다. 맨 위 음 하나에만 가사가 있으면 채울 것이 없다.
            st.notes.sort_unstable();
            // 숨김 꼬리가 잘려 (음높이, 길이)가 같아진 노트도 벨로시티로 가져오기와 같은 순서가 된다
            // (S1-T6 리뷰 3 지적 2). 넷이 모두 같은 노트는 SMF에서 구별할 수 없다.
            lyrics.sort_by_key(|l| (l.0, l.1, l.2, l.3));
            let mut i = 0;
            while i < lyrics.len() {
                let tick = lyrics[i].0;
                let same = lyrics[i..].partition_point(|l| l.0 == tick);
                let group = &lyrics[i..i + same];
                if let Some(last) = group.iter().rposition(|l| l.4.is_some()) {
                    for (_, _, _, _, text) in &group[..=last] {
                        st.add_lyric(tick, text.unwrap_or(""));
                    }
                }
                i += same;
            }
            song.push_track(st);
        }
        song
    }
}

/// 원본 PPQ → 960. 반올림한 정수 산술(SEQ-6).
#[derive(Clone, Copy)]
struct Rescale(u128);

impl Rescale {
    fn tick(self, t: u64) -> u64 {
        let scaled = (u128::from(t) * 960 + self.0 / 2) / self.0;
        u64::try_from(scaled).unwrap_or(u64::MAX)
    }
}

fn insert_point(list: &mut Vec<CcPoint>, point: CcPoint) {
    let at = list.partition_point(|p| p.tick() <= point.tick());
    list.insert(at, point);
}

/// 텍스트 메타 바이트. UTF-8이 아니면 Latin-1로 읽는다(옛 SMF의 흔한 인코딩).
fn decode_text(data: &[u8]) -> String {
    match std::str::from_utf8(data) {
        Ok(s) => s.to_string(),
        Err(_) => data.iter().map(|&b| char::from(b)).collect(),
    }
}

fn has_channel_messages(track: &SmfTrack) -> bool {
    track
        .events()
        .iter()
        .any(|e| matches!(e.kind(), SmfEventKind::Channel(_)))
}

fn lyrics_of(track: &SmfTrack, scale: Rescale) -> Vec<(Tick, String)> {
    track
        .events()
        .iter()
        .filter_map(|e| match e.kind() {
            SmfEventKind::Meta { kind: 0x05, data } => {
                Some((Tick::new(scale.tick(e.tick())), decode_text(data)))
            }
            _ => None,
        })
        .collect()
}

fn track_name(track: &SmfTrack) -> Option<String> {
    track.events().iter().find_map(|e| match e.kind() {
        SmfEventKind::Meta { kind: 0x03, data } => Some(decode_text(data)),
        _ => None,
    })
}

/// MIDI 2.0 min-center-max 확장: 7비트 → 32비트. `>> 25`로 되돌리면 원래 값이다.
fn upscale7(v: u8) -> u32 {
    upscale(u32::from(v & 0x7F), 7)
}

/// 14비트 → 32비트(벤드). 가운데 0x2000은 `0x8000_0000`이다. `>> 18`로 되돌린다.
fn upscale14(v: u16) -> u32 {
    upscale(u32::from(v & 0x3FFF), 14)
}

fn upscale(v: u32, bits: u32) -> u32 {
    // MIDI 2.0 명세(M2-115) 「min-center-max」 확장 알고리즘을 그대로 옮긴다.
    let scale_bits = 32 - bits;
    let mut out = v << scale_bits;
    let center = 1u32 << (bits - 1);
    if v <= center {
        return out;
    }
    // 가운데 위는 가운데 아래 비트를 빈 하위 비트에 되풀이해 채운다(꼭대기가 0xFFFF_FFFF).
    let repeat_bits = bits - 1;
    let mut repeat = v & (center - 1);
    if scale_bits > repeat_bits {
        repeat <<= scale_bits - repeat_bits;
    } else {
        repeat >>= repeat_bits - scale_bits;
    }
    while repeat != 0 {
        out |= repeat;
        repeat >>= repeat_bits;
    }
    out
}

fn downscale7(v: u32) -> u8 {
    u8::try_from(v >> 25).unwrap_or(0x7F)
}

/// SMF 트랙 하나를 채널마다 곡 트랙으로 나눈다. 채널 메시지가 없는 채널은 만들지 않는다.
/// 곡 트랙이 하나도 남지 않으면 가사를 둘째 값으로 돌려준다(전역으로 붙인다).
fn split_track(
    track: &SmfTrack,
    name: Option<&str>,
    scale: Rescale,
) -> (Vec<SongTrack>, Vec<(Tick, String)>) {
    let mut by_channel: Vec<Option<SongTrack>> = (0..16).map(|_| None).collect();
    // (채널, 음높이)마다 켜진 노트(원본 틱, 벨로시티).
    let mut open: Vec<VecDeque<(u64, u8)>> = (0..16 * 128).map(|_| VecDeque::new()).collect();
    let mut lyrics = Vec::new();
    for e in track.events() {
        match e.kind() {
            SmfEventKind::Channel(m) => {
                let b = m.bytes();
                let ch = usize::from(b[0] & 0x0F);
                let st = by_channel[ch].get_or_insert_with(|| {
                    let channel = Channel::new((b[0] & 0x0F) + 1).unwrap_or(Channel::DRUMS);
                    SongTrack::new(name.map(str::to_string), channel)
                });
                let at = Tick::new(scale.tick(e.tick()));
                let d1 = b.get(1).copied().unwrap_or(0);
                let d2 = b.get(2).copied().unwrap_or(0);
                match b[0] & 0xF0 {
                    0x90 if d2 > 0 => open[ch * 128 + usize::from(d1)].push_back((e.tick(), d2)),
                    0x80 | 0x90 => {
                        if let Some((start, v)) = open[ch * 128 + usize::from(d1)].pop_front() {
                            st.notes.push(make_note(scale, start, e.tick(), d1, v));
                        }
                    }
                    0xB0 => st.add_cc(d1, CcPoint::new(at, upscale7(d2))),
                    0xC0 => {
                        if let Ok(p) = Program::new(d1) {
                            st.add_program(at, p);
                        }
                    }
                    0xD0 => st.add_pressure(CcPoint::new(at, upscale7(d1))),
                    0xE0 => st.add_bend(CcPoint::new(
                        at,
                        upscale14(u16::from(d1) | u16::from(d2) << 7),
                    )),
                    _ => {}
                }
            }
            SmfEventKind::Meta { kind: 0x05, data } => {
                lyrics.push((Tick::new(scale.tick(e.tick())), decode_text(data)));
            }
            _ => {}
        }
    }
    // 끝나지 않은 노트는 트랙 끝에서 끝낸다.
    for (key, queue) in open.iter_mut().enumerate() {
        while let Some((start, v)) = queue.pop_front() {
            if let Some(st) = by_channel[key / 128].as_mut() {
                let end = track.end().max(start);
                let pitch = u8::try_from(key % 128).unwrap_or(0);
                st.notes.push(make_note(scale, start, end, pitch, v));
            }
        }
    }
    // 노트는 끝나는 순서로 모였다. 한 번에 정렬한다(끄는 순서가 켜는 순서와 달라도 정렬 삽입이
    // O(n²)이 되지 않게, S1-T6 리뷰 1 지적 1).
    for st in by_channel.iter_mut().flatten() {
        st.notes.sort_unstable();
    }
    let mut tracks: Vec<SongTrack> = by_channel
        .into_iter()
        .flatten()
        .filter(|t| !t.is_empty())
        .collect();
    if tracks.is_empty() {
        return (tracks, lyrics);
    }
    // 가사는 그 틱에 노트가 시작하는 채널 트랙(낮은 채널부터)에, 없으면 첫 트랙에 붙인다.
    for (tick, text) in lyrics {
        let target = tracks
            .iter()
            .position(|t| t.has_note_at(tick))
            .or(if tracks.is_empty() { None } else { Some(0) });
        if let Some(i) = target {
            tracks[i].add_lyric(tick, text);
        }
    }
    (tracks, Vec::new())
}

fn make_note(scale: Rescale, start: u64, end: u64, pitch: u8, velocity: u8) -> SongNote {
    let s = scale.tick(start);
    let length = u32::try_from(scale.tick(end).saturating_sub(s)).unwrap_or(u32::MAX);
    SongNote::new(
        Tick::new(s),
        length,
        Pitch::new(pitch & 0x7F).unwrap_or(Pitch::MIN),
        Velocity::from_midi1(velocity & 0x7F).unwrap_or(Velocity::new(0)),
    )
}

/// 같은 틱에서의 쓰기 순서. 스케줄러 정렬과 같이 Note Off가 먼저, Note On이 나중이다. 뱅크(CC 0·32)는
/// 프로그램보다 먼저 가야 신디가 뱅크를 바꾼 뒤 프로그램을 고른다.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    Conductor,
    Name,
    NoteOff,
    Control,
    Program,
    Bend,
    Pressure,
    Lyric,
    NoteOn,
}

struct Keyed {
    tick: u64,
    class: Class,
    kind: SmfEventKind,
}

impl Keyed {
    fn meta(tick: Tick, class: Class, kind: u8, data: Vec<u8>) -> Keyed {
        Keyed {
            tick: tick.get(),
            class,
            kind: SmfEventKind::Meta { kind, data },
        }
    }

    fn channel(tick: Tick, class: Class, msg: Midi1Message) -> Keyed {
        Keyed {
            tick: tick.get(),
            class,
            kind: SmfEventKind::Channel(msg),
        }
    }
}

fn track_events(t: &SongTrack, with_name: bool) -> Vec<Keyed> {
    let ch = t.channel;
    let status = ch.get() - 1;
    let mut out = Vec::new();
    if with_name && let Some(name) = &t.name {
        out.push(Keyed::meta(
            Tick::ZERO,
            Class::Name,
            0x03,
            name.as_bytes().to_vec(),
        ));
    }
    for n in &t.notes {
        out.push(Keyed::channel(
            n.start,
            Class::NoteOn,
            note_on(ch, n.pitch, n.velocity),
        ));
        let end = Tick::new(n.start.get().saturating_add(u64::from(n.length)));
        let off = Midi1Message::from_status(0x80 | status, n.pitch.get(), 0);
        out.push(Keyed::channel(end, Class::NoteOff, off));
    }
    for (controller, points) in &t.cc {
        for p in points {
            let msg = Midi1Message::from_status(0xB0 | status, *controller, downscale7(p.value()));
            out.push(Keyed::channel(p.tick(), Class::Control, msg));
        }
    }
    for (tick, program) in &t.programs {
        out.push(Keyed::channel(
            *tick,
            Class::Program,
            program_change(ch, *program),
        ));
    }
    for p in &t.bend {
        out.push(Keyed::channel(
            p.tick(),
            Class::Bend,
            pitch_bend(ch, p.value()),
        ));
    }
    for p in &t.pressure {
        let msg = Midi1Message::from_status(0xD0 | status, downscale7(p.value()), 0);
        out.push(Keyed::channel(p.tick(), Class::Pressure, msg));
    }
    for (tick, text) in &t.lyrics {
        out.push(Keyed::meta(
            *tick,
            Class::Lyric,
            0x05,
            text.as_bytes().to_vec(),
        ));
    }
    out
}

/// (틱, 무리)로 안정 정렬해 트랙으로 만든다. 같은 무리 안에서는 넣은 순서를 지킨다.
fn finish_track(mut events: Vec<Keyed>) -> SmfTrack {
    events.sort_by_key(|e| (e.tick, e.class));
    let end = events.last().map_or(0, |e| e.tick);
    let events = events
        .into_iter()
        .map(|e| SmfEvent::new(e.tick, e.kind))
        .collect();
    SmfTrack::from_parts(events, end)
}

/// 첫 프로그램 틱(없으면 0)까지의 마지막 CC 0·32로 뱅크를 만든다. CC 0이 없으면 뱅크 없음.
fn bank_before(t: &SongTrack, until: Tick) -> Option<Bank> {
    let last = |cc: u8| {
        t.cc.get(&cc)
            .and_then(|points| points.iter().rev().find(|p| p.tick() <= until))
            .map(|p| u16::from(downscale7(p.value())))
    };
    let msb = last(0)?;
    Bank::new((msb << 7) | last(32).unwrap_or(0)).ok()
}

type NoteWithLyric = (SongNote, Option<String>);

/// 노트가 걸친 마디를 이어 (클립 시작, 길이, 노트와 가사) 묶음으로 나눈다(SEQ-9).
fn bar_groups(meter: &MeterMap, t: &SongTrack) -> Vec<(Tick, u64, Vec<NoteWithLyric>)> {
    // 가사는 그 틱에 시작하는 노트에 높은 음부터 차례로 붙인다. 가사가 하나면 가장 높은 음(선율)이다.
    // 화음에 가사가 여럿이면 `from_project`가 높은 음부터 내보낸 순서와 맞는다(리뷰 1 제안 5).
    // 노트 하나가 가사 한 자리다. 자리 순서는 (높은 음, 짧은 길이, 작은 벨로시티)이고 `from_project`가 같은 순서로
    // 내보낸다. 같은 음높이·길이의 노트가 겹쳐도 자리를 따로 센다(S1-T6 리뷰 2 지적 3).
    let mut lyric_of: BTreeMap<(Tick, Pitch, u32, Velocity), VecDeque<String>> = BTreeMap::new();
    let mut i = 0;
    while i < t.lyrics.len() {
        let tick = t.lyrics[i].0;
        let same = t.lyrics[i..].partition_point(|l| l.0 == tick);
        // 노트는 시작 순이라 그 틱의 노트를 이분 탐색으로 찾는다(가사 수 × 노트 수가 되지 않게).
        let from = t.notes.partition_point(|n| n.start < tick);
        let to = t.notes.partition_point(|n| n.start <= tick);
        let mut slots: Vec<(Pitch, u32, Velocity)> = t.notes[from..to]
            .iter()
            .map(|n| (n.pitch, n.length, n.velocity))
            .collect();
        slots.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        for ((p, len, v), (_, text)) in slots.into_iter().zip(&t.lyrics[i..i + same]) {
            // 빈 가사는 자리만 채운다(`from_project`가 가사 없는 위 음 자리에 넣는다).
            if !text.is_empty() {
                lyric_of
                    .entry((tick, p, len, v))
                    .or_default()
                    .push_back(text.clone());
            }
        }
        i += same;
    }
    let mut groups: Vec<(u64, u64, Vec<NoteWithLyric>)> = Vec::new();
    let mut by_bar: Vec<(u64, u64, SongNote)> = t
        .notes
        .iter()
        .map(|n| {
            let end = n.start.get().saturating_add(u64::from(n.length));
            let first = meter.position(n.start).bar();
            let last = meter.position(Tick::new(end - 1)).bar();
            (first, last, *n)
        })
        .collect();
    by_bar.sort_by_key(|g| (g.0, g.2));
    for (first, last, note) in by_bar {
        let lyric = lyric_of
            .get_mut(&(note.start, note.pitch, note.length, note.velocity))
            .and_then(VecDeque::pop_front);
        match groups.last_mut() {
            Some(g) if first <= g.1.saturating_add(1) => {
                g.1 = g.1.max(last);
                g.2.push((note, lyric));
            }
            _ => groups.push((first, last, vec![(note, lyric)])),
        }
    }
    groups
        .into_iter()
        .map(|(first, last, notes)| {
            let start = meter.bar_start(first).unwrap_or(Tick::ZERO);
            let note_end = notes
                .iter()
                .map(|(n, _)| n.start.get().saturating_add(u64::from(n.length)))
                .max()
                .unwrap_or(start.get() + 1);
            let end = meter
                .bar_start(last.saturating_add(1))
                .map_or(note_end, |t| t.get().max(note_end));
            (start, end - start.get(), notes)
        })
        .collect()
}
