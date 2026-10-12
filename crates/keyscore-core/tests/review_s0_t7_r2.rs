//! S0-T7 적대적 리뷰 2회차의 깨뜨리기 테스트. 공개 API만 쓴다.

use std::panic::{AssertUnwindSafe, catch_unwind};

use keyscore_core::transform::{self, Grid, NoteRef, PitchRange, QuantizeSettings};
use keyscore_core::{
    Channel, Clip, ClipId, Document, Edit, ModelError, Note, Pitch, Project, Tick, TimeSignature,
    Track, TrackColor, TrackId, Velocity,
};

fn note(start: u64, length: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        length,
        Pitch::new(pitch).unwrap(),
        Velocity::new(1),
    )
    .unwrap()
}

fn new_track() -> Track {
    Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap())
}

fn notes_of(p: &Project, t: TrackId, c: ClipId) -> Vec<Note> {
    p.track(t).unwrap().clip(c).unwrap().notes().to_vec()
}

fn clip_len(p: &Project, t: TrackId, c: ClipId) -> u64 {
    p.track(t).unwrap().clip(c).unwrap().length()
}

fn refs(t: TrackId, c: ClipId, notes: &[Note]) -> Vec<NoteRef> {
    notes
        .iter()
        .map(|n| NoteRef {
            track: t,
            clip: c,
            note: n.clone(),
        })
        .collect()
}

/// 클립 [start, start + 7680)에 노트를 넣고 3840에서 분할한다. `keep_right`가 거짓이면 오른쪽 클립을
/// 지워 왼쪽 클립이 늘어날 자리를 만든다. (문서, 트랙, 왼쪽, 오른쪽)
fn split_doc(
    start: u64,
    notes: &[Note],
    keep_right: bool,
    meter: Option<(u8, u16)>,
) -> (Document, TrackId, ClipId, ClipId) {
    let mut p = Project::new();
    if let Some((n, d)) = meter {
        p.meter_map_mut()
            .set(Tick::ZERO, TimeSignature::new(n, d).unwrap());
    }
    let mut t = new_track();
    let tid = t.id();
    let mut clip = Clip::new(Tick::new(start), 7680).unwrap();
    for n in notes {
        clip.insert_note(n.clone()).unwrap();
    }
    let left = t.insert_clip(clip).unwrap();
    let right = t.split_clip(left, Tick::new(start + 3840)).unwrap();
    if !keep_right {
        t.remove_clip(right).unwrap();
    }
    p.push_track(t).unwrap();
    (Document::new(p, usize::MAX).unwrap(), tid, left, right)
}

// ---------------------------------------------------------------------------
// 꼬리 규칙과 클립 늘림
// ---------------------------------------------------------------------------

/// 꼬리 노트의 시작이 클립 밖으로 멀리 나가면, 늘린 클립 길이(시작을 덮는 마디)가 원래 꼬리 끝보다
/// 길어져 그 노트는 「원래 꼬리」가 아니게 된다. 그런데 새 끝은 늘린 길이를 넘는다. `plan`은 꼬리로
/// 두라고 계산했지만, 적용 시점의 `ReplaceNote`는 「지금 꼬리인가」를 보므로 거부한다.
#[test]
fn review_r2_shift_tail_start_far_out_of_clip_applies() {
    // 꼬리: 1920 + 3840 = 5760 > 3840.
    let tail = note(1920, 3840, 64);
    let (mut d, t, left, _) = split_doc(0, std::slice::from_ref(&tail), false, None);
    let edits = transform::shift(d.project(), &refs(t, left, &[tail]), 4000)
        .expect("결정 개정: 시작이 밖으로 나가면 시작을 덮도록 늘린다");
    let r = d.apply(edits, None);
    assert!(r.is_ok(), "연산이 Ok로 낸 묶음이 적용에서 실패한다: {r:?}");
    assert_eq!(clip_len(d.project(), t, left), 7680);
    assert_eq!(notes_of(d.project(), t, left)[0].start().get(), 5920);
}

