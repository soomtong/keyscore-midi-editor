//! S3-T5 건반 뷰의 계산(REQ 5.4, 5.13, 2 「조성 표시」). 시험 목록은 `docs/reports/s3-t5/plan.md`.

use std::fmt::Write as _;

use keyscore_app::actions::args::KeyLayout;
use keyscore_app::keyboard_view::{KEY_COUNT, KeyCap, describe, keys};
use keyscore_core::{Key, Pitch, PitchClass};

fn labels(caps: &[KeyCap]) -> Vec<Option<&str>> {
    caps.iter().map(|c| c.key.as_deref()).collect()
}

// ---------- 1–2. 라벨 ----------

#[test]
fn labels_by_layout_and_octave_snapshot() {
    let mut text = String::new();
    for (layout, name, octave) in [
        (KeyLayout::Piano, "piano", 4),
        (KeyLayout::Piano, "piano", 3),
        (KeyLayout::Piano, "piano", 0),
        (KeyLayout::Solfege, "solfege", 4),
        (KeyLayout::Solfege, "solfege", 8),
    ] {
        let caps = keys(layout, octave, Key::None, false);
        assert_eq!(caps.len(), KEY_COUNT);
        writeln!(text, "== {name} oct{octave}").unwrap();
        text.push_str(&describe(&caps));
    }
    insta::assert_snapshot!(text);
}

#[test]
fn labels_at_octave_nine_mark_keys_above_127_absent() {
    for layout in [KeyLayout::Piano, KeyLayout::Solfege] {
        let caps = keys(layout, 9, Key::None, false);
        assert_eq!(caps.len(), KEY_COUNT);
        // C9 = 120 … G9 = 127
        for (i, cap) in (0u8..).zip(&caps) {
            if i <= 7 {
                assert_eq!(cap.pitch.map(Pitch::get), Some(120 + i), "{i}");
                assert!(cap.key.is_some(), "{layout:?} {i}");
            } else {
                assert_eq!(cap.pitch, None, "{i}");
                assert_eq!(cap.key, None, "{layout:?} {i}");
                assert_eq!(cap.name, None, "{i}");
            }
        }
    }
    // 옥타브 9에서 계이름형 둘째 옥타브(옥타브 10)는 없다
    let caps = keys(KeyLayout::Solfege, 9, Key::None, false);
    assert!(caps[12..].iter().all(|c| c.key.is_none()));
}

// ---------- 3. 드럼 ----------

#[test]
fn drum_track_labels_snapshot() {
    let mut text = String::new();
    for octave in [2, 3] {
        writeln!(text, "== drum piano oct{octave}").unwrap();
        text.push_str(&describe(&keys(KeyLayout::Piano, octave, Key::None, true)));
    }
    insta::assert_snapshot!(text);
    let caps = keys(KeyLayout::Piano, 2, Key::None, true);
    assert_eq!(caps[0].name.as_deref(), Some("bd"));
    assert_eq!(caps[2].name.as_deref(), Some("sd"));
    assert_eq!(caps[6].name.as_deref(), Some("ch"));
    // 40(E2, Electric Snare)은 짧은 라벨 표에 없다
    assert_eq!(caps[4].name, None);
}

// ---------- 4. 조성 ----------

#[test]
fn out_of_key_follows_project_key() {
    let d_major = Key::Major(PitchClass::new(2).unwrap());
    let caps = keys(KeyLayout::Piano, 4, d_major, false);
    // F#4 안, F4 밖, C#4 안, C4 밖
    assert!(!caps[6].out_of_key);
    assert!(caps[5].out_of_key);
    assert!(!caps[1].out_of_key);
    assert!(caps[0].out_of_key);
    let a_minor = Key::Minor(PitchClass::new(9).unwrap());
    let caps = keys(KeyLayout::Piano, 4, a_minor, false);
    assert!(!caps[0].out_of_key);
    assert!(caps[1].out_of_key);
    assert!(!caps[21].out_of_key, "A5");
    // 조성이 없으면 모두 안
    let caps = keys(KeyLayout::Piano, 4, Key::None, false);
    assert!(caps.iter().all(|c| !c.out_of_key));
}

// ---------- 5. 건반형 ----------

#[test]
fn piano_layout_has_sixteen_key_labels() {
    let caps = keys(KeyLayout::Piano, 4, Key::None, false);
    let expected: Vec<Option<&str>> = [
        "A", "W", "S", "E", "D", "F", "T", "G", "Y", "H", "U", "J", "K", "O", "L", "P",
    ]
    .into_iter()
    .map(Some)
    .chain(std::iter::repeat_n(None, 8))
    .collect();
    assert_eq!(labels(&caps), expected);
    let blacks: Vec<bool> = caps.iter().map(|c| c.black).collect();
    let octave = [
        false, true, false, true, false, false, true, false, true, false, true, false,
    ];
    assert_eq!(blacks, [octave, octave].concat());
}
