//! 가져오기·내보내기 결과 보고(S2-T7c, S1-T6 이월). 버린 것을 세어 사람에게 알린다.

use keyscore_core::{ModelError, Project, Velocity};

use super::raw::{SmfError, SmfEventKind, SmfFile, SmfFormat, SmfTrack};
use super::song::Song;

/// 모델로 옮기는 메타: 트랙 이름, 템포, 박자표, 가사.
const KEPT_META: [u8; 4] = [0x03, 0x51, 0x58, 0x05];

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
}

impl ExportReport {
    /// 잃는 것이 없다.
    #[must_use]
    pub fn is_lossless(&self) -> bool {
        self.hidden_tails == 0
            && self.pitch_offsets == 0
            && self.velocity_rounded == 0
            && self.empty_tracks == 0
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
    let mut file_lyrics: usize = 0;
    for event in file.tracks().iter().flat_map(SmfTrack::events) {
        match event.kind() {
            SmfEventKind::Meta { kind: 0x05, .. } => file_lyrics += 1,
            SmfEventKind::Meta { kind, .. } if KEPT_META.contains(kind) => {}
            SmfEventKind::Meta { .. } | SmfEventKind::SysEx { .. } => report.skipped_events += 1,
            SmfEventKind::Channel(m) if m.bytes()[0] & 0xF0 == 0xA0 => report.skipped_events += 1,
            SmfEventKind::Channel(_) => {}
        }
    }
    let mut song_lyrics: usize = 0;
    for st in song.tracks() {
        song_lyrics += st.lyrics().len();
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
            attached += clip.notes().iter().filter(|n| n.lyric().is_some()).count();
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
        for (_, clip) in track.clips() {
            for note in clip.notes() {
                notes += 1;
                if note.end() > clip.length() {
                    report.hidden_tails += 1;
                }
                if note.offset().units() != 0 {
                    report.pitch_offsets += 1;
                }
                let v = note.velocity();
                if Velocity::from_midi1(v.to_midi1()).ok() != Some(v) {
                    report.velocity_rounded += 1;
                }
            }
        }
        if notes == 0 && track.program().is_none() && track.bank().is_none() {
            report.empty_tracks += 1;
        }
    }
    Ok((file, report))
}