/// 같은 구멍을 시작이 클립 안에 남는 경우로: 다른 노트가 클립을 늘려 꼬리가 「드러난」 뒤, 옮긴 꼬리의
/// 끝이 늘린 길이를 넘는다.
#[test]
fn review_r2_tail_revealed_by_neighbor_extension_then_overhangs() {
    // 꼬리 끝 1920 + 5000 = 6920 > 3840, 일반 노트 3700..3800.
    let tail = note(1920, 5000, 64);
    let plain = note(3700, 100, 60);
    let (mut d, t, left, _) = split_doc(0, &[tail.clone(), plain.clone()], false, None);
    let edits = transform::shift(d.project(), &refs(t, left, &[tail, plain]), 900)
        .expect("일반 노트가 7680까지 늘리고, 꼬리는 시작만 클립 안이면 된다");
    let before = d.project().clone();
    let r = d.apply(edits, None);
    if r.is_err() {
        assert_eq!(d.project(), &before, "실패해도 원자적이어야 한다");
    }
    assert!(r.is_ok(), "연산이 Ok로 낸 묶음이 적용에서 실패한다: {r:?}");
}

/// 꼬리 노트의 시작이 정확히 클립 길이에 닿으면 그 시작을 덮는 다음 마디 경계(3/4에서 2880 배수)로
/// 늘어난다.
#[test]
fn review_r2_tail_start_at_clip_end_extends_to_bar_boundary_in_three_four() {
    // 3/4: 마디 2880. 클립 0..7680을 3840에서 나누므로 왼쪽 길이 3840(마디 경계 아님).
    let tail = note(3000, 2000, 64);
    let (mut d, t, left, _) = split_doc(0, std::slice::from_ref(&tail), false, Some((3, 4)));
    let edits = transform::shift(d.project(), &refs(t, left, &[tail]), 840).unwrap();
    d.apply(edits, None).unwrap();
    // 시작 3840 → 3840을 덮는 마디 끝은 5760.
    assert_eq!(clip_len(d.project(), t, left), 5760);
    let n = &notes_of(d.project(), t, left)[0];
    assert_eq!((n.start().get(), n.end()), (3840, 5840));
    assert!(d.undo().unwrap());
    assert_eq!(clip_len(d.project(), t, left), 3840);
}

/// 꼬리 노트와 일반 노트가 섞인 선택: 오른쪽 이웃이 맞닿아 있으면 꼬리의 조옮김·작은 이동은 되고,
/// 일반 노트가 끝을 넘는 이동은 전체 거부된다.
#[test]
fn review_r2_mixed_tail_and_plain_selection_with_neighbor() {
    let tail = note(1920, 3840, 64);
    let plain = note(3600, 200, 60);
    let (mut d, t, left, _) = split_doc(0, &[tail.clone(), plain.clone()], true, None);
    let sel = refs(t, left, &[tail.clone(), plain.clone()]);
    let edits = transform::transpose(d.project(), &sel, -7, PitchRange::full()).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(clip_len(d.project(), t, left), 3840);
    assert!(d.undo().unwrap());
    // 일반 노트 3600+200 → 3640..3840: 아직 안.
    let edits = transform::shift(d.project(), &sel, 40).unwrap();
    d.apply(edits, None).unwrap();
    assert!(d.undo().unwrap());
    // 41이면 일반 노트가 3841까지 → 늘림 → 이웃과 겹침 → 전체 거부.
    assert!(matches!(
        transform::shift(d.project(), &sel, 41),
        Err(ModelError::ClipOverlap { .. })
    ));
}

/// 클립이 u64 끝 근처일 때 꼬리 노트를 극단값으로 옮겨도 패닉하지 않는다.
#[test]
fn review_r2_tail_near_u64_end_does_not_panic() {
    let start = u64::MAX - 8000;
    let tail = note(1920, 3840, 64);
    let (d, t, left, _) = split_doc(start, std::slice::from_ref(&tail), false, None);
    let sel = refs(t, left, &[tail]);
    for delta in [i64::MAX, i64::MIN, 1920, 5000, 6079, 6080] {
        let r = catch_unwind(AssertUnwindSafe(|| {
            transform::shift(d.project(), &sel, delta)
        }));
        assert!(r.is_ok(), "shift({delta}) 패닉");
    }
    for (n, k) in [(u32::MAX, 1), (2, 1), (1, u32::MAX)] {
        let r = catch_unwind(AssertUnwindSafe(|| {
            transform::scale(d.project(), &sel, n, k)
        }));
        assert!(r.is_ok(), "scale({n}/{k}) 패닉");
    }
}

