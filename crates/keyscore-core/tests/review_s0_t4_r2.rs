//! S0-T4 적대적 리뷰 2회차의 깨뜨리는 테스트. 공개 API만 쓴다.
//!
//! 리뷰 1 뒤 역편집은 `History` 안에만 있고 트랙은 `TrackId`로 가리킨다. 이 파일은 그 설계가
//! 기대는 두 전제를 노린다: 「프로젝트 안의 `TrackId`는 유일하다」와 「역편집은 그것을 만든
//! 맥락에만 적용된다」.
//!
//! 리뷰 2 뒤 개정: 시도 1–5는 프로젝트를 히스토리 밖에서 고치거나(`&mut Project`), 히스토리를
//! 복제·교체해 두 전제를 깨뜨렸다. 수정 뒤 공개 API는 프로젝트와 히스토리를 한 값에 묶은
//! `Document`이고, `&mut Project`를 내주지 않으며 히스토리를 따로 꺼낼 수 없다(`Document` 문서의
//! `compile_fail` 예). 그래서 각 시도를 같은 목표를 `Document`로 노리는 테스트로 바꿨다.
//! 시도 1의 `Project` 수준 중복은 여전히 만들 수 있다(구성 단계). 그 중복은 `Document::new`가 막는다.

use keyscore_core::{
    Channel, Clip, ClipId, Document, Edit, ModelError, Note, Pitch, Project, Tick, Track,
    TrackColor, TrackId, Velocity,
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

/// 시도 1(개정). `track_mut`로 복제본을 대입하면 `Project` 안에서는 중복이 생긴다(구성 단계).
/// 그 프로젝트로는 편집을 시작할 수 없다.
#[test]
fn track_mut_assignment_duplicate_is_rejected_by_document() {
    let (mut p, ..) = fixture();
    let copy = p.tracks()[0].clone();
    *p.track_mut(1).unwrap() = copy;
    assert!(
        !track_ids_unique(&p),
        "구성 단계에서는 중복을 만들 수 있다(전제 확인)"
    );
    assert_eq!(
        Document::new(p, usize::MAX).unwrap_err(),
        ModelError::DuplicateTrackId
    );
}

/// 시도 2(개정). 문서 안에서 복제 트랙을 넣어 중복을 만들려 하면, 그 편집이 실패하고 묶음 전체가
/// 되돌려진다(원자성). 트랙 A를 지운 채로 남지 않는다.
#[test]
fn duplicated_track_id_breaks_batch_atomicity() {
    let (p, ia, ..) = fixture();
    let copy_of_b = p.tracks()[1].clone();
    let mut d = Document::new(p, usize::MAX).unwrap();
    let before = d.project().clone();
    let result = d.apply(
        vec![
            Edit::RemoveTrack { track: ia },
            Edit::InsertTrack {
                index: 0,
                track: Box::new(copy_of_b),
            },
        ],
        None,
    );
    assert_eq!(result, Err(ModelError::DuplicateTrackId));
    assert_eq!(d.project(), &before, "실패한 apply가 프로젝트를 바꿨다");
    assert!(track_ids_unique(d.project()) && clip_ids_unique(d.project()));
}

/// 시도 3(개정). 히스토리를 따로 복제할 수 없다. 문서를 복제하면 프로젝트와 히스토리가 함께
/// 복제되어, 복제본의 undo는 복제본의 프로젝트에만 맞게 적용된다. 분할 없이 꼬리가 생기지 않는다.
#[test]
fn cloned_history_undo_does_not_create_tail_without_split() {
    let (p, ia, _, a) = fixture();
    let mut d = Document::new(p, usize::MAX).unwrap();
    d.apply(
        vec![Edit::RemoveNote {
            track: ia,
            clip: a,
            note: note(0, 900, 60),
        }],
        None,
    )
    .unwrap();
    let mut stale = d.clone();
    d.apply(
        vec![Edit::SplitClip {
            track: ia,
            id: a,
            at: Tick::new(10),
        }],
        None,
    )
    .unwrap();
    assert_eq!(tails(d.project()), 0);
    let original_after_split = d.project().clone();
    assert!(stale.undo().unwrap());
    assert_eq!(tails(stale.project()), 0);
    assert_eq!(
        d.project(),
        &original_after_split,
        "복제본의 undo가 원본을 바꿨다"
    );
    assert!(d.undo().unwrap() && d.undo().unwrap());
    assert_eq!(d.project(), stale.project(), "둘 다 처음 상태로 돌아간다");
}

/// 시도 4(개정). 다른 히스토리가 없으므로, 분할 뒤 오른쪽에 넣은 노트는 같은 문서의 undo
/// 단위다. undo는 그 노트를 먼저 지우고 그다음 분할을 되돌린다. 노트가 말없이 사라지지 않는다.
#[test]
fn undo_split_after_note_from_other_history_keeps_note_or_fails() {
    let (p, ia, _, a) = fixture();
    let mut d = Document::new(p, usize::MAX).unwrap();
    d.apply(
        vec![Edit::SplitClip {
            track: ia,
            id: a,
            at: Tick::new(480),
        }],
        None,
    )
    .unwrap();
    let right = d
        .project()
        .track(ia)
        .unwrap()
        .clips()
        .map(|(id, _)| id)
        .find(|&id| id != a)
        .unwrap();
    let added = note(0, 10, 72);
    d.apply(
        vec![Edit::InsertNote {
            track: ia,
            clip: right,
            note: added.clone(),
        }],
        None,
    )
    .unwrap();
    let with_note = d.project().clone();
    let has_added = |p: &Project| {
        p.tracks()
            .iter()
            .flat_map(Track::clips)
            .any(|(_, c)| c.notes().contains(&added))
    };
    assert!(d.undo().unwrap());
    assert!(!has_added(d.project()), "첫 undo는 노트 삽입을 되돌린다");
    assert!(d.undo().unwrap());
    assert!(d.redo().unwrap() && d.redo().unwrap());
    assert!(has_added(d.project()));
    assert_eq!(d.project(), &with_note);
}

/// 시도 5(개정). 히스토리 밖에서 클립을 줄일 수 없다. 같은 모양(노트 삭제 → 분할 → 오른쪽
/// 클립 삭제)을 문서로 하고 undo하면, 각 단계에서 분할 없이 생긴 꼬리가 없고 처음 상태로 돌아간다.
#[test]
fn undo_after_outside_shrink_does_not_create_tail() {
    let (p, ia, _, a) = fixture();
    let mut d = Document::new(p, usize::MAX).unwrap();
    let initial = d.project().clone();
    d.apply(
        vec![Edit::RemoveNote {
            track: ia,
            clip: a,
            note: note(0, 900, 60),
        }],
        None,
    )
    .unwrap();
    d.apply(
        vec![Edit::SplitClip {
            track: ia,
            id: a,
            at: Tick::new(10),
        }],
        None,
    )
    .unwrap();
    let right = d
        .project()
        .track(ia)
        .unwrap()
        .clips()
        .map(|(id, _)| id)
        .find(|&id| id != a)
        .unwrap();
    d.apply(
        vec![Edit::RemoveClip {
            track: ia,
            id: right,
        }],
        None,
    )
    .unwrap();
    for _ in 0..3 {
        assert!(d.undo().unwrap());
        assert_eq!(tails(d.project()), 0);
    }
    assert_eq!(d.project(), &initial);
}

/// 시도 6(못 깨뜨림 기대). 트랙 A의 클립을 꺼내 트랙 B에 넣으면 새 id를 받는다. 히스토리
/// 경유와 직접 호출 모두. 그리고 undo 전부 = 초기, redo 전부 = 최종(`TrackId`·`ClipId` 포함).
#[test]
fn clip_moved_between_tracks_gets_new_id_and_round_trips() {
    let (p, ia, ib, a) = fixture();
    let initial = p.clone();
    let clip = p.track(ia).unwrap().clip(a).unwrap().clone();
    let mut d = Document::new(p, usize::MAX).unwrap();
    d.apply(
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
    let moved: Vec<ClipId> = d
        .project()
        .track(ib)
        .unwrap()
        .clips()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(moved.len(), 1);
    assert_ne!(moved[0], a, "다른 트랙으로 옮긴 클립이 같은 id를 받았다");
    assert!(clip_ids_unique(d.project()) && track_ids_unique(d.project()));
    let after = d.project().clone();
    assert!(d.undo().unwrap());
    assert_eq!(d.project(), &initial);
    assert!(d.redo().unwrap());
    assert_eq!(d.project(), &after);

    // 히스토리 밖 직접 호출도 새 id다.
    let mut q = initial.clone();
    let taken = q.track_by_id_mut(ia).unwrap().remove_clip(a).unwrap();
    let id = q.track_by_id_mut(ib).unwrap().insert_clip(taken).unwrap();
    assert_ne!(id, a);
    assert!(clip_ids_unique(&q));
}
