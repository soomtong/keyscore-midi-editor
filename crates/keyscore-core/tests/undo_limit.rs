//! S2-T6f: 메모리 상한 때문에 버린 undo 단위 수(결정 2026-10-19-undo-history-memory-limit A, EDT-1).

use keyscore_core::{Channel, Document, Edit, Project, Track, TrackColor, TrackProp};

fn doc(limit: usize) -> (Document, keyscore_core::TrackId) {
    let track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let id = track.id();
    let mut doc = Document::new(Project::new(), limit).unwrap();
    doc.apply(
        vec![Edit::InsertTrack {
            index: 0,
            track: Box::new(track),
        }],
        None,
    )
    .unwrap();
    (doc, id)
}

fn rename(doc: &mut Document, id: keyscore_core::TrackId, i: usize) {
    doc.apply(
        vec![Edit::SetTrackProp {
            track: id,
            prop: TrackProp::Name(format!("{i:0>200}")),
        }],
        None,
    )
    .unwrap();
}

#[test]
fn document_counts_dropped_undo_units() {
    // 넉넉한 상한: 버리지 않는다
    let (mut big, id) = doc(1 << 20);
    for i in 0..20 {
        rename(&mut big, id, i);
    }
    assert_eq!(big.dropped_undo_units(), 0);

    // 작은 상한: 넣은 단위 = 남은 단위 + 버린 단위
    let (mut small, id) = doc(2000);
    let mut applied = 1; // 트랙 넣기
    for i in 0..40 {
        rename(&mut small, id, i);
        applied += 1;
        assert_eq!(
            small.dropped_undo_units() + small.undo_len() as u64,
            applied,
            "{i}번째 편집 뒤"
        );
    }
    assert!(small.dropped_undo_units() > 0);

    // 되돌리기·다시 실행으로 버려도 센다
    let before = small.dropped_undo_units();
    while small.undo().unwrap() {}
    let undone = small.redo_len();
    while small.redo().unwrap() {}
    assert!(small.dropped_undo_units() >= before);
    assert_eq!(
        small.dropped_undo_units() + small.undo_len() as u64,
        applied,
        "되돌린 {undone}단위를 다시 한 뒤"
    );
}
