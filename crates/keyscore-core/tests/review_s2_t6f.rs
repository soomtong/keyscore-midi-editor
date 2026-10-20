//! S2-T6f 적대적 리뷰 1회차: 버린 undo 단위 수가 apply·undo·redo의 상한 적용 경로마다 정확한가.

use keyscore_core::{Channel, Document, Edit, Project, Track, TrackColor, TrackId, TrackProp};

fn doc(limit: usize) -> (Document, TrackId) {
    let mut project = Project::new();
    let track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let id = track.id();
    project.push_track(track).unwrap();
    (Document::new(project, limit).unwrap(), id)
}

fn rename(doc: &mut Document, id: TrackId, name: String) {
    doc.apply(
        vec![Edit::SetTrackProp {
            track: id,
            prop: TrackProp::Name(name),
        }],
        None,
    )
    .unwrap();
}

/// 넣은 단위 = 남은 undo + 남은 redo + 버린 단위. 되돌리기(redo 쪽이 커짐)와 다시 실행에서 버린 것도 정확히
/// 센다.
#[test]
fn dropped_count_is_exact_on_undo_and_redo_paths() {
    let (mut d, id) = doc(1500);
    let mut applied = 0_u64;
    for i in 0..4 {
        rename(&mut d, id, format!("n{i}"));
        applied += 1;
    }
    rename(&mut d, id, "x".repeat(3000));
    applied += 1;
    assert_eq!(d.dropped_undo_units(), 0);
    let total = |d: &Document| d.dropped_undo_units() + d.undo_len() as u64 + d.redo_len() as u64;
    assert!(d.undo().unwrap());
    assert!(d.dropped_undo_units() > 0, "되돌리기로 버리지 않았다");
    assert_eq!(total(&d), applied, "되돌리기 뒤");
    while d.undo().unwrap() {
        assert_eq!(total(&d), applied, "되돌리기 반복 중");
    }
    while d.redo().unwrap() {
        assert_eq!(total(&d), applied, "다시 실행 중");
    }
    // 코얼레싱으로 묶인 단위가 커져 버릴 때도 센다(새 단위는 늘지 않는다)
    let key = Some(keyscore_core::CoalesceKey(7));
    let before = d.dropped_undo_units() + d.undo_len() as u64;
    for i in 0..5 {
        d.apply(
            vec![Edit::SetTrackProp {
                track: id,
                prop: TrackProp::Name(format!("{i:0>400}")),
            }],
            key,
        )
        .unwrap();
    }
    assert_eq!(
        d.dropped_undo_units() + d.undo_len() as u64,
        before + 1,
        "묶인 다섯 편집은 한 단위다"
    );
}
