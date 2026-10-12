//! S0-T4 적대적 리뷰 2회차의 깨뜨리는 테스트. 공개 API만 쓴다.
//!
//! 리뷰 1 뒤 역편집은 `History` 안에만 있고 트랙은 `TrackId`로 가리킨다. 이 파일은 그 설계가
//! 기대는 두 전제를 노린다: 「프로젝트 안의 `TrackId`는 유일하다」와 「역편집은 그것을 만든
//! 맥락에만 적용된다」.

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

fn new_track(name: &str) -> Track {
    Track::new(name, Channel::new(1).unwrap(), TrackColor::new(0).unwrap())
}

/// 트랙 둘(A, B). A에 클립 a(0..960)와 노트 (0, 길이 900).
fn fixture() -> (Project, TrackId, TrackId, ClipId) {
    let mut p = Project::new();
    let (ta, tb) = (new_track("A"), new_track("B"));
    let (ia, ib) = (ta.id(), tb.id());
    p.push_track(ta).unwrap();
    p.push_track(tb).unwrap();
    let t = p.track_by_id_mut(ia).unwrap();
    let a = t.insert_clip(Clip::new(Tick::ZERO, 960).unwrap()).unwrap();
    t.insert_note(a, note(0, 900, 60)).unwrap();
    (p, ia, ib, a)
}

fn tails(p: &Project) -> usize {
    p.tracks()
        .iter()
        .flat_map(Track::clips)
        .map(|(_, c)| c.notes().iter().filter(|n| n.end() > c.length()).count())
        .sum()
}

fn track_ids_unique(p: &Project) -> bool {
    let mut ids: Vec<TrackId> = p.tracks().iter().map(Track::id).collect();
    ids.sort();
    ids.windows(2).all(|w| w[0] != w[1])
}

fn clip_ids_unique(p: &Project) -> bool {
    let mut ids: Vec<ClipId> = p
        .tracks()
        .iter()
        .flat_map(|t| t.clips().map(|(id, _)| id))
        .collect();
    ids.sort();
    ids.windows(2).all(|w| w[0] != w[1])
}

/// 시도 1. `Project::track_mut`가 내주는 `&mut Track`에 다른 트랙의 복제본을 대입하면
/// `insert_track`의 중복 검사를 거치지 않는다. 프로젝트 안에서 `TrackId`와 `ClipId`가 겹친다.
#[test]
fn track_mut_assignment_cannot_duplicate_track_id() {
    let (mut p, ..) = fixture();
    let copy = p.tracks()[0].clone();
    *p.track_mut(1).unwrap() = copy;
    assert!(
        track_ids_unique(&p),
        "같은 TrackId의 트랙 둘: {:?}",
        p.tracks().iter().map(Track::id).collect::<Vec<_>>()
    );
    assert!(clip_ids_unique(&p), "같은 ClipId의 클립이 두 트랙에 있다");
}

/// 시도 2. 시도 1의 상태에서 `RemoveTrack` 뒤 실패하는 편집을 묶는다. 되돌리기의
/// `InsertTrack`이 남은 복제본 때문에 `DuplicateTrackId`로 실패해, `apply`가
/// `Err(HistoryInconsistent)`를 돌려주면서 트랙 A를 지운 채로 둔다(원자성 위반, 데이터 손실).
#[test]
fn duplicated_track_id_breaks_batch_atomicity() {
    let (mut p, ia, ..) = fixture();
    let mut copy = p.tracks()[0].clone();
    std::mem::swap(p.track_mut(1).unwrap(), &mut copy);
    let before = p.clone();
    let stranger = new_track("X").id();
    let mut h = History::new(usize::MAX);
    let result = h.apply(
        &mut p,
        vec![
            Edit::RemoveTrack { track: ia },
            Edit::RemoveTrack { track: stranger },
        ],
        None,
    );
    assert!(result.is_err());
    assert_eq!(p, before, "실패한 apply가 프로젝트를 바꿨다: {result:?}");
}

/// 시도 3. 리뷰 1 시도 1의 의도(보관한 역편집을 다른 맥락에 적용)를 `History::clone`으로
/// 다시 노린다. 모든 편집은 히스토리로 적용한다. 노트를 지운 뒤 히스토리를 복제하고, 원본
/// 히스토리로 클립을 분할해 짧게 만든 다음, 복제본으로 undo하면 그 분할을 한 번도 가로지르지
/// 않은 노트가 숨김 꼬리로 되살아난다(결정 Q1).
#[test]
fn cloned_history_undo_does_not_create_tail_without_split() {
    let (mut p, ia, _, a) = fixture();
    let mut h = History::new(usize::MAX);
    h.apply(
        &mut p,
        vec![Edit::RemoveNote {
            track: ia,
            clip: a,
            note: note(0, 900, 60),
        }],
        None,
    )
    .unwrap();
    let mut stale = h.clone();
    h.apply(
        &mut p,
        vec![Edit::SplitClip {
            track: ia,
            id: a,
            at: Tick::new(10),
        }],
        None,
    )
    .unwrap();
    assert_eq!(tails(&p), 0, "분할 시점에 노트가 없었으므로 꼬리도 없다");
    let before = p.clone();
    let result = stale.undo(&mut p);
    assert!(
        tails(&p) == 0 && (result.is_ok() || p == before),
        "분할 없이 꼬리 노트가 생겼다: {result:?}, 클립 {:?}",
        p.track(ia).unwrap().clip(a)
    );
}

