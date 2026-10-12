//! S0-T4 적대적 리뷰 3회차의 깨뜨리기 시도. 공개 API(`Document`)만 쓴다.
//!
//! 리뷰 2 뒤 편집의 공개 진입점은 프로젝트와 히스토리를 한 값에 묶은 `Document`다. 이 파일은
//! 그 경계 안에서 남는 경로를 노린다: 옛 스냅숏의 트랙을 다시 넣기(같은 `TrackId`, 옛 `ClipId`),
//! `into_project` 뒤 고쳐서 다시 `Document::new`, 문서 복제본끼리의 간섭, u64 끝 근처의 숨김 꼬리,
//! 묶기(coalesce)와 실패한 묶음이 섞인 undo·redo.

use keyscore_core::{
    Channel, Clip, ClipId, CoalesceKey, Document, Edit, ModelError, Note, Pitch, Project, Tick,
    Track, TrackColor, TrackId, Velocity,
};
use proptest::prelude::*;

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

/// u64 끝 근처에서 시작하는 클립의 시작. 숨김 꼬리가 u64를 넘는지 보려고 쓴다.
const HIGH: u64 = u64::MAX - 4000;

/// 트랙 둘(A, B). A에 클립 a(0..960, 노트 (0, 900)), B에 클립 h(HIGH..HIGH+3000, 노트 (0, 2900)).
fn fixture() -> (Project, TrackId, TrackId, ClipId, ClipId) {
    let mut p = Project::new();
    let (ta, tb) = (new_track("A"), new_track("B"));
    let (ia, ib) = (ta.id(), tb.id());
    p.push_track(ta).unwrap();
    p.push_track(tb).unwrap();
    let t = p.track_by_id_mut(ia).unwrap();
    let a = t.insert_clip(Clip::new(Tick::ZERO, 960).unwrap()).unwrap();
    t.insert_note(a, note(0, 900, 60)).unwrap();
    let t = p.track_by_id_mut(ib).unwrap();
    let h = t
        .insert_clip(Clip::new(Tick::new(HIGH), 3000).unwrap())
        .unwrap();
    t.insert_note(h, note(0, 2900, 62)).unwrap();
    (p, ia, ib, a, h)
}

/// 문서 안에서 지켜야 할 불변식 전부. 실패하면 이유.
fn check(p: &Project) -> Result<(), String> {
    let mut track_ids: Vec<TrackId> = p.tracks().iter().map(Track::id).collect();
    let n = track_ids.len();
    track_ids.sort();
    track_ids.dedup();
    if track_ids.len() != n {
        return Err("같은 TrackId의 트랙 둘".into());
    }
    let mut clip_ids: Vec<ClipId> = p
        .tracks()
        .iter()
        .flat_map(|t| t.clips().map(|(id, _)| id))
        .collect();
    let n = clip_ids.len();
    clip_ids.sort();
    clip_ids.dedup();
    if clip_ids.len() != n {
        return Err("같은 ClipId의 클립 둘(트랙 사이 포함)".into());
    }
    for track in p.tracks() {
        let clips: Vec<&Clip> = track.clips().map(|(_, c)| c).collect();
        for pair in clips.windows(2) {
            if pair[0].end() > pair[1].start().get() {
                return Err(format!("클립 겹침: {:?} / {:?}", pair[0], pair[1]));
            }
        }
        for clip in clips {
            for n in clip.notes() {
                if n.start().get() >= clip.length() {
                    return Err(format!("노트 시작이 클립 밖: {clip:?}"));
                }
                if clip.start().get().checked_add(n.end()).is_none() {
                    return Err(format!("노트 절대 끝이 u64를 넘는다: {clip:?}"));
                }
            }
        }
    }
    Ok(())
}

