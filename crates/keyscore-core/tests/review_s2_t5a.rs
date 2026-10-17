//! S2-T5a 적대적 리뷰 1: 결정 B(포트·조성 값이 바뀌면 모르는 키를 지운다)를 깨뜨리려는 시도.
//! 여기 남은 테스트는 깨뜨리지 못한 시도다.

use keyscore_core::{
    CoalesceKey, Document, Edit, Key, OutputPort, PitchClass, TrackId, TrackProp, ksq,
};

const TEXT: &str = r#"{"key": {"tonic": 2, "mode": "major", "x": 1}, "tracks": [{"port": {"named": "A", "uid": 7}}]}"#;

fn doc() -> (Document, TrackId, String) {
    let (p, _) = ksq::from_str(TEXT).unwrap();
    let id = p.tracks()[0].id();
    let doc = Document::new(p, 1 << 20).unwrap();
    let text = ksq::to_string(doc.project());
    (doc, id, text)
}

fn port(id: TrackId, port: OutputPort) -> Edit {
    Edit::SetTrackProp {
        track: id,
        prop: TrackProp::Port(port),
    }
}

fn named(name: &str) -> OutputPort {
    OutputPort::Named(name.to_owned())
}

fn d_major() -> Key {
    Key::Major(PitchClass::new(2).unwrap())
}

#[test]
fn port_round_trip_in_one_unit_clears_and_undo_restores() {
    // 한 undo 단위 안에서 A → B → A: 값은 처음과 같지만 중간에 바뀌었으므로 모르는 키는 지워진다.
    let (mut doc, id, original) = doc();
    doc.apply(vec![port(id, named("B")), port(id, named("A"))], None)
        .unwrap();
    let after = ksq::to_string(doc.project());
    assert!(!after.contains("uid"), "{after}");
    doc.undo().unwrap();
    assert_eq!(ksq::to_string(doc.project()), original);
    doc.redo().unwrap();
    assert_eq!(ksq::to_string(doc.project()), after);
}

#[test]
fn coalesced_port_edits_undo_to_original_unknown_keys() {
    // 같은 coalesce 키로 묶인 연속 편집(인스펙터 드롭다운 등)도 undo 한 번에 원래 모르는 키로 돌아온다.
    let (mut doc, id, original) = doc();
    let key = Some(CoalesceKey(9));
    doc.apply(vec![port(id, named("B"))], key).unwrap();
    doc.apply(vec![port(id, OutputPort::Virtual)], key).unwrap();
    doc.apply(vec![port(id, named("A"))], key).unwrap();
    assert_eq!(doc.undo_len(), 1);
    assert!(!ksq::to_string(doc.project()).contains("uid"));
    doc.undo().unwrap();
    assert_eq!(ksq::to_string(doc.project()), original);
}

#[test]
fn failed_batch_keeps_unknown_keys() {
    // 묶음의 뒤 편집이 실패하면 앞의 포트·조성 편집이 지운 모르는 키도 되돌아와야 한다.
    let (mut doc, id, original) = doc();
    let (other, _) = ksq::from_str(r#"{"tracks": [{}]}"#).unwrap();
    let ghost = other.tracks()[0].id();
    let result = doc.apply(
        vec![
            port(id, named("B")),
            Edit::SetKey(Key::None),
            port(ghost, OutputPort::Virtual),
        ],
        None,
    );
    assert!(result.is_err());
    assert_eq!(ksq::to_string(doc.project()), original);
    assert_eq!(doc.undo_len(), 0);
}

#[test]
fn key_none_round_trip_with_undo_redo() {
    let (mut doc, _, original) = doc();
    doc.apply(vec![Edit::SetKey(Key::None)], None).unwrap();
    doc.apply(vec![Edit::SetKey(d_major())], None).unwrap();
    let end = ksq::to_string(doc.project());
    assert!(!end.contains("\"x\""), "{end}");
    doc.undo().unwrap();
    doc.undo().unwrap();
    assert_eq!(ksq::to_string(doc.project()), original);
    doc.redo().unwrap();
    doc.redo().unwrap();
    assert_eq!(ksq::to_string(doc.project()), end);
}

#[test]
fn same_value_edit_then_undo_redo_keeps_unknown_keys() {
    let (mut doc, id, original) = doc();
    doc.apply(vec![port(id, named("A")), Edit::SetKey(d_major())], None)
        .unwrap();
    assert_eq!(ksq::to_string(doc.project()), original);
    doc.undo().unwrap();
    assert_eq!(ksq::to_string(doc.project()), original);
    doc.redo().unwrap();
    assert_eq!(ksq::to_string(doc.project()), original);
}

#[test]
fn history_bytes_count_large_unknown_keys() {
    // 큰 모르는 키는 히스토리 상한 계산에 들어가야 한다(메모리 추정).
    let blob = "z".repeat(100_000);
    let text = format!(r#"{{"tracks": [{{"port": {{"named": "A", "blob": "{blob}"}}}}]}}"#);
    let (p, _) = ksq::from_str(&text).unwrap();
    let id = p.tracks()[0].id();
    let mut doc = Document::new(p, 1 << 20).unwrap();
    doc.apply(vec![port(id, named("B"))], None).unwrap();
    assert!(doc.history_bytes() >= 100_000, "{}", doc.history_bytes());
    // undo 뒤 redo 쪽은 빈 모르는 키라 작아진다
    doc.undo().unwrap();
    assert!(doc.history_bytes() < 1_000, "{}", doc.history_bytes());
}

#[test]
fn history_limit_evicts_units_with_large_unknown_keys() {
    let blob = "z".repeat(40_000);
    let mut tracks = Vec::new();
    for i in 0..4 {
        tracks.push(format!(
            r#"{{"channel": {}, "port": {{"named": "P{i}", "blob": "{blob}"}}}}"#,
            i + 1
        ));
    }
    let text = format!(r#"{{"tracks": [{}]}}"#, tracks.join(","));
    let (p, _) = ksq::from_str(&text).unwrap();
    let ids: Vec<_> = p.tracks().iter().map(keyscore_core::Track::id).collect();
    let mut doc = Document::new(p, 100_000).unwrap();
    for id in ids {
        doc.apply(vec![port(id, OutputPort::Virtual)], None)
            .unwrap();
    }
    assert!(doc.history_bytes() <= 100_000, "{}", doc.history_bytes());
    assert!(doc.undo_len() < 4);
}
