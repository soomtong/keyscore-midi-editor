//! 모델 불변식의 속성 기반 회귀 테스트(S0-T2 게이트).
//!
//! 공개 API만 쓴다. 공개 API로 불변식을 깨뜨릴 수 없다는 것을 crate 바깥에서 보이기 위해서다.
//! 이 파일은 이후 모든 단계의 회귀 테스트로 남는다(plan/stages/10-S0-foundation.md S0-T2).

use keyscore_core::{
    Channel, Clip, ClipId, ModelError, Note, Pitch, Tick, Track, TrackColor, Velocity,
};
use proptest::prelude::*;

/// 테스트가 다루는 시간 범위. 작게 잡아야 겹침·맞닿음·경계 사례가 자주 나온다.
const SPAN: u64 = 20_000;

#[derive(Debug, Clone)]
enum Op {
    InsertClip {
        start: u64,
        length: u64,
    },
    RemoveClip {
        pick: usize,
    },
    MoveClip {
        pick: usize,
        new_start: u64,
    },
    SplitClip {
        pick: usize,
        at: u64,
    },
    InsertNote {
        pick: usize,
        start: u64,
        length: u32,
        pitch: u8,
        lyric: bool,
    },
    RemoveNote {
        pick: usize,
        note_pick: usize,
    },
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0..SPAN, 0..4_000u64).prop_map(|(start, length)| Op::InsertClip { start, length }),
        1 => any::<usize>().prop_map(|pick| Op::RemoveClip { pick }),
        2 => (any::<usize>(), 0..SPAN).prop_map(|(pick, new_start)| Op::MoveClip { pick, new_start }),
        2 => (any::<usize>(), 0..SPAN + 4_000).prop_map(|(pick, at)| Op::SplitClip { pick, at }),
        4 => (any::<usize>(), 0..4_000u64, 0..2_000u32, 0..=127u8, any::<bool>())
            .prop_map(|(pick, start, length, pitch, lyric)| Op::InsertNote { pick, start, length, pitch, lyric }),
        1 => (any::<usize>(), any::<usize>()).prop_map(|(pick, note_pick)| Op::RemoveNote { pick, note_pick }),
    ]
}

fn new_track() -> Track {
    Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    )
}

fn pick_clip(track: &Track, pick: usize) -> Option<ClipId> {
    let ids: Vec<ClipId> = track.clips().map(|(id, _)| id).collect();
    if ids.is_empty() {
        None
    } else {
        Some(ids[pick % ids.len()])
    }
}

/// 연산을 적용한다. 생성자 단계에서 거부된 입력(길이 0 등)은 `Err`로 본다.
fn apply(track: &mut Track, op: &Op) -> Result<(), ModelError> {
    match *op {
        Op::InsertClip { start, length } => track
            .insert_clip(Clip::new(Tick::new(start), length)?)
            .map(|_| ()),
        Op::RemoveClip { pick } => {
            if let Some(id) = pick_clip(track, pick) {
                track.remove_clip(id);
            }
            Ok(())
        }
        Op::MoveClip { pick, new_start } => match pick_clip(track, pick) {
            Some(id) => track.move_clip(id, Tick::new(new_start)),
            None => Ok(()),
        },
        Op::SplitClip { pick, at } => match pick_clip(track, pick) {
            Some(id) => track.split_clip(id, Tick::new(at)).map(|_| ()),
            None => Ok(()),
        },
        Op::InsertNote {
            pick,
            start,
            length,
            pitch,
            lyric,
        } => match pick_clip(track, pick) {
            Some(id) => {
                let mut note = Note::new(
                    Tick::new(start),
                    length,
                    Pitch::new(pitch).unwrap(),
                    Velocity::from_midi1(100).unwrap(),
                )?;
                if lyric {
                    note = note.with_lyric("라");
                }
                track.insert_note(id, note)
            }
            None => Ok(()),
        },
        Op::RemoveNote { pick, note_pick } => {
            if let Some(id) = pick_clip(track, pick) {
                let notes = track.clip(id).unwrap().notes().to_vec();
                if !notes.is_empty() {
                    assert!(
                        track
                            .remove_note(id, &notes[note_pick % notes.len()])
                            .unwrap()
                    );
                }
            }
            Ok(())
        }
    }
}

