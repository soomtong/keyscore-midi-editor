//! undo 엔진의 속성 기반 테스트(S0-T4 게이트). 공개 API(`Document`)만 쓴다.

use keyscore_core::{
    Channel, Clip, ClipId, CoalesceKey, Document, Edit, Key, ModelError, Note, Pitch, PitchClass,
    Project, Tempo, Tick, TimeSignature, Track, TrackColor, TrackId, TrackProp, Velocity,
};
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    AddTrack {
        at: usize,
        channel: u8,
    },
    RemoveTrack {
        pick: usize,
    },
    Prop {
        pick: usize,
        mute: bool,
        name: u8,
    },
    InsertClip {
        pick: usize,
        start: u64,
        length: u64,
    },
    RemoveClip {
        pick: usize,
        clip: usize,
    },
    MoveClip {
        pick: usize,
        clip: usize,
        to: u64,
    },
    SplitClip {
        pick: usize,
        clip: usize,
        offset: u64,
    },
    ClipName {
        pick: usize,
        clip: usize,
        name: Option<u8>,
    },
    InsertNote {
        pick: usize,
        clip: usize,
        start: u64,
        length: u32,
        pitch: u8,
    },
    RemoveNote {
        pick: usize,
        clip: usize,
        note: usize,
    },
    Tempo {
        at: u64,
        bpm: Option<u32>,
    },
    Meter {
        at: u64,
        num: Option<u8>,
    },
    Key(Option<u8>),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        1 => (0..4usize, 1..=16u8).prop_map(|(at, channel)| Op::AddTrack { at, channel }),
        1 => any::<usize>().prop_map(|pick| Op::RemoveTrack { pick }),
        1 => (any::<usize>(), any::<bool>(), any::<u8>()).prop_map(|(pick, mute, name)| Op::Prop { pick, mute, name }),
        3 => (any::<usize>(), 0..20_000u64, 0..3_000u64).prop_map(|(pick, start, length)| Op::InsertClip { pick, start, length }),
        1 => (any::<usize>(), any::<usize>()).prop_map(|(pick, clip)| Op::RemoveClip { pick, clip }),
        2 => (any::<usize>(), any::<usize>(), 0..20_000u64).prop_map(|(pick, clip, to)| Op::MoveClip { pick, clip, to }),
        2 => (any::<usize>(), any::<usize>(), 0..3_000u64).prop_map(|(pick, clip, offset)| Op::SplitClip { pick, clip, offset }),
        1 => (any::<usize>(), any::<usize>(), any::<Option<u8>>()).prop_map(|(pick, clip, name)| Op::ClipName { pick, clip, name }),
        4 => (any::<usize>(), any::<usize>(), 0..3_000u64, 1..1_000u32, 0..=127u8)
            .prop_map(|(pick, clip, start, length, pitch)| Op::InsertNote { pick, clip, start, length, pitch }),
        1 => (any::<usize>(), any::<usize>(), any::<usize>()).prop_map(|(pick, clip, note)| Op::RemoveNote { pick, clip, note }),
        1 => (0..4u64, any::<Option<u32>>()).prop_map(|(at, bpm)| Op::Tempo { at: at * 960, bpm: bpm.map(|b| 40 + b % 200) }),
        1 => (0..4u64, any::<Option<u8>>()).prop_map(|(at, num)| Op::Meter { at: at * 3840, num: num.map(|n| 1 + n % 12) }),
        1 => any::<Option<u8>>().prop_map(Op::Key),
    ]
}

fn initial() -> Project {
    let mut p = Project::new();
    p.push_track(Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    ))
    .unwrap();
    p
}

fn pick_track(p: &Project, pick: usize) -> Option<TrackId> {
    (!p.tracks().is_empty()).then(|| p.tracks()[pick % p.tracks().len()].id())
}

fn pick_clip(p: &Project, track: TrackId, pick: usize) -> Option<ClipId> {
    let ids: Vec<ClipId> = p.track(track)?.clips().map(|(id, _)| id).collect();
    (!ids.is_empty()).then(|| ids[pick % ids.len()])
}

