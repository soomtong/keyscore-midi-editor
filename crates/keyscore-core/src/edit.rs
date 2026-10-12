use crate::{
    Bank, Channel, Clip, ClipId, Key, ModelError, Note, OutputPort, Program, Project, Tempo, Tick,
    TimeSignature, Track, TrackColor, Velocity,
};

/// 트랙의 속성 하나(REQ SEQ-7). [`Edit::SetTrackProp`]이 바꾼다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackProp {
    Name(String),
    Color(TrackColor),
    Port(OutputPort),
    Channel(Channel),
    Bank(Option<Bank>),
    Program(Option<Program>),
    DefaultVelocity(Velocity),
    Mute(bool),
    Solo(bool),
    LyricLang(Option<String>),
}

/// 모델을 바꾸는 원시 편집. 적용하면 역편집이 나온다(EDT-1).
///
/// 모든 변형은 모델의 검사 API를 거쳐 적용된다. 그래서 undo·redo로도 불변식(겹침 없음,
/// 노트는 클립 안, 값 범위)을 깨뜨릴 수 없다. 트랙은 인덱스로, 클립은 [`ClipId`]로 가리킨다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    InsertTrack {
        index: usize,
        track: Box<Track>,
    },
    RemoveTrack {
        index: usize,
    },
    SetTrackProp {
        track: usize,
        prop: TrackProp,
    },
    /// `id`가 `None`이면 새로 발급한다. 역편집(`RemoveClip`)에 발급된 id가 들어가므로,
    /// redo는 같은 id로 다시 넣는다.
    InsertClip {
        track: usize,
        clip: Box<Clip>,
        id: Option<ClipId>,
    },
    RemoveClip {
        track: usize,
        id: ClipId,
    },
    MoveClip {
        track: usize,
        id: ClipId,
        to: Tick,
    },
    /// `right_id`가 `None`이면 오른쪽 클립 id를 새로 발급한다. 역편집(`UnsplitClip`)에 기록된다.
    SplitClip {
        track: usize,
        id: ClipId,
        at: Tick,
        right_id: Option<ClipId>,
    },
    /// 분할의 역. 오른쪽 클립을 지우고 왼쪽 클립을 분할 전 원본으로 되돌린다.
    UnsplitClip {
        track: usize,
        left: ClipId,
        right: ClipId,
        original: Box<Clip>,
    },
    SetClipName {
        track: usize,
        id: ClipId,
        name: Option<String>,
    },
    SetClipColor {
        track: usize,
        id: ClipId,
        color: Option<TrackColor>,
    },
    InsertNote {
        track: usize,
        clip: ClipId,
        note: Note,
    },
    RemoveNote {
        track: usize,
        clip: ClipId,
        note: Note,
    },
    /// `tempo`가 `None`이면 그 틱의 변화를 지운다.
    SetTempo {
        at: Tick,
        tempo: Option<Tempo>,
    },
    /// `signature`가 `None`이면 그 틱의 변화를 지운다.
    SetMeter {
        at: Tick,
        signature: Option<TimeSignature>,
    },
    SetKey(Key),
}

impl Edit {
    /// 적용하고 역편집을 돌려준다.
    ///
    /// # Errors
    ///
    /// 대상이 없거나 불변식을 깨뜨리는 편집이면 [`ModelError`]. 이때 프로젝트는 바뀌지 않는다.
    pub fn apply(self, project: &mut Project) -> Result<Edit, ModelError> {
        todo!("{self:?} {}", project.tracks().len())
    }