/// 시도 1. 옛 스냅숏의 트랙(같은 `TrackId`, 옛 `ClipId`)을 `InsertTrack`으로 다시 넣어
/// 프로젝트 안에서 `ClipId`를 겹치게 만들려 한다. 트랙을 지운 뒤 다시 넣기, 지우고 넣기를 한
/// 묶음으로 하기(옛 판으로 바꿔치기), 복제 문서에서 분할해 새 id를 받은 판을 원본에 넣기를 모두
/// 해 본다. 각 단계 뒤 불변식, 끝에서 undo 전부 = 초기, redo 전부 = 최종.
#[test]
fn reinserting_stale_track_cannot_share_clip_id_and_round_trips() {
    let (project, ia, ib, a, _) = fixture();
    let mut d = Document::new(project, usize::MAX).unwrap();
    let initial = d.project().clone();
    let snapshot_a = d.project().track(ia).unwrap().clone();

    // 복제 문서에서 a를 분할해 오른쪽 id를 받은 판의 A를 만든다.
    let mut fork = d.clone();
    fork.apply(
        vec![Edit::SplitClip {
            track: ia,
            id: a,
            at: Tick::new(100),
        }],
        None,
    )
    .unwrap();
    let forked_a = fork.project().track(ia).unwrap().clone();

    // A가 있는 동안에는 어느 판도 넣을 수 없다.
    for t in [&snapshot_a, &forked_a] {
        let r = d.apply(
            vec![Edit::InsertTrack {
                index: 0,
                track: Box::new(t.clone()),
            }],
            None,
        );
        assert_eq!(r, Err(ModelError::DuplicateTrackId));
    }

    // 원본에서 a를 분할하면 복제본과 다른 오른쪽 id를 받는다.
    d.apply(
        vec![Edit::SplitClip {
            track: ia,
            id: a,
            at: Tick::new(100),
        }],
        None,
    )
    .unwrap();
    check(d.project()).unwrap();

    // 한 묶음으로 A를 복제 문서의 판으로 바꿔치기한다. 이제 프로젝트에는 복제본이 발급한 id가 있다.
    d.apply(
        vec![
            Edit::RemoveTrack { track: ia },
            Edit::InsertTrack {
                index: 1,
                track: Box::new(forked_a.clone()),
            },
        ],
        None,
    )
    .unwrap();
    check(d.project()).unwrap();

    // 다시 처음 판으로 바꿔치기(분할 전 a). 그다음 B 자리에 넣어 보기(같은 id라 실패해야 한다).
    d.apply(
        vec![
            Edit::RemoveTrack { track: ia },
            Edit::InsertTrack {
                index: 0,
                track: Box::new(snapshot_a.clone()),
            },
        ],
        None,
    )
    .unwrap();
    check(d.project()).unwrap();
    let r = d.apply(
        vec![
            Edit::RemoveTrack { track: ib },
            Edit::InsertTrack {
                index: 1,
                track: Box::new(forked_a.clone()),
            },
        ],
        None,
    );
    assert_eq!(r, Err(ModelError::DuplicateTrackId));
    check(d.project()).unwrap();

    let last = d.project().clone();
    let n = d.undo_len();
    for _ in 0..n {
        assert!(d.undo().unwrap());
        check(d.project()).unwrap();
    }
    assert_eq!(d.project(), &initial);
    for _ in 0..n {
        assert!(d.redo().unwrap());
        check(d.project()).unwrap();
    }
    assert_eq!(d.project(), &last);
}

/// 시도 2. `into_project`로 꺼낸 프로젝트를 고쳐 다시 `Document::new`에 넣는다. 다른 문서(복제본)의
/// 트랙을 `track_mut` 대입·`push_track`으로 섞으면 `TrackId`가 같은 트랙만 `ClipId`를 공유하므로,
/// `new`의 `TrackId` 검사가 모두 잡아야 한다. 잡지 못하면 불변식이 깨진 문서가 생긴다.
#[test]
fn document_new_rejects_every_mix_of_tracks_from_cloned_documents() {
    let (p, ia, ib, a, _) = fixture();
    let d = Document::new(p, usize::MAX).unwrap();
    let mut fork = d.clone();
    fork.apply(
        vec![Edit::SplitClip {
            track: ia,
            id: a,
            at: Tick::new(100),
        }],
        None,
    )
    .unwrap();
    let theirs = fork.into_project();

    // (1) 다른 문서의 A를 우리 B 자리에 대입.
    let mut mine = d.clone().into_project();
    *mine.track_mut(1).unwrap() = theirs.track(ia).unwrap().clone();
    let r1 = Document::new(mine, usize::MAX);
    // (2) 다른 문서의 A를 뒤에 덧붙이기. push_track은 같은 id를 거부한다.
    let mut mine = d.clone().into_project();
    let r2 = mine.push_track(theirs.track(ia).unwrap().clone());
    // (3) 트랙을 맞바꾸기: 우리 B를 지우고 그 자리에 다른 문서의 B. 같은 id라 정상이어야 한다.
    let mut mine = d.clone().into_project();
    *mine.track_mut(1).unwrap() = theirs.track(ib).unwrap().clone();
    let r3 = Document::new(mine, usize::MAX);

    assert_eq!(r1.unwrap_err(), ModelError::DuplicateTrackId);
    assert_eq!(r2, Err(ModelError::DuplicateTrackId));
    let ok = r3.expect("같은 id의 판 교체는 정상 구성이다");
    check(ok.project()).unwrap();
    assert_eq!(
        (ok.undo_len(), ok.redo_len()),
        (0, 0),
        "새 문서는 빈 히스토리"
    );
}

