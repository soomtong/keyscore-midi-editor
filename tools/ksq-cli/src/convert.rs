use std::fmt;

use keyscore_core::Project;
use keyscore_text::{Diagnostic, Mode};

use crate::CliError;

/// `ksq2txt`가 텍스트로 옮기지 못한 정보 한 가지.
///
/// 텍스트는 헤더 하나(템포·박자·조)와 트랙마다 시작 0인 클립 하나, 노트의 (시작, 길이, 음높이,
/// 가사)만 나른다(REQ 5.14). 손실이 하나도 없으면 `txt2ksq`로 되읽은 프로젝트의 `.ksq`가 원본과
/// 바이트까지 같다. `track`은 0부터 센 트랙 순번이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loss {
    /// 틱 0 뒤의 템포 변화.
    TempoChanges { dropped: usize },
    /// 틱 0 뒤의 박자표 변화.
    MeterChanges { dropped: usize },
    /// 클립이 여럿이거나 0에서 시작하지 않아, 시작 0 클립 하나로 합쳤다. 빈 곳은 쉼표가 된다.
    ClipLayout {
        track: usize,
        name: String,
        clips: usize,
        first_start: u64,
    },
    /// 클립 이름·색.
    ClipAttributes { track: usize, name: String },
    /// 클립 끝을 넘는 노트 꼬리(분할로 생긴 숨김 꼬리)를 잘랐다.
    HiddenTails {
        track: usize,
        name: String,
        notes: usize,
    },
    /// 벨로시티가 텍스트 기본값(MIDI 1.0 100)이 아닌 노트. 내보내기는 `v`를 쓰지 않는다.
    Velocities {
        track: usize,
        name: String,
        notes: usize,
    },
    /// 피치 오프셋이 있는 노트.
    PitchOffsets {
        track: usize,
        name: String,
        notes: usize,
    },
    /// 텍스트가 나르지 않는 트랙 속성 가운데 `txt2ksq`의 기본값과 다른 것.
    TrackAttributes {
        track: usize,
        name: String,
        attributes: Vec<&'static str>,
    },
    /// 이 도구가 모르는 `.ksq` 키(경로 목록).
    UnknownKeys { paths: Vec<String> },
    /// 클립이 없는 트랙. 텍스트 블록은 비어 있을 수 없어 한 마디 쉼표 클립으로 적었다.
    EmptyTrack { track: usize, name: String },
    /// 같은 이름의 트랙은 텍스트에서 한 트랙의 성부로 합쳐지므로(결정 Q2) 이름을 바꿨다.
    RenamedTrack {
        track: usize,
        from: String,
        to: String,
    },
    /// 클립 길이가 쉼표로 닿지 않아 가장 가까운 길이로 적었다
    /// (`docs/decisions/2026-10-12-ktext-implementation-details.md`).
    ClipLength {
        track: usize,
        name: String,
        from: u64,
        to: u64,
    },
    /// 되읽은 결과가 위 손실로 설명되지 않게 달라졌다. `keyscore-text`가 어긋난 것이다.
    Mismatch { detail: String },
}

impl fmt::Display for Loss {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = f;
        todo!()
    }
}

/// 프로젝트를 계이름 텍스트로 쓴다. 텍스트가 나르지 않는 정보는 [`Loss`]로 돌려준다.
///
/// # Errors
///
/// 내보낸 텍스트를 다시 읽지 못하면 [`CliError::ExportUnreadable`], 노트를 합친 클립을 만들 수
/// 없으면 [`CliError::Model`].
pub fn project_to_text(project: &Project) -> Result<(String, Vec<Loss>), CliError> {
    let _ = project;
    todo!()
}

