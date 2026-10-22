//! 가져오기·내보내기 결과 보고(S2-T7c, S1-T6 이월). 버린 것을 세어 사람에게 알린다.

use std::collections::BTreeMap;

use keyscore_core::{ModelError, Project, Tempo, TimeSignature, Velocity};

use super::raw::{SmfError, SmfEventKind, SmfFile, SmfFormat};
use super::song::Song;

/// 모델로 옮기는 메타인가: 곡 트랙이 되는 Type 1 트랙의 첫 이름, 올바른 템포, 올바른 박자표(가사는 따로 센다).
/// 지휘 트랙·Type 0의 제목, 0 µs 템포, 잘못된 박자표는 `Song::from_smf`가 버린다(리뷰 1 제안 6).
fn kept_meta(
    kind: u8,
    data: &[u8],
    format: SmfFormat,
    has_channel: bool,
    named: &mut bool,
) -> bool {
    match (kind, data) {
        (0x03, _) => {
            let kept = format == SmfFormat::Parallel && has_channel && !*named;
            *named = true;
            kept
        }
        (0x51, &[a, b, c]) => {
            Tempo::from_micros_per_quarter(u32::from_be_bytes([0, a, b, c])).is_ok()
        }
        (0x58, &[nn, dd, ..]) => dd < 16 && TimeSignature::new(nn, 1u16 << dd).is_ok(),
        _ => false,
    }
}

/// SMF 가져오기에서 모델로 옮기지 못한 것.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// 만든 트랙 수.
    pub tracks: usize,
    /// 옮긴 노트 수.
    pub notes: usize,
    /// 붙일 곡 트랙이 없어 버린 가사.
    pub orphan_lyrics: usize,
    /// 노트 시작에 붙지 않아 버린 가사.
    pub unattached_lyrics: usize,
    /// 레인(CC·벤드·채널 프레셔) 포인트. 모델의 레인은 S6이다. 뱅크로 쓴 CC 0·32는 세지 않는다.
    pub lane_points: usize,
    /// 첫 번째 뒤의 프로그램 변경(트랙은 프로그램 하나만 가진다).
    pub extra_programs: usize,
    /// 폴리 프레셔, `SysEx`, 이름·템포·박자표·가사 밖의 메타.
    pub skipped_events: usize,
}

impl ImportReport {
    /// 버린 것이 없다.
    #[must_use]
    pub fn is_lossless(&self) -> bool {
        self.orphan_lyrics == 0
            && self.unattached_lyrics == 0
            && self.lane_points == 0
            && self.extra_programs == 0
            && self.skipped_events == 0
    }
}

/// SMF 내보내기에서 잃는 것.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportReport {
    /// 클립 끝에서 자른 숨김 꼬리.
    pub hidden_tails: usize,
    /// 피치 오프셋을 잃은 노트.
    pub pitch_offsets: usize,
    /// 7비트로 줄며 값이 바뀐 벨로시티.
    pub velocity_rounded: usize,
    /// 노트·프로그램·뱅크가 없어 채널 메시지가 없는 트랙(다른 프로그램이 읽을 때 사라질 수 있다).
    pub empty_tracks: usize,
    /// 같은 트랙에서 같은 음높이의 앞 노트가 끝나기 전에 시작하는 노트. SMF의 Note On/Off 짝으로는 겹침을
    /// 나타낼 수 없어 다시 읽으면 길이가 바뀐다(리뷰 1 지적 5).
    pub overlapping_notes: usize,
}

impl ExportReport {
    /// 잃는 것이 없다.
    #[must_use]
    pub fn is_lossless(&self) -> bool {
        self.hidden_tails == 0
            && self.pitch_offsets == 0
            && self.velocity_rounded == 0
            && self.empty_tracks == 0
            && self.overlapping_notes == 0
    }
}

