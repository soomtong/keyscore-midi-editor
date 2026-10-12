//! S0-T4 적대적 리뷰 1회차의 깨뜨리는 테스트. 공개 API만 쓴다.
//!
//! 리뷰 1 당시에는 역편집(`RestoreNote`, `UnsplitClip`, id를 지정한 `InsertClip`)이 공개 `Edit`였고
//! `Edit::apply`도 공개였다. 시도 1–3은 그 값을 손으로 만들거나 보관했다가 다른 맥락에 적용해
//! 불변식과 원자성을 깨뜨렸다. 수정 뒤에는 이 값들을 밖에서 만들 수 없다(`Edit` 문서의
//! `compile_fail` 예 참고). 그래서 시도 1–3은 같은 의도를 지금의 공개 API(히스토리)로 다시
//! 시도하는 테스트로 바꿨다. 시도 4는 그대로다.

use keyscore_core::{
    Channel, Clip, ClipId, Edit, History, Note, Pitch, Project, Tick, Track, TrackColor, TrackId,
    Velocity,
};

fn note(start: u64, length: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        length,
        Pitch::new(pitch).unwrap(),
        Velocity::new(1000),
    )
    .unwrap()
}

/// 트랙 하나, 클립 둘(a = 0..960, b = 1920..2880). 각 클립에 노트 하나.
fn fixture() -> (Project, TrackId, ClipId, ClipId) {
    let mut p = Project::new();
    let track = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    let tid = track.id();
    p.push_track(track).unwrap();
    let t = p.track_by_id_mut(tid).unwrap();
    let a = t.insert_clip(Clip::new(Tick::ZERO, 960).unwrap()).unwrap();
    let b = t
        .insert_clip(Clip::new(Tick::new(1920), 960).unwrap())
        .unwrap();
    t.insert_note(a, note(0, 900, 60)).unwrap();
    t.insert_note(b, note(0, 100, 64)).unwrap();
    (p, tid, a, b)
}

/// 분할을 한 번도 하지 않은 프로젝트에 숨김 꼬리 노트가 있는지.
fn tails(p: &Project) -> usize {
    p.tracks()
        .iter()
        .flat_map(Track::clips)
        .map(|(_, c)| c.notes().iter().filter(|n| n.end() > c.length()).count())
        .sum()
}

/// 시도 1(개정): 노트 삭제 → 클립 삭제 → 같은 자리에 짧은 클립 삽입을 히스토리로 하고,
/// undo·redo를 오가도 분할 없이 꼬리가 생기지 않는다(결정 Q1).
#[test]
fn restore_note_reused_on_fresh_short_clip_is_rejected() {
    let (mut p, t, a, _) = fixture();
    let initial = p.clone();
    let mut h = History::new(usize::MAX);
    h.apply(
        &mut p,
        vec![Edit::RemoveNote {
            track: t,
            clip: a,
            note: note(0, 900, 60),
        }],
        None,
    )
    .unwrap();
    h.apply(&mut p, vec![Edit::RemoveClip { track: t, id: a }], None)
        .unwrap();
    h.apply(
        &mut p,
        vec![Edit::InsertClip {
            track: t,
            clip: Box::new(Clip::new(Tick::ZERO, 10).unwrap()),
        }],
        None,
    )
    .unwrap();
    // 새 클립은 새 id를 받는다. 지운 클립의 id를 다시 쓸 공개 경로가 없다.
    assert!(p.track(t).unwrap().clip(a).is_none());
    for _ in 0..3 {
        assert!(h.undo(&mut p).unwrap());
        assert_eq!(tails(&p), 0);
    }
    assert_eq!(p, initial);
    for _ in 0..3 {
        assert!(h.redo(&mut p).unwrap());
        assert_eq!(tails(&p), 0);
    }
}

/// 시도 2(개정): 분할 되돌리기는 히스토리 안에서만 만들어진다. 분할을 히스토리로 하고 undo하면
/// 초기 상태다. 위조된 되돌리기의 거부는 crate 안 단위 테스트
/// `unsplit_rejects_clips_that_were_not_split_from_original`가 맡는다.
#[test]
fn handmade_unsplit_then_undo_restores_initial() {
    let (mut p, t, a, _) = fixture();
    let initial = p.clone();
    let mut h = History::new(usize::MAX);
    h.apply(
        &mut p,
        vec![Edit::SplitClip {
            track: t,
            id: a,
            at: Tick::new(480),
        }],
        None,
    )
    .unwrap();
    assert_eq!(p.track(t).unwrap().clips().count(), 3);
    let undone = h.undo(&mut p);
    assert!(matches!(undone, Ok(true)), "undo 실패: {undone:?}");
    assert_eq!(p, initial, "undo 1번이 초기 상태가 아니다");
}

/// 시도 3(개정): 분할 뒤에 실패하는 편집을 묶으면 묶음 전체가 되돌려지고, 프로젝트와
/// 히스토리는 그대로다(원자성).
#[test]
fn handmade_unsplit_in_failing_batch_leaves_project_unchanged() {
    let (mut p, t, a, _) = fixture();
    let before = p.clone();
    let stranger = Track::new("X", Channel::new(2).unwrap(), TrackColor::new(0).unwrap()).id();
    let mut h = History::new(usize::MAX);
    let result = h.apply(
        &mut p,
        vec![
            Edit::SplitClip {
                track: t,
                id: a,
                at: Tick::new(480),
            },
            Edit::RemoveTrack { track: stranger },
        ],
        None,
    );
    assert!(result.is_err());
    assert_eq!(p, before, "실패한 묶음이 프로젝트를 바꿨다: {result:?}");
    assert_eq!(h.undo_len(), 0);
}

/// 같은 (시작, 음높이)에 길이만 다른 노트 둘. 앞의 것을 지우고 undo하면 원래 순서로 돌아와야
/// 한다. 노트 벡터 순서가 `PartialEq`에 들어가므로 순서가 바뀌면 초기 상태와 다르다.
#[test]
fn remove_first_of_same_key_notes_then_undo_restores_order() {
    let (mut p, t, a, _) = fixture();
    let mut h = History::new(usize::MAX);
    h.apply(
        &mut p,
        vec![
            Edit::InsertNote {
                track: t,
                clip: a,
                note: note(100, 10, 70),
            },
            Edit::InsertNote {
                track: t,
                clip: a,
                note: note(100, 20, 70),
            },
        ],
        None,
    )
    .unwrap();
    let before = p.clone();
    h.apply(
        &mut p,
        vec![Edit::RemoveNote {
            track: t,
            clip: a,
            note: note(100, 10, 70),
        }],
        None,
    )
    .unwrap();
    assert!(h.undo(&mut p).unwrap());
    assert_eq!(p, before, "undo 뒤 노트 순서가 바뀌었다");
}