// ---------------------------------------------------------------------------
// 공개 Edit::ReplaceNote
// ---------------------------------------------------------------------------

/// 꼬리가 아닌 노트를 꼬리로 바꿀 수 없다(분할 없이 꼬리 만들기).
#[test]
fn review_r2_replace_plain_into_tail_is_rejected() {
    let (mut d, t, c, _) = split_doc(0, &[note(100, 100, 60)], true, None);
    let before = d.project().clone();
    let r = d.apply(
        vec![Edit::ReplaceNote {
            track: t,
            clip: c,
            from: note(100, 100, 60),
            to: note(3000, 2000, 60),
        }],
        None,
    );
    assert!(matches!(r, Err(ModelError::NoteOutsideClip { .. })));
    assert_eq!(d.project(), &before);
    assert_eq!(d.undo_len(), 0);
}

/// 한 묶음 안에서 꼬리 → 일반 → 꼬리로 바꾸기. 두 번째 바꾸기의 `from`은 꼬리가 아니므로 거부되어야
/// 하고, 묶음 전체가 되돌아가야 한다.
#[test]
fn review_r2_replace_tail_to_plain_then_back_to_tail_in_one_batch_is_rejected() {
    let tail = note(1920, 3840, 64);
    let (mut d, t, c, _) = split_doc(0, std::slice::from_ref(&tail), true, None);
    let before = d.project().clone();
    let r = d.apply(
        vec![
            Edit::ReplaceNote {
                track: t,
                clip: c,
                from: tail,
                to: note(10, 10, 64),
            },
            Edit::ReplaceNote {
                track: t,
                clip: c,
                from: note(10, 10, 64),
                to: note(10, 9000, 64),
            },
        ],
        None,
    );
    assert!(matches!(r, Err(ModelError::NoteOutsideClip { .. })));
    assert_eq!(d.project(), &before);
}

/// 꼬리 노트를 다른 클립(오른쪽 클립)으로 옮기는 조합: `RemoveNote` + `InsertNote`는 꼬리를 받지 않고,
/// `ReplaceNote`는 클립 하나 안에서만 동작한다. 오른쪽 클립에 같은 값의 노트가 없으면 `NoteNotFound`.
#[test]
fn review_r2_tail_cannot_be_moved_to_other_clip() {
    let tail = note(1920, 3840, 64);
    let (mut d, t, left, right) = split_doc(0, std::slice::from_ref(&tail), true, None);
    let before = d.project().clone();
    let r = d.apply(
        vec![
            Edit::RemoveNote {
                track: t,
                clip: left,
                note: tail.clone(),
            },
            Edit::InsertNote {
                track: t,
                clip: right,
                note: note(1920, 3840, 64),
            },
        ],
        None,
    );
    assert!(r.is_err());
    assert_eq!(d.project(), &before);
    // 오른쪽 클립(길이 3840)에 일반 노트를 두고, 왼쪽의 꼬리 값을 from으로 오른쪽에 ReplaceNote.
    d.apply(
        vec![Edit::InsertNote {
            track: t,
            clip: right,
            note: note(0, 10, 60),
        }],
        None,
    )
    .unwrap();
    let before = d.project().clone();
    let r = d.apply(
        vec![Edit::ReplaceNote {
            track: t,
            clip: right,
            from: tail,
            to: note(0, 9000, 60),
        }],
        None,
    );
    assert_eq!(r, Err(ModelError::NoteNotFound));
    assert_eq!(d.project(), &before);
}