/// 현재 상태에서 연산을 편집 묶음으로 바꾼다. 대상이 없으면 `None`.
/// 생성자에서 거부되는 값(길이 0 등)도 `None`으로 둔다(편집 이전 단계의 실패라서).
#[allow(clippy::too_many_lines, reason = "연산 변형마다 한 갈래인 변환 표")]
fn to_edits(p: &Project, op: &Op) -> Option<Vec<Edit>> {
    let edit = match *op {
        Op::AddTrack { at, channel } => Edit::InsertTrack {
            index: at,
            track: Box::new(Track::new(
                "T",
                Channel::new(channel).unwrap(),
                TrackColor::new(1).unwrap(),
            )),
        },
        // 트랙을 하나는 남겨야 나머지 연산이 대상을 찾는다(대상 없음이 많으면 시험이 약해진다).
        Op::RemoveTrack { pick } if p.tracks().len() > 1 => Edit::RemoveTrack {
            track: pick_track(p, pick)?,
        },
        Op::RemoveTrack { .. } => return None,
        Op::Prop { pick, mute, name } => {
            let track = pick_track(p, pick)?;
            let prop = if name % 2 == 0 {
                TrackProp::Mute(mute)
            } else {
                TrackProp::Name(format!("n{name}"))
            };
            Edit::SetTrackProp { track, prop }
        }
        Op::InsertClip {
            pick,
            start,
            length,
        } => Edit::InsertClip {
            track: pick_track(p, pick)?,
            clip: Box::new(Clip::new(Tick::new(start), length).ok()?),
        },
        Op::RemoveClip { pick, clip } => {
            let track = pick_track(p, pick)?;
            Edit::RemoveClip {
                track,
                id: pick_clip(p, track, clip)?,
            }
        }
        Op::MoveClip { pick, clip, to } => {
            let track = pick_track(p, pick)?;
            Edit::MoveClip {
                track,
                id: pick_clip(p, track, clip)?,
                to: Tick::new(to),
            }
        }
        Op::SplitClip { pick, clip, offset } => {
            let track = pick_track(p, pick)?;
            let id = pick_clip(p, track, clip)?;
            let start = p.track(track)?.clip(id)?.start().get();
            Edit::SplitClip {
                track,
                id,
                at: Tick::new(start + offset),
            }
        }
        Op::ClipName { pick, clip, name } => {
            let track = pick_track(p, pick)?;
            Edit::SetClipName {
                track,
                id: pick_clip(p, track, clip)?,
                name: name.map(|n| format!("c{n}")),
            }
        }
        Op::InsertNote {
            pick,
            clip,
            start,
            length,
            pitch,
        } => {
            let track = pick_track(p, pick)?;
            let id = pick_clip(p, track, clip)?;
            // 시작은 클립 안으로 맞춘다. 길이 때문에 끝이 넘치는 실패 사례는 그대로 남는다.
            let start = start % p.track(track)?.clip(id)?.length();
            let note = Note::new(
                Tick::new(start),
                length,
                Pitch::new(pitch).unwrap(),
                Velocity::new(5000),
            )
            .ok()?;
            Edit::InsertNote {
                track,
                clip: id,
                note,
            }
        }
        Op::RemoveNote { pick, clip, note } => {
            let track = pick_track(p, pick)?;
            let id = pick_clip(p, track, clip)?;
            let notes = p.track(track)?.clip(id)?.notes();
            let note = notes.get(note % notes.len().max(1))?.clone();
            Edit::RemoveNote {
                track,
                clip: id,
                note,
            }
        }
        Op::Tempo { at, bpm } => Edit::SetTempo {
            at: Tick::new(at),
            tempo: bpm.map(|b| Tempo::from_bpm(b).unwrap()),
        },
        Op::Meter { at, num } => Edit::SetMeter {
            at: Tick::new(at),
            signature: num.map(|n| TimeSignature::new(n, 4).unwrap()),
        },
        Op::Key(k) => Edit::SetKey(match k {
            None => Key::None,
            Some(k) if k % 2 == 0 => Key::Major(PitchClass::new(k % 12).unwrap()),
            Some(k) => Key::Minor(PitchClass::new(k % 12).unwrap()),
        }),
    };
    Some(vec![edit])
}

/// S0-T2 불변식을 공개 API로 검사한다(`tests/model_invariants.rs`와 같은 기준).
fn check_invariants(p: &Project) -> Result<(), String> {
    for track in p.tracks() {
        let clips: Vec<&Clip> = track.clips().map(|(_, c)| c).collect();
        for pair in clips.windows(2) {
            if pair[0].end() > pair[1].start().get() {
                return Err(format!("클립 겹침: {:?} / {:?}", pair[0], pair[1]));
            }
        }
        for clip in clips {
            if clip
                .notes()
                .iter()
                .any(|n| n.start().get() >= clip.length())
            {
                return Err(format!("노트가 클립 밖: {clip:?}"));
            }
        }
        let mut ids: Vec<ClipId> = track.clips().map(|(id, _)| id).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        if ids.len() != n {
            return Err("한 트랙 안에 같은 ClipId".into());
        }
    }
    if p.tempo_map().changes().next().map(|(t, _)| t) != Some(Tick::ZERO) {
        return Err("템포 맵에 틱 0 항목이 없다".into());
    }
    if p.meter_map().changes().next().map(|(t, _)| t) != Some(Tick::ZERO) {
        return Err("박자표 맵에 틱 0 항목이 없다".into());
    }
    Ok(())
}