    /// 히스토리 메모리 상한 계산용 추정 크기(바이트).
    #[must_use]
    pub fn approx_bytes(&self) -> usize {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Pitch;

    fn note(start: u64) -> Note {
        Note::new(
            Tick::new(start),
            10,
            Pitch::new(60).unwrap(),
            Velocity::new(1000),
        )
        .unwrap()
    }

    /// 트랙 하나, 클립 둘(0..960, 1920..2880), 첫 클립에 노트 하나.
    fn fixture() -> (Project, ClipId, ClipId) {
        let mut p = Project::new();
        p.push_track(Track::new(
            "Lead",
            Channel::new(1).unwrap(),
            TrackColor::new(0).unwrap(),
        ));
        let t = p.track_mut(0).unwrap();
        let a = t.insert_clip(Clip::new(Tick::ZERO, 960).unwrap()).unwrap();
        let b = t
            .insert_clip(Clip::new(Tick::new(1920), 960).unwrap())
            .unwrap();
        t.insert_note(a, note(0)).unwrap();
        (p, a, b)
    }

    /// apply → 역편집 apply → 원래 상태. 그리고 역의 역을 다시 적용하면 apply 직후 상태.
    fn round_trip(p: &mut Project, edit: Edit) {
        let before = p.clone();
        let inverse = edit.apply(p).expect("편집 적용");
        let after = p.clone();
        let again = inverse.apply(p).expect("역편집 적용");
        assert_eq!(*p, before, "역편집이 원래 상태로 돌리지 못했다");
        again.apply(p).expect("역의 역 적용");
        assert_eq!(*p, after, "역의 역이 적용 직후 상태와 다르다");
    }

    #[test]
    fn edit_apply_returns_inverse_for_each_variant() {
        let (mut p, a, b) = fixture();
        let extra = Track::new(
            "Bass",
            Channel::new(2).unwrap(),
            TrackColor::new(1).unwrap(),
        );
        let edits = vec![
            Edit::InsertTrack {
                index: 1,
                track: Box::new(extra),
            },
            Edit::SetTrackProp {
                track: 0,
                prop: TrackProp::Name("Lead 2".into()),
            },
            Edit::SetTrackProp {
                track: 0,
                prop: TrackProp::Channel(Channel::new(3).unwrap()),
            },
            Edit::SetTrackProp {
                track: 0,
                prop: TrackProp::Mute(true),
            },
            Edit::SetTrackProp {
                track: 0,
                prop: TrackProp::LyricLang(Some("ko".into())),
            },
            Edit::InsertClip {
                track: 0,
                clip: Box::new(Clip::new(Tick::new(4000), 10).unwrap()),
                id: None,
            },
            Edit::MoveClip {
                track: 0,
                id: b,
                to: Tick::new(5000),
            },
            Edit::SplitClip {
                track: 0,
                id: a,
                at: Tick::new(480),
                right_id: None,
            },
            Edit::SetClipName {
                track: 0,
                id: a,
                name: Some("후렴".into()),
            },
            Edit::SetClipColor {
                track: 0,
                id: a,
                color: Some(TrackColor::new(5).unwrap()),
            },
            Edit::InsertNote {
                track: 0,
                clip: a,
                note: note(100),
            },
            Edit::RemoveNote {
                track: 0,
                clip: a,
                note: note(0),
            },
            Edit::RemoveClip { track: 0, id: b },
            Edit::SetTempo {
                at: Tick::new(960),
                tempo: Some(Tempo::from_bpm(90).unwrap()),
            },
            Edit::SetTempo {
                at: Tick::ZERO,
                tempo: Some(Tempo::from_bpm(100).unwrap()),
            },
            Edit::SetMeter {
                at: Tick::new(3840),
                signature: Some(TimeSignature::new(3, 4).unwrap()),
            },
            Edit::SetKey(Key::Minor(crate::PitchClass::new(9).unwrap())),
        ];
        for edit in edits {
            round_trip(&mut p, edit);
        }
        // 트랙 삭제는 트랙이 둘 이상일 때 시험한다.
        p.push_track(Track::new(
            "Pad",
            Channel::new(4).unwrap(),
            TrackColor::new(2).unwrap(),
        ));
        round_trip(&mut p, Edit::RemoveTrack { index: 0 });
    }

    #[test]
    fn edit_apply_failure_leaves_project_unchanged() {
        let (mut p, a, b) = fixture();
        let before = p.clone();
        let failing = vec![
            Edit::RemoveTrack { index: 5 },
            Edit::InsertTrack {
                index: 5,
                track: Box::new(p.tracks()[0].clone()),
            },
            Edit::SetTrackProp {
                track: 9,
                prop: TrackProp::Solo(true),
            },
            Edit::MoveClip {
                track: 0,
                id: a,
                to: Tick::new(1900),
            },
            Edit::InsertClip {
                track: 0,
                clip: Box::new(Clip::new(Tick::new(100), 10).unwrap()),
                id: None,
            },
            Edit::InsertClip {
                track: 0,
                clip: Box::new(Clip::new(Tick::new(9000), 10).unwrap()),
                id: Some(b),
            },
            Edit::SplitClip {
                track: 0,
                id: a,
                at: Tick::new(960),
                right_id: None,
            },
            Edit::RemoveNote {
                track: 0,
                clip: a,
                note: note(5),
            },
            Edit::InsertNote {
                track: 0,
                clip: a,
                note: note(955),
            },
            Edit::SetTempo {
                at: Tick::ZERO,
                tempo: None,
            },
            Edit::SetMeter {
                at: Tick::ZERO,
                signature: None,
            },
            Edit::SetTempo {
                at: Tick::new(7),
                tempo: None,
            },
        ];
        for edit in failing {
            let shown = format!("{edit:?}");
            assert!(edit.apply(&mut p).is_err(), "실패해야 한다: {shown}");
            assert_eq!(p, before, "실패한 편집이 상태를 바꿨다: {shown}");
        }
    }

    #[test]
    fn edit_split_inverse_keeps_right_id_for_redo() {
        let (mut p, a, _) = fixture();
        let inverse = Edit::SplitClip {
            track: 0,
            id: a,
            at: Tick::new(480),
            right_id: None,
        }
        .apply(&mut p)
        .unwrap();
        let Edit::UnsplitClip { right, .. } = inverse.clone() else {
            panic!("{inverse:?}")
        };
        let redo = inverse.apply(&mut p).unwrap();
        assert!(matches!(redo, Edit::SplitClip { right_id: Some(r), .. } if r == right));
        redo.apply(&mut p).unwrap();
        assert!(
            p.tracks()[0].clip(right).is_some(),
            "redo가 같은 오른쪽 id를 썼다"
        );
    }

    #[test]
    fn edit_approx_bytes_grows_with_notes() {
        let mut clip = Clip::new(Tick::ZERO, 100_000).unwrap();
        let small = Edit::InsertClip {
            track: 0,
            clip: Box::new(clip.clone()),
            id: None,
        }
        .approx_bytes();
        for i in 0..1000 {
            clip.insert_note(note(i * 10)).unwrap();
        }
        let big = Edit::InsertClip {
            track: 0,
            clip: Box::new(clip),
            id: None,
        }
        .approx_bytes();
        assert!(
            big >= small + 1000 * std::mem::size_of::<Note>(),
            "{small} {big}"
        );
    }
}