/// 꼬리를 꼬리로 바꾸며 클립 밖으로 훨씬 더 멀리 뻗는다(분할 때의 원래 끝 7680을 넘어). 결정 개정의
/// 「꼬리는 꼬리로 둔다」와 스케일 규칙(길이도 비율만큼)에서 따라 나오므로 허용으로 기록한다.
/// u64 끝을 넘는 꼬리는 거부되어야 한다.
#[test]
fn review_r2_replace_tail_to_longer_tail_is_allowed_but_bounded_by_u64() {
    let tail = note(1920, 3840, 64);
    let (mut d, t, c, _) = split_doc(0, std::slice::from_ref(&tail), true, None);
    let far = note(1920, u32::MAX, 64);
    d.apply(
        vec![Edit::ReplaceNote {
            track: t,
            clip: c,
            from: tail.clone(),
            to: far.clone(),
        }],
        None,
    )
    .unwrap();
    assert_eq!(notes_of(d.project(), t, c), vec![far]);
    assert!(d.undo().unwrap());
    assert_eq!(notes_of(d.project(), t, c), vec![tail.clone()]);
    assert!(d.redo().unwrap());
    assert!(d.undo().unwrap());

    let start = u64::MAX - 8000;
    let (mut d, t, c, _) = split_doc(start, std::slice::from_ref(&tail), true, None);
    let before = d.project().clone();
    let r = d.apply(
        vec![Edit::ReplaceNote {
            track: t,
            clip: c,
            from: tail,
            to: far_note(),
        }],
        None,
    );
    assert_eq!(r, Err(ModelError::TickOverflow));
    assert_eq!(d.project(), &before);
}

fn far_note() -> Note {
    note(1920, u32::MAX, 64)
}

/// `from`이 없는 경우, 다른 트랙의 클립 id, 시작이 클립 밖인 `to`.
#[test]
fn review_r2_replace_rejects_missing_from_foreign_clip_and_outside_start() {
    let (mut d, t, c, right) = split_doc(0, &[note(100, 100, 60)], true, None);
    let before = d.project().clone();
    let cases = [
        (t, c, note(200, 100, 60), note(300, 100, 60)),
        (t, right, note(100, 100, 60), note(300, 100, 60)),
        (t, c, note(100, 100, 60), note(3840, 1, 60)),
    ];
    for (track, clip, from, to) in cases {
        let r = d.apply(
            vec![Edit::ReplaceNote {
                track,
                clip,
                from,
                to,
            }],
            None,
        );
        assert!(r.is_err());
        assert_eq!(d.project(), &before);
    }
    // 다른 트랙 id.
    let other = new_track().id();
    let r = d.apply(
        vec![Edit::ReplaceNote {
            track: other,
            clip: c,
            from: note(100, 100, 60),
            to: note(300, 100, 60),
        }],
        None,
    );
    assert_eq!(r, Err(ModelError::UnknownTrack));
    assert_eq!(d.undo_len(), 0);
}

/// from == to(일반·꼬리 모두)는 아무것도 바꾸지 않고 undo/redo가 된다.
#[test]
fn review_r2_replace_with_identical_value_round_trips() {
    let tail = note(1920, 3840, 64);
    let plain = note(100, 100, 60);
    let (mut d, t, c, _) = split_doc(0, &[tail.clone(), plain.clone()], true, None);
    let before = d.project().clone();
    for n in [tail, plain] {
        d.apply(
            vec![Edit::ReplaceNote {
                track: t,
                clip: c,
                from: n.clone(),
                to: n,
            }],
            None,
        )
        .unwrap();
        assert_eq!(d.project(), &before);
    }
    assert!(d.undo().unwrap());
    assert!(d.undo().unwrap());
    assert_eq!(d.project(), &before);
    assert!(d.redo().unwrap());
    assert!(d.redo().unwrap());
    assert_eq!(d.project(), &before);
}

/// 꼬리를 이미 있는 일반 노트와 같은 값으로 바꾼 뒤 undo: 꼬리 하나와 일반 노트 하나로 정확히 돌아와야
/// 한다(같은 값 둘 중 무엇을 고르든 결과는 같아야 한다).
#[test]
fn review_r2_replace_tail_into_duplicate_of_existing_note_and_undo() {
    let tail = note(1920, 3840, 64);
    let plain = note(100, 100, 60);
    let (mut d, t, c, _) = split_doc(0, &[tail.clone(), plain.clone()], true, None);
    let before = d.project().clone();
    d.apply(
        vec![Edit::ReplaceNote {
            track: t,
            clip: c,
            from: tail,
            to: plain.clone(),
        }],
        None,
    )
    .unwrap();
    assert_eq!(notes_of(d.project(), t, c), vec![plain.clone(), plain]);
    assert!(d.undo().unwrap());
    assert_eq!(d.project(), &before);
    assert!(d.redo().unwrap());
    assert!(d.undo().unwrap());
    assert_eq!(d.project(), &before);
}