/// 연산 열을 히스토리로 적용하고 성공한 단위 수를 돌려준다.
fn run(d: &mut Document, ops: &[Op]) -> usize {
    let mut applied = 0;
    for op in ops {
        if let Some(edits) = to_edits(d.project(), op)
            && d.apply(edits, None).is_ok()
        {
            applied += 1;
        }
    }
    applied
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn history_random_edits_undo_all_restores_initial(ops in prop::collection::vec(op(), 0..60)) {
        let mut d = Document::new(initial(), usize::MAX).unwrap();
        let start = d.project().clone();
        let n = run(&mut d, &ops);
        prop_assert_eq!(d.undo_len(), n);
        for _ in 0..n {
            prop_assert!(d.undo().unwrap());
            if let Err(why) = check_invariants(d.project()) {
                prop_assert!(false, "undo 뒤 불변식 깨짐: {}", why);
            }
        }
        prop_assert!(!d.undo().unwrap());
        prop_assert_eq!(d.project(), &start);
    }

    #[test]
    fn history_undo_then_redo_all_restores_final(ops in prop::collection::vec(op(), 0..60)) {
        let mut d = Document::new(initial(), usize::MAX).unwrap();
        let n = run(&mut d, &ops);
        let end = d.project().clone();
        for _ in 0..n {
            d.undo().unwrap();
        }
        for _ in 0..n {
            prop_assert!(d.redo().unwrap());
            if let Err(why) = check_invariants(d.project()) {
                prop_assert!(false, "redo 뒤 불변식 깨짐: {}", why);
            }
        }
        prop_assert!(!d.redo().unwrap());
        // ClipId까지 같아야 한다(S0-T4 이월 조건).
        prop_assert_eq!(d.project(), &end);
    }

    #[test]
    fn history_failed_transaction_leaves_project_unchanged(
        setup in prop::collection::vec(op(), 0..30),
        batch in prop::collection::vec(op(), 1..6),
    ) {
        let mut d = Document::new(initial(), usize::MAX).unwrap();
        run(&mut d, &setup);
        // 묶음은 모두 지금 상태 기준으로 만든다. 앞 편집이 상태를 바꾸면 뒤 편집이 실패할 수 있다.
        let edits: Vec<Edit> = batch.iter().filter_map(|op| to_edits(d.project(), op)).flatten().collect();
        if edits.is_empty() {
            // 빈 묶음은 undo 단위를 만들지 않는다(history_empty_apply_adds_nothing).
            return Ok(());
        }
        let (before, undo_len) = (d.project().clone(), d.undo_len());
        if d.apply(edits, Some(CoalesceKey(7))).is_ok() {
            prop_assert!(d.undo().unwrap());
            prop_assert_eq!(d.project(), &before);
        } else {
            prop_assert_eq!(d.project(), &before);
            prop_assert_eq!(d.undo_len(), undo_len);
        }
    }

    #[test]
    fn history_mixed_undo_redo_and_new_edits_stay_consistent(
        steps in prop::collection::vec((op(), 0..4u8), 0..80),
    ) {
        // 새 편집, undo, redo를 섞는다. 같은 일을 하는 기준 모델(스냅샷 스택)과 매 단계 비교한다.
        let mut d = Document::new(initial(), usize::MAX).unwrap();
        let start = d.project().clone();
        let mut past: Vec<Project> = Vec::new();
        let mut future: Vec<Project> = Vec::new();
        for (op, action) in &steps {
            match action {
                0 => {
                    let current = d.project().clone();
                    if d.undo().unwrap() {
                        let previous = past.pop().unwrap();
                        prop_assert_eq!(d.project(), &previous);
                        future.push(current);
                    }
                }
                1 => {
                    let current = d.project().clone();
                    if d.redo().unwrap() {
                        let next = future.pop().unwrap();
                        prop_assert_eq!(d.project(), &next);
                        past.push(current);
                    }
                }
                _ => {
                    if let Some(edits) = to_edits(d.project(), op) {
                        let snapshot = d.project().clone();
                        if d.apply(edits, None).is_ok() {
                            past.push(snapshot);
                            future.clear();
                        }
                    }
                }
            }
            prop_assert_eq!(d.undo_len(), past.len());
            prop_assert_eq!(d.redo_len(), future.len());
            if let Err(why) = check_invariants(d.project()) {
                prop_assert!(false, "불변식 깨짐: {}", why);
            }
        }
        // 끝까지 undo하면 처음 상태.
        while d.undo().unwrap() {}
        // initial()은 부를 때마다 새 TrackId를 만드므로 처음 상태의 복제와 비교한다.
        prop_assert_eq!(d.project(), &start);
    }
}

#[test]
fn history_errors_are_model_errors() {
    // 없는 트랙을 가리키는 편집은 모델 오류로 거부된다.
    let mut d = Document::new(initial(), usize::MAX).unwrap();
    let stranger = Track::new("X", Channel::new(2).unwrap(), TrackColor::new(0).unwrap()).id();
    let err = d
        .apply(vec![Edit::RemoveTrack { track: stranger }], None)
        .unwrap_err();
    assert!(matches!(err, ModelError::UnknownTrack), "{err:?}");
}
