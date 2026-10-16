//! S2-T1 적대적 리뷰 1회차: `.ksq` 모르는 키 정렬 수정(`by_name`). 공개 API만 쓴다.
//!
//! 워크스페이스 빌드(`cargo test --workspace`)에서는 gpui-kit 때문에 `serde_json`의
//! `preserve_order`가 켜진다. 이때 `Map` 순회는 넣은 순서(파일 순서)다. 수정이 모든 쓰기 경로를
//! 덮지 않으면 같은 모델의 출력이 빌드 feature에 따라 달라진다.

use keyscore_core::ksq;

/// 모르는 키를 이름 역순으로 적은 파일. 모든 자리(최상위, 조성, 트랙, 포트, 클립, 노트, 중첩 객체)에 둔다.
const REVERSED: &str = r#"{
  "format_version": 1,
  "zz_top": 1,
  "aa_top": {"z": 1, "a": {"y": 2, "b": 3}},
  "key": {"tonic": 0, "mode": "major", "zz_key": 1, "aa_key": 2},
  "tempo": [[0, 500000]],
  "meter": [[0, 4, 4]],
  "tracks": [
    {
      "name": "T",
      "color": 0,
      "port": {"named": "IAC", "zz_port": 1, "mm_port": 3, "aa_port": 2},
      "channel": 1,
      "bank": null,
      "program": null,
      "default_velocity": 51492,
      "mute": false,
      "solo": false,
      "zz_track": 1,
      "aa_track": 2,
      "clips": [
        {
          "start": 0,
          "length": 960,
          "name": null,
          "color": null,
          "zz_clip": 1,
          "aa_clip": 2,
          "notes": [
            [0, 480, 60, 51492, {"lyric": "x", "zz_note": 1, "mm_note": 3, "aa_note": 2}]
          ]
        }
      ]
    }
  ]
}
"#;

fn assert_before(text: &str, first: &str, second: &str) {
    let a = text
        .find(first)
        .unwrap_or_else(|| panic!("{first} 없음\n{text}"));
    let b = text
        .find(second)
        .unwrap_or_else(|| panic!("{second} 없음\n{text}"));
    assert!(a < b, "{first}가 {second}보다 앞이어야 한다\n{text}");
}

/// 지금 수정이 고친 자리: 최상위·조성·트랙·클립·중첩 객체. 이름 순이다.
#[test]
fn unknown_keys_are_written_by_name_in_fixed_places() {
    let (project, _) = ksq::from_str(REVERSED).unwrap();
    let text = ksq::to_string(&project);
    assert_before(&text, "aa_top", "zz_top");
    assert_before(&text, "\"a\": {", "\"z\": 1");
    assert_before(&text, "\"b\": 3", "\"y\": 2");
    assert_before(&text, "aa_key", "zz_key");
    assert_before(&text, "aa_track", "zz_track");
    assert_before(&text, "aa_clip", "zz_clip");
}

/// 포트 객체의 모르는 키는 `t.port_extra().iter()`로 쓴다(`by_name`을 거치지 않음). `preserve_order`
/// 에서는 읽기의 `Map::remove`(`swap_remove`)가 순서를 섞어, 한 번 더 읽고 쓰면 순서가 뒤집힌다.
#[test]
fn unknown_port_keys_are_written_by_name() {
    let mut text = REVERSED.to_owned();
    for _ in 0..3 {
        let (project, _) = ksq::from_str(&text).unwrap();
        text = ksq::to_string(&project);
        assert_before(&text, "aa_port", "mm_port");
        assert_before(&text, "mm_port", "zz_port");
    }
}

/// 노트 속성 객체의 모르는 키는 `extra.iter()`로 쓴다(`by_name`을 거치지 않음). 읽을 때 정규화되어
/// 지금은 이름 순이다. 회귀 방지로 둔다.
#[test]
fn unknown_note_keys_are_written_by_name() {
    let mut text = REVERSED.to_owned();
    for _ in 0..3 {
        let (project, _) = ksq::from_str(&text).unwrap();
        text = ksq::to_string(&project);
        assert_before(&text, "aa_note", "mm_note");
        assert_before(&text, "mm_note", "zz_note");
    }
}

/// 쓰기 → 읽기 → 쓰기가 같은 바이트다(feature와 상관없이).
#[test]
fn reversed_unknown_keys_round_trip_byte_stable() {
    let (project, _) = ksq::from_str(REVERSED).unwrap();
    let first = ksq::to_string(&project);
    let (again, _) = ksq::from_str(&first).unwrap();
    assert_eq!(ksq::to_string(&again), first);
}