/// 시도 3. 복제 문서 둘을 번갈아 편집·undo·redo해도 서로 영향이 없고, 각자 undo 전부 = 분기 시점,
/// redo 전부 = 각자의 최종이다. 같은 묶기 키를 두 문서에 이어 써도 다른 문서의 단위와 묶이지 않는다.
#[test]
fn cloned_documents_interleaved_do_not_interfere() {
    let (p, ia, _, a, _) = fixture();
    let mut d1 = Document::new(p, usize::MAX).unwrap();
    let key = Some(CoalesceKey(1));
    d1.apply(
        vec![Edit::MoveClip {
            track: ia,
            id: a,
            to: Tick::new(10),
        }],
        key,
    )
    .unwrap();
    let mut d2 = d1.clone();
    let fork_point = d1.project().clone();
    d1.apply(
        vec![Edit::MoveClip {
            track: ia,
            id: a,
            to: Tick::new(20),
        }],
        key,
    )
    .unwrap();
    d2.apply(
        vec![Edit::RemoveNote {
            track: ia,
            clip: a,
            note: note(0, 900, 60),
        }],
        key,
    )
    .unwrap();
    d2.apply(
        vec![Edit::SplitClip {
            track: ia,
            id: a,
            at: Tick::new(15),
        }],
        None,
    )
    .unwrap();
    // 묶기: d1은 두 이동이 한 단위, d2는 이동+노트 삭제가 한 단위 + 분할.
    assert_eq!((d1.undo_len(), d2.undo_len()), (1, 2));
    let (end1, end2) = (d1.project().clone(), d2.project().clone());

    assert!(d2.undo().unwrap());
    assert_eq!(d1.project(), &end1);
    assert!(d1.undo().unwrap());
    assert!(d2.undo().unwrap());
    assert_eq!(d1.project(), d2.project(), "둘 다 이동 전 처음 상태");
    assert_ne!(d1.project(), &fork_point);
    check(d2.project()).unwrap();
    assert!(d1.redo().unwrap());
    assert!(d2.redo().unwrap() && d2.redo().unwrap());
    assert_eq!(d1.project(), &end1);
    assert_eq!(d2.project(), &end2);
    check(d2.project()).unwrap();
}

/// 시도 4. u64 끝 근처의 클립을 숨김 꼬리를 남기게 분할하고, 옮기고, 노트를 지우고, 묶음 실패를
/// 섞은 뒤 undo·redo. 숨김 꼬리의 절대 끝이 u64를 넘는 상태가 생기면 안 된다.
#[test]
fn hidden_tail_near_u64_end_stays_in_range_through_undo_redo() {
    let (project, _, ib, _, h) = fixture();
    let mut d = Document::new(project, usize::MAX).unwrap();
    let initial = d.project().clone();
    // 노트 (0, 2900)을 가로지르는 분할: 왼쪽 h(길이 10)에 꼬리 2890.
    d.apply(
        vec![Edit::SplitClip {
            track: ib,
            id: h,
            at: Tick::new(HIGH + 10),
        }],
        None,
    )
    .unwrap();
    let right = d
        .project()
        .track(ib)
        .unwrap()
        .clips()
        .map(|(id, _)| id)
        .find(|&id| id != h)
        .unwrap();
    // 오른쪽을 지우고 왼쪽을 끝으로 밀면 길이로는 들어가지만 꼬리는 넘친다. 거부되어야 한다.
    d.apply(
        vec![Edit::RemoveClip {
            track: ib,
            id: right,
        }],
        None,
    )
    .unwrap();
    let r = d.apply(
        vec![Edit::MoveClip {
            track: ib,
            id: h,
            to: Tick::new(u64::MAX - 10),
        }],
        None,
    );
    assert_eq!(r, Err(ModelError::TickOverflow));
    check(d.project()).unwrap();
    // 꼬리 노트를 지웠다가, 클립을 끝 가까이 옮긴 뒤(꼬리가 없어 허용), 묶음 undo.
    d.apply(
        vec![
            Edit::RemoveNote {
                track: ib,
                clip: h,
                note: note(0, 2900, 62),
            },
            Edit::MoveClip {
                track: ib,
                id: h,
                to: Tick::new(u64::MAX - 10),
            },
        ],
        None,
    )
    .unwrap();
    check(d.project()).unwrap();
    let last = d.project().clone();
    let n = d.undo_len();
    for _ in 0..n {
        assert!(d.undo().unwrap());
        check(d.project()).unwrap();
    }
    assert_eq!(d.project(), &initial);
    for _ in 0..n {
        assert!(d.redo().unwrap());
        check(d.project()).unwrap();
    }
    assert_eq!(d.project(), &last);
}

