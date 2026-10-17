//! 포트·조성 값이 바뀔 때 `.ksq` 모르는 키(결정 `docs/decisions/2026-10-12-ksq-unknown-keys-on-value-change.md` B).
//!
//! 값이 바뀌면 그 객체의 모르는 키를 지운다. 같은 값이면 두고, undo는 값과 모르는 키를 함께 되살린다.

use keyscore_core::{Document, Edit, Key, OutputPort, PitchClass, TrackProp, ksq};
use proptest::prelude::*;

const PORT_TEXT: &str = r#"{"tracks": [{"port": {"named": "A", "uid": 7}}]}"#;
const KEY_TEXT: &str = r#"{"key": {"tonic": 2, "mode": "major", "accidentals": "sharps"}}"#;

fn port_doc() -> (Document, keyscore_core::TrackId) {
    let (p, _) = ksq::from_str(PORT_TEXT).unwrap();
    let id = p.tracks()[0].id();
    (Document::new(p, 1 << 20).unwrap(), id)
}

fn set_port(doc: &mut Document, id: keyscore_core::TrackId, port: OutputPort) {
    doc.apply(
        vec![Edit::SetTrackProp {
            track: id,
            prop: TrackProp::Port(port),
        }],
        None,
    )
    .unwrap();
}

fn named(name: &str) -> OutputPort {
    OutputPort::Named(name.to_string())
}

#[test]
fn port_change_clears_unknown_keys_and_undo_restores_them() {
    let (mut doc, id) = port_doc();
    let original = ksq::to_string(doc.project());
    assert!(original.contains(r#""uid": 7"#), "{original}");

    set_port(&mut doc, id, named("B"));
    let changed = ksq::to_string(doc.project());
    assert!(changed.contains(r#""named": "B""#), "{changed}");
    assert!(
        !changed.contains("uid"),
        "포트 A의 uid가 B에 붙었다: {changed}"
    );

    doc.undo().unwrap();
    assert_eq!(
        ksq::to_string(doc.project()),
        original,
        "undo가 값과 모르는 키를 되살린다"
    );
    doc.redo().unwrap();
    assert_eq!(ksq::to_string(doc.project()), changed, "redo는 다시 지운다");
}

#[test]
fn same_port_value_keeps_unknown_keys() {
    let (mut doc, id) = port_doc();
    let original = ksq::to_string(doc.project());
    set_port(&mut doc, id, named("A"));
    assert_eq!(ksq::to_string(doc.project()), original);
}

#[test]
fn unknown_keys_do_not_come_back_through_another_value() {
    let (mut doc, id) = port_doc();
    set_port(&mut doc, id, OutputPort::Virtual);
    set_port(&mut doc, id, named("A"));
    let out = ksq::to_string(doc.project());
    assert!(
        !out.contains("uid"),
        "가상을 거쳐 A로 돌아오자 옛 uid가 되살아났다: {out}"
    );
    // undo 두 번이면 처음 그대로
    doc.undo().unwrap();
    doc.undo().unwrap();
    let original = ksq::to_string(&ksq::from_str(PORT_TEXT).unwrap().0);
    assert_eq!(ksq::to_string(doc.project()), original);
}

#[test]
fn key_change_clears_unknown_keys_and_undo_restores_them() {
    let (p, _) = ksq::from_str(KEY_TEXT).unwrap();
    let mut doc = Document::new(p, 1 << 20).unwrap();
    let original = ksq::to_string(doc.project());
    assert!(original.contains("accidentals"), "{original}");

    // 같은 값은 그대로
    doc.apply(
        vec![Edit::SetKey(Key::Major(PitchClass::new(2).unwrap()))],
        None,
    )
    .unwrap();
    assert_eq!(ksq::to_string(doc.project()), original);

    doc.apply(
        vec![Edit::SetKey(Key::Minor(PitchClass::new(9).unwrap()))],
        None,
    )
    .unwrap();
    let changed = ksq::to_string(doc.project());
    assert!(!changed.contains("accidentals"), "{changed}");
    doc.undo().unwrap();
    assert_eq!(ksq::to_string(doc.project()), original);
    doc.redo().unwrap();
    assert_eq!(ksq::to_string(doc.project()), changed);

    // 없음을 거쳐 원래 조성으로 돌아와도 되살아나지 않는다
    doc.apply(vec![Edit::SetKey(Key::None)], None).unwrap();
    doc.apply(
        vec![Edit::SetKey(Key::Major(PitchClass::new(2).unwrap()))],
        None,
    )
    .unwrap();
    assert!(!ksq::to_string(doc.project()).contains("accidentals"));
}

fn port_strategy() -> impl Strategy<Value = OutputPort> {
    prop_oneof![
        Just(OutputPort::Virtual),
        Just(named("A")),
        Just(named("B")),
    ]
}

fn key_strategy() -> impl Strategy<Value = Key> {
    prop_oneof![
        Just(Key::None),
        (0u8..12).prop_map(|t| Key::Major(PitchClass::new(t).unwrap())),
        (0u8..12).prop_map(|t| Key::Minor(PitchClass::new(t).unwrap())),
    ]
}

#[derive(Debug, Clone)]
enum Step {
    Port(OutputPort),
    Key(Key),
}

proptest! {
    /// 포트·조성 편집을 무작위로 섞은 뒤 전부 undo하면 처음 `.ksq`와 같고, 전부 redo하면 끝 상태와 같다.
    #[test]
    fn unknown_key_edits_keep_history_invariants(
        steps in proptest::collection::vec(
            prop_oneof![port_strategy().prop_map(Step::Port), key_strategy().prop_map(Step::Key)],
            1..20,
        )
    ) {
        let text = r#"{"key": {"tonic": 2, "mode": "major", "x": 1}, "tracks": [{"port": {"named": "A", "uid": 7}}]}"#;
        let (p, _) = ksq::from_str(text).unwrap();
        let id = p.tracks()[0].id();
        let mut doc = Document::new(p, 1 << 20).unwrap();
        let start = ksq::to_string(doc.project());
        let mut applied = 0;
        for step in steps {
            let edit = match step {
                Step::Port(port) => Edit::SetTrackProp { track: id, prop: TrackProp::Port(port) },
                Step::Key(key) => Edit::SetKey(key),
            };
            doc.apply(vec![edit], None).unwrap();
            applied += 1;
        }
        let end = ksq::to_string(doc.project());
        for _ in 0..applied {
            prop_assert!(doc.undo().unwrap());
        }
        prop_assert_eq!(ksq::to_string(doc.project()), start);
        for _ in 0..applied {
            prop_assert!(doc.redo().unwrap());
        }
        prop_assert_eq!(ksq::to_string(doc.project()), end);
    }
}