/// 같은 값 노트 둘 중 하나를 바꾸고, 같은 묶음에서 바뀐 값을 다시 바꾼다. undo 한 번에 원래대로.
#[test]
fn review_r2_replace_chain_among_identical_notes_undo() {
    let (mut d, t, c, _) = split_doc(0, &[note(10, 10, 60), note(10, 10, 60)], true, None);
    let before = d.project().clone();
    d.apply(
        vec![
            Edit::ReplaceNote {
                track: t,
                clip: c,
                from: note(10, 10, 60),
                to: note(20, 10, 60),
            },
            Edit::ReplaceNote {
                track: t,
                clip: c,
                from: note(10, 10, 60),
                to: note(20, 10, 60),
            },
            Edit::ReplaceNote {
                track: t,
                clip: c,
                from: note(20, 10, 60),
                to: note(30, 10, 60),
            },
        ],
        None,
    )
    .unwrap();
    assert_eq!(
        notes_of(d.project(), t, c),
        vec![note(20, 10, 60), note(30, 10, 60)]
    );
    assert!(d.undo().unwrap());
    assert_eq!(d.project(), &before);
}

// ---------------------------------------------------------------------------
// 중복 선택
// ---------------------------------------------------------------------------

/// 같은 값 노트가 둘이면 같은 참조를 두 번까지는 고를 수 있고(둘 다 움직임), 세 번이면 거부된다.
/// 다른 클립에 같은 값이 있어도 클립별로 센다.
#[test]
fn review_r2_duplicate_selection_counts_per_clip() {
    let n = note(10, 10, 60);
    let (mut d, t, left, right) = split_doc(0, &[n.clone(), n.clone()], true, None);
    d.apply(
        vec![Edit::InsertNote {
            track: t,
            clip: right,
            note: n.clone(),
        }],
        None,
    )
    .unwrap();
    let two = refs(t, left, &[n.clone(), n.clone()]);
    let edits = transform::shift(d.project(), &two, 100).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(
        notes_of(d.project(), t, left),
        vec![note(110, 10, 60), note(110, 10, 60)]
    );
    assert!(d.undo().unwrap());
    let three = refs(t, left, &[n.clone(), n.clone(), n.clone()]);
    assert_eq!(
        transform::shift(d.project(), &three, 100),
        Err(ModelError::NoteNotFound)
    );
    // 왼쪽 둘 + 오른쪽 하나(같은 값)는 클립별로 개수 안이다.
    let mut mixed = refs(t, left, &[n.clone(), n.clone()]);
    mixed.extend(refs(t, right, std::slice::from_ref(&n)));
    assert!(transform::shift(d.project(), &mixed, 100).is_ok());
    // 오른쪽 둘은 넘는다.
    let over = refs(t, right, &[n.clone(), n]);
    assert_eq!(
        transform::transpose(d.project(), &over, 1, PitchRange::full()),
        Err(ModelError::NoteNotFound)
    );
    let s = QuantizeSettings::new(Grid::Quarter, false, 100, 0).unwrap();
    assert_eq!(
        transform::quantize(d.project(), &over, &s),
        Err(ModelError::NoteNotFound)
    );
    assert_eq!(
        transform::scale(d.project(), &over, 2, 1),
        Err(ModelError::NoteNotFound)
    );
}

// ---------------------------------------------------------------------------
// 클립별 스케일 기준점
// ---------------------------------------------------------------------------