/// 시도 5. 실패한 `apply`는 redo 스택·묶기·바이트 수까지 바꾸지 않는다. undo 뒤 실패한 묶음이
/// redo를 지우면 사용자가 다시 할 수 없게 되고, 바이트 수가 바뀌면 상한 계산이 어긋난다.
#[test]
fn failed_apply_keeps_redo_coalescing_and_bytes() {
    let (p, ia, _, a, _) = fixture();
    let mut d = Document::new(p, usize::MAX).unwrap();
    let key = Some(CoalesceKey(9));
    d.apply(
        vec![Edit::MoveClip {
            track: ia,
            id: a,
            to: Tick::new(5),
        }],
        key,
    )
    .unwrap();
    d.apply(
        vec![Edit::SplitClip {
            track: ia,
            id: a,
            at: Tick::new(50),
        }],
        None,
    )
    .unwrap();
    assert!(d.undo().unwrap());
    let snapshot = (
        d.project().clone(),
        d.undo_len(),
        d.redo_len(),
        d.history_bytes(),
    );
    let r = d.apply(
        vec![
            Edit::SplitClip {
                track: ia,
                id: a,
                at: Tick::new(50),
            },
            Edit::RemoveTrack { track: ia },
            Edit::RemoveTrack { track: ia },
        ],
        key,
    );
    assert_eq!(r, Err(ModelError::UnknownTrack));
    assert_eq!(
        (
            d.project().clone(),
            d.undo_len(),
            d.redo_len(),
            d.history_bytes()
        ),
        snapshot
    );
    assert!(d.redo().unwrap(), "실패한 apply가 redo를 지웠다");
    check(d.project()).unwrap();
}

// ---- 속성 테스트: 옛 스냅숏 트랙 재삽입·복제 문서·묶기·실패 묶음·u64 끝을 섞은 무작위 열 ----

#[derive(Debug, Clone)]
enum Act {
    Batch(Vec<Step>, Option<u8>),
    Undo,
    Redo,
    Seal,
    /// 문서를 복제해 복제본을 끝까지 undo하고, 원본이 그대로인지 본다.
    Fork,
}