/// 시도 4. 분할 undo는 지금 두 클립의 경계·끝만 확인하고 `original`로 덮어쓴다. 분할 뒤
/// 오른쪽 클립에 넣은 노트가 다른 히스토리(또는 히스토리 밖)에서 왔으면, undo는 `Ok`를
/// 돌려주면서 그 노트를 지운다. `History` 문서는 이 경우 `Err`이고 프로젝트는 그대로라고 한다.
#[test]
fn undo_split_after_note_from_other_history_keeps_note_or_fails() {
    let (mut p, ia, _, a) = fixture();
    let mut h = History::new(usize::MAX);
    h.apply(
        &mut p,
        vec![Edit::SplitClip {
            track: ia,
            id: a,
            at: Tick::new(480),
        }],
        None,
    )
    .unwrap();
    let right = p
        .track(ia)
        .unwrap()
        .clips()
        .map(|(id, _)| id)
        .find(|&id| id != a)
        .unwrap();
    let mut other = History::new(usize::MAX);
    let added = note(0, 10, 72);
    other
        .apply(
            &mut p,
            vec![Edit::InsertNote {
                track: ia,
                clip: right,
                note: added.clone(),
            }],
            None,
        )
        .unwrap();
    let before = p.clone();
    let result = h.undo(&mut p);
    let kept = p
        .track(ia)
        .unwrap()
        .clips()
        .any(|(_, c)| c.notes().iter().any(|n| n.pitch() == added.pitch()));
    assert!(
        (result.is_err() && p == before) || kept,
        "undo가 {result:?}를 돌려주며 다른 곳에서 넣은 노트를 지웠다"
    );
}

/// 시도 5. 히스토리 밖에서 클립을 분할하고 오른쪽을 지운 뒤 노트 삭제를 undo한다.
/// `History` 문서는 밖에서 고친 뒤의 undo가 실패하더라도 프로젝트를 바꾸지 않는다고 하지만,
/// 실제로는 `Ok`이고 분할을 거치지 않은 노트가 꼬리로 되살아난다.
#[test]
fn undo_after_outside_shrink_does_not_create_tail() {
    let (mut p, ia, _, a) = fixture();
    let mut h = History::new(usize::MAX);
    h.apply(
        &mut p,
        vec![Edit::RemoveNote {
            track: ia,
            clip: a,
            note: note(0, 900, 60),
        }],
        None,
    )
    .unwrap();
    let t = p.track_by_id_mut(ia).unwrap();
    let right = t.split_clip(a, Tick::new(10)).unwrap();
    t.remove_clip(right).unwrap();
    let before = p.clone();
    let result = h.undo(&mut p);
    assert!(
        tails(&p) == 0 && (result.is_ok() || p == before),
        "분할 없이 꼬리 노트가 생겼다: {result:?}"
    );
}

/// 시도 6(못 깨뜨림 기대). 트랙 A의 클립을 꺼내 트랙 B에 넣으면 새 id를 받는다. 히스토리
/// 경유와 직접 호출 모두. 그리고 undo 전부 = 초기, redo 전부 = 최종(`TrackId`·`ClipId` 포함).
#[test]
fn clip_moved_between_tracks_gets_new_id_and_round_trips() {
    let (mut p, ia, ib, a) = fixture();
    let initial = p.clone();
    let mut h = History::new(usize::MAX);
    let clip = p.track(ia).unwrap().clip(a).unwrap().clone();
    h.apply(
        &mut p,
        vec![
            Edit::RemoveClip { track: ia, id: a },
            Edit::InsertClip {
                track: ib,
                clip: Box::new(clip),
            },
            Edit::RemoveTrack { track: ia },
        ],
        None,
    )
    .unwrap();
    let moved: Vec<ClipId> = p.track(ib).unwrap().clips().map(|(id, _)| id).collect();
    assert_eq!(moved.len(), 1);
    assert_ne!(moved[0], a, "다른 트랙으로 옮긴 클립이 같은 id를 받았다");
    assert!(clip_ids_unique(&p) && track_ids_unique(&p));
    let after = p.clone();
    assert!(h.undo(&mut p).unwrap());
    assert_eq!(p, initial);
    assert!(h.redo(&mut p).unwrap());
    assert_eq!(p, after);

    // 히스토리 밖 직접 호출도 새 id다.
    let mut q = initial.clone();
    let taken = q.track_by_id_mut(ia).unwrap().remove_clip(a).unwrap();
    let id = q.track_by_id_mut(ib).unwrap().insert_clip(taken).unwrap();
    assert_ne!(id, a);
    assert!(clip_ids_unique(&q));
}