/// 여러 트랙·여러 클립, 클립 안 여러 노트. 기준점은 클립 시작이 아니라 그 클립의 가장 이른 「선택」
/// 노트다. 선택되지 않은 더 이른 노트는 기준이 아니다.
#[test]
fn review_r2_scale_anchor_per_clip_across_tracks() {
    let mut p = Project::new();
    let mut ids = Vec::new();
    for _ in 0..2 {
        let mut t = new_track();
        let tid = t.id();
        let mut a = Clip::new(Tick::ZERO, 3840).unwrap();
        for n in [note(100, 100, 50), note(960, 480, 60), note(1920, 480, 62)] {
            a.insert_note(n).unwrap();
        }
        let mut b = Clip::new(Tick::new(7680), 3840).unwrap();
        for n in [note(1000, 200, 60), note(1400, 200, 62)] {
            b.insert_note(n).unwrap();
        }
        let a = t.insert_clip(a).unwrap();
        let b = t.insert_clip(b).unwrap();
        p.push_track(t).unwrap();
        ids.push((tid, a, b));
    }
    let mut d = Document::new(p, usize::MAX).unwrap();
    let mut sel = Vec::new();
    for &(tid, a, b) in &ids {
        sel.extend(refs(tid, a, &[note(960, 480, 60), note(1920, 480, 62)]));
        sel.extend(refs(tid, b, &[note(1400, 200, 62), note(1000, 200, 60)]));
    }
    let edits = transform::scale(d.project(), &sel, 1, 2).unwrap();
    d.apply(edits, None).unwrap();
    for &(tid, a, b) in &ids {
        assert_eq!(
            notes_of(d.project(), tid, a),
            vec![note(100, 100, 50), note(960, 240, 60), note(1440, 240, 62)]
        );
        assert_eq!(
            notes_of(d.project(), tid, b),
            vec![note(1000, 100, 60), note(1200, 100, 62)]
        );
    }
    assert!(d.undo().unwrap());
    let edits = transform::scale(d.project(), &sel, 3, 1).unwrap();
    d.apply(edits, None).unwrap();
    for &(tid, a, b) in &ids {
        assert_eq!(
            notes_of(d.project(), tid, a),
            vec![
                note(100, 100, 50),
                note(960, 1440, 60),
                note(3840, 1440, 62)
            ]
        );
        // a는 3840 + 1440 = 5280 → 7680으로 늘어 b(7680 시작)와 맞닿는다.
        assert_eq!(clip_len(d.project(), tid, a), 7680);
        assert_eq!(
            notes_of(d.project(), tid, b),
            vec![note(1000, 600, 60), note(2200, 600, 62)]
        );
    }
}

/// 꼬리 노트가 클립의 가장 이른 선택 노트(기준점)일 때. 이웃이 맞닿아 있어도 반속·배속 모두 된다.
#[test]
fn review_r2_scale_with_tail_as_anchor() {
    let tail = note(1000, 3000, 64); // 끝 4000 > 3840.
    let plain = note(2000, 100, 60);
    let (mut d, t, c, _) = split_doc(0, &[tail.clone(), plain.clone()], true, None);
    let sel = refs(t, c, &[plain, tail]);
    let edits = transform::scale(d.project(), &sel, 1, 2).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(
        notes_of(d.project(), t, c),
        vec![note(1000, 1500, 64), note(1500, 50, 60)]
    );
    assert!(d.undo().unwrap());
    let edits = transform::scale(d.project(), &sel, 2, 1).unwrap();
    d.apply(edits, None).unwrap();
    assert_eq!(clip_len(d.project(), t, c), 3840);
    assert_eq!(
        notes_of(d.project(), t, c),
        vec![note(1000, 6000, 64), note(3000, 200, 60)]
    );
}

/// 같은 결함이 스케일에도 있다: 일반 노트(0)가 기준점이고 꼬리(1920, 끝 5760)를 ×3 하면 꼬리 시작
/// 5760이 클립 밖이라 7680으로 늘리는데, 늘린 길이 ≥ 원래 꼬리 끝이라 적용에서 거부된다.
#[test]
fn review_r2_scale_tail_start_out_of_clip_applies() {
    let tail = note(1920, 3840, 64);
    let plain = note(0, 100, 60);
    let (mut d, t, left, _) = split_doc(0, &[tail.clone(), plain.clone()], false, None);
    let edits = transform::scale(d.project(), &refs(t, left, &[plain, tail]), 3, 1)
        .expect("꼬리 시작 5760을 덮도록 7680으로 늘린다");
    let r = d.apply(edits, None);
    assert!(r.is_ok(), "연산이 Ok로 낸 묶음이 적용에서 실패한다: {r:?}");
    assert_eq!(clip_len(d.project(), t, left), 7680);
}
