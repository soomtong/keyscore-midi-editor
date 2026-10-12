//! S0-T4 적대적 리뷰 1회차의 깨뜨리는 테스트. 공개 API만 쓴다.

use keyscore_core::{
    Channel, Clip, ClipId, Edit, History, Note, Pitch, Project, Tick, Track, TrackColor, Velocity,
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
    t.insert_note(a, note(0, 900, 60)).unwrap();
    t.insert_note(b, note(0, 100, 64)).unwrap();
    (p, a, b)
}

/// 숨김 꼬리는 분할로만 생긴다(결정 Q1). 노트 삭제로 얻은 `RestoreNote`는 `Clone`이라
/// 나중에 다른 상태에 다시 적용할 수 있다. 같은 id의 짧은 새 클립에 적용하면 분할을 한 번도
/// 거치지 않은 클립에 꼬리가 생긴다.
#[test]
fn restore_note_reused_on_fresh_short_clip_is_rejected() {
    let (mut p, a, _) = fixture();
    let restore = Edit::RemoveNote {
        track: 0,
        clip: a,
        note: note(0, 900, 60),
    }
    .apply(&mut p)
    .unwrap();
    // 같은 id로 길이 10짜리 빈 클립을 새로 만든다(분할 없음).
    Edit::RemoveClip { track: 0, id: a }.apply(&mut p).unwrap();
    Edit::InsertClip {
        track: 0,
        clip: Box::new(Clip::new(Tick::ZERO, 10).unwrap()),
        id: Some(a),
    }
    .apply(&mut p)
    .unwrap();

    let mut h = History::new(usize::MAX);
    let result = h.apply(&mut p, vec![restore], None);
    let clip = p.tracks()[0].clip(a).unwrap();
    let tails = clip
        .notes()
        .iter()
        .filter(|n| n.end() > clip.length())
        .count();
    assert!(
        result.is_err() && tails == 0,
        "분할 없이 꼬리 노트가 생겼다: {result:?}, 클립 길이 {}, 노트 {:?}",
        clip.length(),
        clip.notes()
    );
}

/// 손으로 만든 `UnsplitClip`은 서로 붙어 있지 않은 두 클립을 받아 오른쪽을 지운다.
/// 히스토리는 이를 받아들이지만 undo가 초기 상태로 돌리지 못한다(지운 클립 b와 노트 손실).
#[test]
fn handmade_unsplit_then_undo_restores_initial() {
    let (mut p, a, b) = fixture();
    let initial = p.clone();
    let mut h = History::new(usize::MAX);
    let applied = h.apply(
        &mut p,
        vec![Edit::UnsplitClip {
            track: 0,
            left: a,
            right: b,
            original: Box::new(Clip::new(Tick::ZERO, 960).unwrap()),
        }],
        None,
    );
    if applied.is_err() {
        // 거부했다면 상태가 그대로여야 한다.
        assert_eq!(p, initial);
        return;
    }
    let undone = h.undo(&mut p);
    assert!(matches!(undone, Ok(true)), "undo 실패: {undone:?}");
    assert_eq!(p, initial, "undo 1번이 초기 상태가 아니다");
}

/// 손으로 만든 `UnsplitClip` 뒤에 실패하는 편집을 묶으면, `apply_all`의 되돌리기가 실패해
/// `HistoryInconsistent`를 돌려주면서 프로젝트는 바뀐 채로 남는다(원자성 위반).
#[test]
fn handmade_unsplit_in_failing_batch_leaves_project_unchanged() {
    let (mut p, a, b) = fixture();
    let before = p.clone();
    let mut h = History::new(usize::MAX);
    let result = h.apply(
        &mut p,
        vec![
            Edit::UnsplitClip {
                track: 0,
                left: a,
                right: b,
                original: Box::new(Clip::new(Tick::ZERO, 960).unwrap()),
            },
            Edit::RemoveTrack { index: 99 },
        ],
        None,
    );
    assert!(result.is_err());
    assert_eq!(p, before, "실패한 묶음이 프로젝트를 바꿨다: {result:?}");
    assert_eq!(h.undo_len(), 0);
}

/// 같은 (시작, 음높이)에 길이만 다른 노트 둘. 앞의 것을 지우고 undo하면 `restore_note`가
/// 같은 키 뒤에 넣어 순서가 바뀐다. 노트 벡터 순서가 `PartialEq`에 들어가므로 초기 상태와 다르다.
#[test]
fn remove_first_of_same_key_notes_then_undo_restores_order() {
    let (mut p, a, _) = fixture();
    let mut h = History::new(usize::MAX);
    h.apply(
        &mut p,
        vec![
            Edit::InsertNote {
                track: 0,
                clip: a,
                note: note(100, 10, 70),
            },
            Edit::InsertNote {
                track: 0,
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
            track: 0,
            clip: a,
            note: note(100, 10, 70),
        }],
        None,
    )
    .unwrap();
    assert!(h.undo(&mut p).unwrap());
    assert_eq!(p, before, "undo 뒤 노트 순서가 바뀌었다");
}