#[derive(Debug, Clone)]
enum Step {
    InsertClip {
        pick: usize,
        start: u64,
        len: u64,
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
    Split {
        pick: usize,
        clip: usize,
        offset: u64,
    },
    InsertNote {
        pick: usize,
        clip: usize,
        start: u64,
        len: u32,
    },
    RemoveNote {
        pick: usize,
        clip: usize,
        note: usize,
    },
    RemoveTrack {
        pick: usize,
    },
    NewTrack {
        index: usize,
    },
    /// 지금까지 본 상태(복제 문서 포함) 가운데 하나에서 트랙 하나를 꺼내 넣는다.
    Stale {
        snap: usize,
        track: usize,
        index: usize,
    },
    /// 같은 id의 트랙을 지우고 옛 판을 넣는다(한 묶음 안).
    Replace {
        snap: usize,
        track: usize,
    },
}

/// u64 끝 근처 시작점을 섞는다.
fn tick() -> impl Strategy<Value = u64> {
    prop_oneof![3 => 0..6000u64, 1 => (0..6000u64).prop_map(|x| u64::MAX - x)]
}

fn step() -> impl Strategy<Value = Step> {
    let u = any::<usize>;
    prop_oneof![
        2 => (u(), tick(), 1..3000u64).prop_map(|(pick, start, len)| Step::InsertClip { pick, start, len }),
        1 => (u(), u()).prop_map(|(pick, clip)| Step::RemoveClip { pick, clip }),
        2 => (u(), u(), tick()).prop_map(|(pick, clip, to)| Step::MoveClip { pick, clip, to }),
        2 => (u(), u(), 1..3000u64).prop_map(|(pick, clip, offset)| Step::Split { pick, clip, offset }),
        2 => (u(), u(), 0..3000u64, 1..3000u32).prop_map(|(pick, clip, start, len)| Step::InsertNote { pick, clip, start, len }),
        2 => (u(), u(), u()).prop_map(|(pick, clip, note)| Step::RemoveNote { pick, clip, note }),
        1 => u().prop_map(|pick| Step::RemoveTrack { pick }),
        1 => u().prop_map(|index| Step::NewTrack { index }),
        2 => (u(), u(), u()).prop_map(|(snap, track, index)| Step::Stale { snap, track, index }),
        2 => (u(), u()).prop_map(|(snap, track)| Step::Replace { snap, track }),
    ]
}

fn act() -> impl Strategy<Value = Act> {
    prop_oneof![
        6 => (prop::collection::vec(step(), 1..4), prop::option::of(0..2u8)).prop_map(|(s, k)| Act::Batch(s, k)),
        2 => Just(Act::Undo),
        1 => Just(Act::Redo),
        1 => Just(Act::Seal),
        1 => Just(Act::Fork),
    ]
}

fn nth<T>(xs: &[T], i: usize) -> Option<&T> {
    xs.get(i % xs.len().max(1))
}

fn clip_of(p: &Project, track: TrackId, i: usize) -> Option<ClipId> {
    let ids: Vec<ClipId> = p.track(track)?.clips().map(|(id, _)| id).collect();
    nth(&ids, i).copied()
}

/// 편집을 만든다. 같은 묶음의 편집은 모두 묶음 시작 상태 기준이라 뒤 편집이 실패할 수 있다(의도).
fn to_edit(p: &Project, pool: &[Project], s: &Step) -> Option<Vec<Edit>> {
    let track = |i: usize| nth(p.tracks(), i).map(Track::id);
    Some(match *s {
        Step::InsertClip { pick, start, len } => vec![Edit::InsertClip {
            track: track(pick)?,
            clip: Box::new(Clip::new(Tick::new(start), len).ok()?),
        }],
        Step::RemoveClip { pick, clip } => {
            let t = track(pick)?;
            vec![Edit::RemoveClip {
                track: t,
                id: clip_of(p, t, clip)?,
            }]
        }
        Step::MoveClip { pick, clip, to } => {
            let t = track(pick)?;
            vec![Edit::MoveClip {
                track: t,
                id: clip_of(p, t, clip)?,
                to: Tick::new(to),
            }]
        }
        Step::Split { pick, clip, offset } => {
            let t = track(pick)?;
            let id = clip_of(p, t, clip)?;
            let start = p.track(t)?.clip(id)?.start().get();
            vec![Edit::SplitClip {
                track: t,
                id,
                at: Tick::new(start.checked_add(offset)?),
            }]
        }
        Step::InsertNote {
            pick,
            clip,
            start,
            len,
        } => {
            let t = track(pick)?;
            vec![Edit::InsertNote {
                track: t,
                clip: clip_of(p, t, clip)?,
                note: note(start, len, 64),
            }]
        }
        Step::RemoveNote { pick, clip, note } => {
            let t = track(pick)?;
            let id = clip_of(p, t, clip)?;
            let n = nth(p.track(t)?.clip(id)?.notes(), note)?.clone();
            vec![Edit::RemoveNote {
                track: t,
                clip: id,
                note: n,
            }]
        }
        Step::RemoveTrack { pick } => vec![Edit::RemoveTrack {
            track: track(pick)?,
        }],
        Step::NewTrack { index } => vec![Edit::InsertTrack {
            index: index % (p.tracks().len() + 1),
            track: Box::new(new_track("N")),
        }],
        Step::Stale { snap, track, index } => {
            let t = nth(nth(pool, snap)?.tracks(), track)?.clone();
            vec![Edit::InsertTrack {
                index: index % (p.tracks().len() + 1),
                track: Box::new(t),
            }]
        }
        Step::Replace { snap, track } => {
            let t = nth(nth(pool, snap)?.tracks(), track)?.clone();
            let index = p.track_index(t.id())?;
            vec![
                Edit::RemoveTrack { track: t.id() },
                Edit::InsertTrack {
                    index,
                    track: Box::new(t),
                },
            ]
        }
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// 문서의 undo·redo가 「그 시점 상태」로 정확히 돌아가는지를 모형(상태 스택)과 비교한다.
    /// 모든 단계에서 불변식(트랙 사이 포함 ClipId 유일, 겹침 없음, 노트 시작은 클립 안, u64 끝)을
    /// 검사하고, 실패한 묶음은 프로젝트·히스토리 길이·바이트를 바꾸지 않아야 한다.
    #[test]
    fn document_matches_state_stack_model(acts in prop::collection::vec(act(), 0..50)) {
        let (p, ..) = fixture();
        let mut d = Document::new(p, usize::MAX).unwrap();
        let mut pool = vec![d.project().clone()];
        // undo 단위 하나 = 그 단위 직전 상태. redo 단위 하나 = undo 직전 상태.
        let mut undo: Vec<Project> = Vec::new();
        let mut redo: Vec<Project> = Vec::new();
        let (mut last_key, mut sealed) = (None::<u8>, true);
        for a in &acts {
            match a {
                Act::Batch(steps, key) => {
                    let cur = d.project().clone();
                    let Some(edits) = steps
                        .iter()
                        .map(|s| to_edit(&cur, &pool, s))
                        .collect::<Option<Vec<_>>>()
                        .map(|v| v.into_iter().flatten().collect::<Vec<_>>())
                    else { continue };
                    let before = (d.undo_len(), d.redo_len(), d.history_bytes());
                    match d.apply(edits, key.map(|k| CoalesceKey(u64::from(k)))) {
                        Ok(()) => {
                            let merge = !sealed && key.is_some() && *key == last_key && !undo.is_empty();
                            if !merge {
                                undo.push(cur);
                            }
                            redo.clear();
                            last_key = *key;
                            sealed = false;
                            pool.push(d.project().clone());
                        }
                        Err(e) => {
                            prop_assert_ne!(e.clone(), ModelError::HistoryInconsistent);
                            prop_assert_eq!(d.project(), &cur, "실패한 묶음이 프로젝트를 바꿨다: {:?}", e);
                            prop_assert_eq!((d.undo_len(), d.redo_len(), d.history_bytes()), before);
                        }
                    }
                }
                Act::Undo => {
                    let cur = d.project().clone();
                    let r = d.undo();
                    if let Some(prev) = undo.pop() {
                        prop_assert_eq!(r, Ok(true));
                        prop_assert_eq!(d.project(), &prev, "undo가 그 단위 직전 상태가 아니다");
                        redo.push(cur);
                        sealed = true;
                    } else {
                        prop_assert_eq!(r, Ok(false));
                    }
                }
                Act::Redo => {
                    let cur = d.project().clone();
                    let r = d.redo();
                    if let Some(next) = redo.pop() {
                        prop_assert_eq!(r, Ok(true));
                        prop_assert_eq!(d.project(), &next, "redo가 undo 직전 상태가 아니다");
                        undo.push(cur);
                        sealed = true;
                    } else {
                        prop_assert_eq!(r, Ok(false));
                    }
                }
                Act::Seal => {
                    d.seal();
                    sealed = true;
                }
                Act::Fork => {
                    let mine = d.project().clone();
                    let mut f = d.clone();
                    while f.undo().unwrap() {
                        if let Err(why) = check(f.project()) {
                            prop_assert!(false, "복제본 undo 뒤 불변식: {}", why);
                        }
                    }
                    prop_assert_eq!(f.project(), undo.first().unwrap_or(&mine));
                    prop_assert_eq!(d.project(), &mine, "복제본의 undo가 원본을 바꿨다");
                    pool.push(f.project().clone());
                    // 복제본에서 분할 하나(새 ClipId)를 하고 그 상태도 옛 트랙 공급원에 넣는다.
                    if let Some(t) = f.project().tracks().first().map(Track::id)
                        && let Some(c) = clip_of(f.project(), t, 0)
                    {
                        let s = f.project().track(t).unwrap().clip(c).unwrap().start().get();
                        if f.apply(vec![Edit::SplitClip { track: t, id: c, at: Tick::new(s.saturating_add(1)) }], None).is_ok() {
                            pool.push(f.project().clone());
                        }
                    }
                }
            }
            if let Err(why) = check(d.project()) {
                prop_assert!(false, "{:?} 뒤 불변식 깨짐: {}", a, why);
            }
            prop_assert_eq!((d.undo_len(), d.redo_len()), (undo.len(), redo.len()));
        }
        let last = d.project().clone();
        let n = undo.len();
        for _ in 0..n {
            prop_assert!(d.undo().unwrap());
            if let Err(why) = check(d.project()) {
                prop_assert!(false, "undo 뒤 불변식: {}", why);
            }
        }
        prop_assert_eq!(d.project(), &pool[0]);
        for _ in 0..n {
            prop_assert!(d.redo().unwrap());
        }
        prop_assert_eq!(d.project(), &last);
    }
}