/// 가져오기 오류.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("{0}")]
    Smf(#[from] SmfError),
    #[error("{0}")]
    Model(#[from] ModelError),
}

/// SMF를 모델 프로젝트로 옮기고 버린 것을 센다. 결과는 `Song::from_smf(file)?.to_project()?`와 같다.
///
/// # Errors
///
/// [`ImportError`].
pub fn smf_import(file: &SmfFile) -> Result<(Project, ImportReport), ImportError> {
    let song = Song::from_smf(file)?;
    let project = song.to_project()?;
    let mut report = ImportReport {
        tracks: project.tracks().len(),
        ..ImportReport::default()
    };
    // 가사는 빈 것을 세지 않는다. 우리 내보내기는 화음에서 가사 있는 음 위의 음 자리를 빈 가사로 채운다. 다시
    // 가져오면 그 빈 가사는 노트에 붙지 않지만 잃은 것은 없다(리뷰 1 지적 3).
    let mut file_lyrics: usize = 0;
    for track in file.tracks() {
        let has_channel = track
            .events()
            .iter()
            .any(|e| matches!(e.kind(), SmfEventKind::Channel(_)));
        let mut named = false;
        for event in track.events() {
            match event.kind() {
                SmfEventKind::Meta { kind: 0x05, data } => {
                    if !data.is_empty() {
                        file_lyrics += 1;
                    }
                }
                SmfEventKind::Meta { kind, data }
                    if kept_meta(*kind, data, file.format(), has_channel, &mut named) => {}
                SmfEventKind::Meta { .. } | SmfEventKind::SysEx { .. } => {
                    report.skipped_events += 1;
                }
                SmfEventKind::Channel(m) if m.bytes()[0] & 0xF0 == 0xA0 => {
                    report.skipped_events += 1;
                }
                SmfEventKind::Channel(_) => {}
            }
        }
    }
    let mut song_lyrics: usize = 0;
    for st in song.tracks() {
        song_lyrics += st
            .lyrics()
            .iter()
            .filter(|(_, text)| !text.is_empty())
            .count();
        report.lane_points += st
            .cc()
            .iter()
            .filter(|(controller, _)| !matches!(controller, 0 | 32))
            .map(|(_, points)| points.len())
            .sum::<usize>()
            + st.bend().len()
            + st.pressure().len();
        report.extra_programs += st.programs().len().saturating_sub(1);
    }
    let mut attached: usize = 0;
    for track in project.tracks() {
        for (_, clip) in track.clips() {
            report.notes += clip.notes().len();
            attached += clip
                .notes()
                .iter()
                .filter(|n| n.lyric().is_some_and(|l| !l.is_empty()))
                .count();
        }
    }
    report.orphan_lyrics = file_lyrics.saturating_sub(song_lyrics);
    report.unattached_lyrics = song_lyrics.saturating_sub(attached);
    Ok((project, report))
}

/// 모델 프로젝트를 SMF로 쓰고 잃는 것을 센다. 결과는 `Song::from_project(project).to_smf(format)`와 같다.
///
/// # Errors
///
/// [`SmfError`].
pub fn smf_export(
    project: &Project,
    format: SmfFormat,
) -> Result<(SmfFile, ExportReport), SmfError> {
    let file = Song::from_project(project).to_smf(format)?;
    let mut report = ExportReport::default();
    for track in project.tracks() {
        let mut notes = 0;
        let mut spans: BTreeMap<keyscore_core::Pitch, Vec<(u64, u64)>> = BTreeMap::new();
        for (_, clip) in track.clips() {
            for note in clip.notes() {
                notes += 1;
                if note.end() > clip.length() {
                    report.hidden_tails += 1;
                }
                if note.offset().units() != 0 {
                    report.pitch_offsets += 1;
                }
                // SMF의 Note On 벨로시티는 1 이상이다(0은 Note Off). 0은 1로 나간다(리뷰 1 제안 6).
                let v = note.velocity();
                if Velocity::from_midi1(v.to_midi1().max(1)).ok() != Some(v) {
                    report.velocity_rounded += 1;
                }
                // 펼친 위치(클립 끝에서 자른 길이)로 음높이마다 모은다. `Song::from_project`와 같다.
                let start = clip.start().get().saturating_add(note.start().get());
                let end = clip
                    .start()
                    .get()
                    .saturating_add(note.end().min(clip.length()));
                spans.entry(note.pitch()).or_default().push((start, end));
            }
        }
        for list in spans.values_mut() {
            list.sort_unstable();
            let mut reach = 0;
            for (i, &(start, end)) in list.iter().enumerate() {
                if i > 0 && start < reach {
                    report.overlapping_notes += 1;
                }
                reach = reach.max(end);
            }
        }
        if notes == 0 && track.program().is_none() && track.bank().is_none() {
            report.empty_tracks += 1;
        }
    }
    Ok((file, report))
}