/// 계이름 텍스트를 프로젝트로 읽는다. 헤더는 템포·박자 맵의 틱 0 값과 조가 되고, 트랙 블록마다
/// 트랙 하나와 시작 0 클립 하나가 된다. 텍스트에 없는 헤더는 [`Project::new`]의 기본값이다.
///
/// # Errors
///
/// 문법 오류면 [`CliError::Parse`].
pub fn text_to_project(text: &str, mode: Mode) -> Result<(Project, Vec<Diagnostic>), CliError> {
    let _ = (text, mode);
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyscore_core::{
        Bank, Channel, Clip, MeterMap, Note, Pitch, PitchOffset, Program, Tempo, TempoMap, Tick,
        TimeSignature, Track, TrackColor, Velocity, ksq,
    };

    fn note(start: u64, length: u32, pitch: u8) -> Note {
        Note::new(
            Tick::new(start),
            length,
            Pitch::new(pitch).unwrap(),
            Velocity::from_midi1(100).unwrap(),
        )
        .unwrap()
    }

    fn clip(start: u64, length: u64, notes: Vec<Note>) -> Clip {
        let mut c = Clip::new(Tick::new(start), length).unwrap();
        for n in notes {
            c.insert_note(n).unwrap();
        }
        c
    }

    /// `txt2ksq`가 만드는 것과 같은 속성의 트랙(색 = 순번 % 12).
    fn track(index: usize, name: &str, clips: Vec<Clip>) -> Track {
        let color = TrackColor::new(u8::try_from(index % 12).unwrap()).unwrap();
        let mut t = Track::new(name, Channel::new(1).unwrap(), color);
        for c in clips {
            t.insert_clip(c).unwrap();
        }
        t
    }

    fn project(tracks: Vec<Track>) -> Project {
        let mut p = Project::new();
        for t in tracks {
            p.push_track(t).unwrap();
        }
        p
    }

    #[test]
    fn ksq2txt_reports_losses_for_multi_clip_tempo_change_velocity() {
        let loud = Note::new(
            Tick::new(0),
            480,
            Pitch::new(64).unwrap(),
            Velocity::from_midi1(80).unwrap(),
        )
        .unwrap();
        let mut p = project(vec![track(
            0,
            "Lead",
            vec![
                clip(0, 960, vec![note(0, 480, 60)]),
                clip(1920, 960, vec![loud]),
            ],
        )]);
        p.tempo_map_mut()
            .set(Tick::new(3840), Tempo::from_bpm(90).unwrap());
        p.meter_map_mut()
            .set(Tick::new(3840), TimeSignature::new(3, 4).unwrap());

        let (text, losses) = project_to_text(&p).unwrap();

        assert!(
            losses.contains(&Loss::TempoChanges { dropped: 1 }),
            "{losses:?}"
        );
        assert!(
            losses.contains(&Loss::MeterChanges { dropped: 1 }),
            "{losses:?}"
        );
        assert!(
            losses.contains(&Loss::ClipLayout {
                track: 0,
                name: "Lead".to_owned(),
                clips: 2,
                first_start: 0,
            }),
            "{losses:?}"
        );
        assert!(
            losses.contains(&Loss::Velocities {
                track: 0,
                name: "Lead".to_owned(),
                notes: 1,
            }),
            "{losses:?}"
        );
        assert_eq!(losses.len(), 4, "{losses:?}");
        // 합친 클립은 0에서 두 번째 클립 끝까지이고, 노트는 트랙 기준 자리에 있다.
        let (back, _) = text_to_project(&text, Mode::Lenient).unwrap();
        let clips: Vec<_> = back.tracks()[0].clips().map(|(_, c)| c.clone()).collect();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].length(), 2880);
        let starts: Vec<_> = clips[0].notes().iter().map(|n| n.start().get()).collect();
        assert_eq!(starts, vec![0, 1920]);
    }

    #[test]
    fn ksq2txt_renames_duplicate_track_names_and_round_trips() {
        let p = project(vec![
            track(0, "A", vec![clip(0, 960, vec![note(0, 960, 60)])]),
            track(1, "A", vec![clip(0, 960, vec![note(0, 960, 62)])]),
            track(2, "A (2)", vec![clip(0, 960, vec![note(0, 960, 64)])]),
        ]);

        let (text, losses) = project_to_text(&p).unwrap();

        // 「A (2)」는 이미 있으므로 두 번째 「A」는 「A (3)」이 된다.
        assert_eq!(
            losses,
            vec![Loss::RenamedTrack {
                track: 1,
                from: "A".to_owned(),
                to: "A (3)".to_owned(),
            }]
        );
        let (back, warnings) = text_to_project(&text, Mode::Strict).unwrap();
        assert!(warnings.is_empty());
        let names: Vec<_> = back.tracks().iter().map(Track::name).collect();
        assert_eq!(names, vec!["A", "A (3)", "A (2)"]);
        let pitches: Vec<_> = back
            .tracks()
            .iter()
            .map(|t| t.clips().next().unwrap().1.notes()[0].pitch().get())
            .collect();
        assert_eq!(pitches, vec![60, 62, 64]);
    }

    #[test]
    fn ksq2txt_without_losses_round_trips_ksq_bytes() {
        let mut p = project(vec![
            track(
                0,
                "Lead",
                vec![clip(0, 1920, vec![note(0, 480, 60), note(480, 1440, 67)])],
            ),
            track(1, "Pad \"x\"", vec![clip(0, 960, vec![note(0, 960, 48)])]),
        ]);
        *p.tempo_map_mut() = TempoMap::new(Tempo::from_bpm(132).unwrap());
        *p.meter_map_mut() = MeterMap::new(TimeSignature::new(6, 8).unwrap());

        let (text, losses) = project_to_text(&p).unwrap();

        assert_eq!(losses, vec![]);
        let (back, _) = text_to_project(&text, Mode::Strict).unwrap();
        assert_eq!(ksq::to_string(&back), ksq::to_string(&p));
    }

    #[test]
    fn ksq2txt_empty_track_writes_one_bar_rest_and_reports_loss() {
        let p = project(vec![track(0, "Empty", vec![])]);

        let (text, losses) = project_to_text(&p).unwrap();

        assert_eq!(
            losses,
            vec![Loss::EmptyTrack {
                track: 0,
                name: "Empty".to_owned(),
            }]
        );
        let (back, _) = text_to_project(&text, Mode::Strict).unwrap();
        let clip = back.tracks()[0].clips().next().unwrap().1.clone();
        assert_eq!(clip.length(), 3840);
        assert!(clip.notes().is_empty());
    }

    #[test]
    fn ksq2txt_hidden_tail_offset_and_track_attributes_are_reported() {
        let mut c = Clip::new(Tick::ZERO, 1920).unwrap();
        c.insert_note(
            note(0, 960, 60)
                .with_offset(PitchOffset::new(PitchOffset::SEMITONE / 2).unwrap())
                .unwrap(),
        )
        .unwrap();
        c.set_name("verse");
        let mut t = track(0, "Lead", vec![c]);
        t.set_mute(true);
        t.set_lyric_lang(Some("ko".to_owned()));
        // 숨김 꼬리는 분할로만 생긴다.
        let id = t
            .insert_clip(clip(1920, 1920, vec![note(0, 1920, 62)]))
            .unwrap();
        t.split_clip(id, Tick::new(2880)).unwrap();
        let p = project(vec![t]);

        let (_, losses) = project_to_text(&p).unwrap();

        let name = "Lead".to_owned();
        for expected in [
            Loss::PitchOffsets {
                track: 0,
                name: name.clone(),
                notes: 1,
            },
            Loss::ClipAttributes {
                track: 0,
                name: name.clone(),
            },
            Loss::HiddenTails {
                track: 0,
                name: name.clone(),
                notes: 1,
            },
            Loss::TrackAttributes {
                track: 0,
                name: name.clone(),
                attributes: vec!["mute", "lyric_lang"],
            },
        ] {
            assert!(losses.contains(&expected), "{expected:?} 없음: {losses:?}");
        }
    }

    #[test]
    fn ksq2txt_unknown_keys_are_reported_with_paths() {
        let base = project(vec![track(
            0,
            "A",
            vec![clip(0, 960, vec![note(0, 960, 60)])],
        )]);
        let text = ksq::to_string(&base)
            .replacen(
                "\"format_version\": 1,",
                "\"format_version\": 1, \"x_top\": 1,",
                1,
            )
            .replacen("\"mute\": false,", "\"mute\": false, \"x_track\": true,", 1)
            .replacen(
                "[0, 960, 60, 51492]",
                "[0, 960, 60, 51492, {\"x_note\": 2}]",
                1,
            );
        let (p, _) = ksq::from_str(&text).unwrap();

        let (_, losses) = project_to_text(&p).unwrap();

        assert_eq!(
            losses,
            vec![Loss::UnknownKeys {
                paths: vec![
                    "x_top".to_owned(),
                    "tracks[0].x_track".to_owned(),
                    "tracks[0].clips[0].notes[0].x_note".to_owned(),
                ],
            }]
        );
    }

    #[test]
    fn ksq2txt_unreachable_clip_length_reports_rounded_length() {
        // 노트 끝 960 뒤 7틱은 쉼표(15틱 단위)로 닿지 않는다.
        let p = project(vec![track(
            0,
            "A",
            vec![clip(0, 967, vec![note(0, 960, 60)])],
        )]);

        let (_, losses) = project_to_text(&p).unwrap();

        assert_eq!(
            losses,
            vec![Loss::ClipLength {
                track: 0,
                name: "A".to_owned(),
                from: 967,
                to: 960,
            }]
        );
    }

    #[test]
    fn txt2ksq_missing_headers_use_project_defaults() {
        let (p, warnings) =
            text_to_project("@track A ch3 prog5 bank7\nc4\n", Mode::Strict).unwrap();

        assert!(warnings.is_empty());
        let fresh = Project::new();
        assert_eq!(p.tempo_map(), fresh.tempo_map());
        assert_eq!(p.meter_map(), fresh.meter_map());
        assert_eq!(p.key(), fresh.key());
        let t = &p.tracks()[0];
        assert_eq!(
            (
                t.channel().get(),
                t.program().map(Program::get),
                t.bank().map(Bank::get)
            ),
            (3, Some(5), Some(7))
        );
    }

    #[test]
    fn txt2ksq_lenient_returns_warnings() {
        let (_, warnings) = text_to_project("@track A ch1\nc4 q c4\n", Mode::Lenient).unwrap();

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn loss_display_names_track_by_one_based_number() {
        let loss = Loss::Velocities {
            track: 0,
            name: "Lead".to_owned(),
            notes: 3,
        };

        assert!(loss.to_string().starts_with("트랙 1 \"Lead\": "), "{loss}");
    }
}