/// 공개 API로 관찰한 불변식. 깨지면 이유를 돌려준다.
fn check_invariants(track: &Track) -> Result<(), String> {
    let clips: Vec<&Clip> = track.clips().map(|(_, c)| c).collect();
    for clip in &clips {
        if clip.length() == 0 {
            return Err(format!("길이 0 클립: {clip:?}"));
        }
        if clip.start().get().checked_add(clip.length()).is_none() {
            return Err(format!("끝 오버플로: {clip:?}"));
        }
        for note in clip.notes() {
            if note.start().get() >= clip.length() {
                return Err(format!("노트 시작이 클립 밖: {note:?} in {clip:?}"));
            }
            if note.length() == 0 {
                return Err(format!("길이 0 노트: {note:?}"));
            }
        }
        let keys: Vec<(u64, u8)> = clip
            .notes()
            .iter()
            .map(|n| (n.start().get(), n.pitch().get()))
            .collect();
        if !keys.windows(2).all(|w| w[0] <= w[1]) {
            return Err(format!("노트 정렬 깨짐: {keys:?}"));
        }
    }
    for pair in clips.windows(2) {
        if pair[0].end() > pair[1].start().get() {
            return Err(format!(
                "클립 겹침 또는 정렬 깨짐: {:?} / {:?}",
                pair[0], pair[1]
            ));
        }
    }
    let ids: Vec<ClipId> = track.clips().map(|(id, _)| id).collect();
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    if unique.len() != ids.len() {
        return Err(format!("클립 식별자 중복: {ids:?}"));
    }
    Ok(())
}

/// 트랙의 모든 노트를 (절대 시작, 길이, 음높이, 벨로시티, 가사)로 펼쳐 정렬한 것.
fn absolute_notes(track: &Track) -> Vec<(u64, u32, u8, u16, Option<String>)> {
    let mut out: Vec<_> = track
        .clips()
        .flat_map(|(_, clip)| {
            clip.notes().iter().map(move |n| {
                (
                    clip.start().get() + n.start().get(),
                    n.length(),
                    n.pitch().get(),
                    n.velocity().get(),
                    n.lyric().map(str::to_owned),
                )
            })
        })
        .collect();
    out.sort();
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn track_ops_random_sequence_preserve_invariants(ops in prop::collection::vec(op(), 0..80)) {
        let mut track = new_track();
        for op in &ops {
            let _ = apply(&mut track, op);
            if let Err(why) = check_invariants(&track) {
                prop_assert!(false, "{op:?} 뒤 불변식 깨짐: {why}");
            }
        }
    }

    #[test]
    fn track_ops_failed_op_leaves_track_unchanged(ops in prop::collection::vec(op(), 0..80)) {
        let mut track = new_track();
        for op in &ops {
            let before = track.clone();
            if apply(&mut track, op).is_err() {
                prop_assert_eq!(&track, &before, "실패한 {:?}가 상태를 바꿨다", op);
            }
        }
    }

    #[test]
    fn track_split_clip_preserves_absolute_notes(
        ops in prop::collection::vec(op(), 0..60),
        pick in any::<usize>(),
        at_frac in 0.0f64..1.0,
    ) {
        let mut track = new_track();
        for op in &ops {
            let _ = apply(&mut track, op);
        }
        let Some(id) = pick_clip(&track, pick) else { return Ok(()) };
        let clip = track.clip(id).unwrap();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
        let at = clip.start().get() + 1 + ((clip.length().saturating_sub(1)) as f64 * at_frac) as u64;
        if at >= clip.end() {
            return Ok(());
        }
        let before = absolute_notes(&track);
        let clip_count = track.clips().count();
        track.split_clip(id, Tick::new(at)).unwrap();
        prop_assert_eq!(absolute_notes(&track), before);
        prop_assert_eq!(track.clips().count(), clip_count + 1);
    }

    #[test]
    fn track_move_clip_moves_notes_with_it(
        ops in prop::collection::vec(op(), 0..60),
        pick in any::<usize>(),
        new_start in 0..SPAN,
    ) {
        let mut track = new_track();
        for op in &ops {
            let _ = apply(&mut track, op);
        }
        let Some(id) = pick_clip(&track, pick) else { return Ok(()) };
        let relative = track.clip(id).unwrap().notes().to_vec();
        if track.move_clip(id, Tick::new(new_start)).is_ok() {
            let clip = track.clip(id).unwrap();
            prop_assert_eq!(clip.start().get(), new_start);
            prop_assert_eq!(clip.notes(), relative.as_slice());
        }
    }
}
