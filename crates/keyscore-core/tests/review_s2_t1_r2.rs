//! S2-T1 적대적 리뷰 2회차: `.ksq` 쓰기의 모든 `Map` 순회가 이름 순인가(리뷰 1 지적 1). 공개 API만 쓴다.
//!
//! `cargo test --workspace`에서는 gpui-kit 때문에 `serde_json`의 `preserve_order`가 켜진다. 리뷰 1이
//! 다루지 않은 자리(포트·노트·조성의 모르는 값 안에 든 중첩 객체와 배열 속 객체)를 역순으로 둔다.

use keyscore_core::ksq;

const NESTED: &str = r#"{
  "format_version": 1,
  "key": {"tonic": 2, "mode": "minor", "zz": [{"y": 1, "b": 2}], "aa": {"q": {"z": 0, "a": 1}}},
  "tempo": [[0, 500000]],
  "meter": [[0, 4, 4]],
  "zz_top": [[{"z": 1, "a": 2}]],
  "tracks": [
    {
      "name": "T",
      "color": 0,
      "port": {"named": "IAC", "zz": {"y": [1, {"d": 1, "c": 2}], "b": null}, "aa": []},
      "channel": 1,
      "bank": null,
      "program": null,
      "default_velocity": 51492,
      "mute": false,
      "solo": false,
      "clips": [
        {
          "start": 0,
          "length": 960,
          "name": null,
          "color": null,
          "notes": [
            [0, 480, 60, 51492, {"zz": {"y": 1, "b": {"d": 0, "c": 0}}, "lyric": "x", "aa": [{"z": 1, "a": 2}]}],
            [480, 480, 62, 51492, {"offset": 3, "mm": {}, "aa": {}}]
          ]
        }
      ]
    }
  ]
}
"#;

/// 같은 파일을 여러 번 쓰고 읽어도 바이트가 같고, 모든 깊이의 객체 키가 이름 순이다.
#[test]
fn nested_unknown_objects_everywhere_are_sorted_and_byte_stable() {
    let (project, warnings) = ksq::from_str(NESTED).unwrap();
    let first = ksq::to_string(&project);
    let mut text = first.clone();
    for _ in 0..3 {
        // 모델 동등 비교는 하지 않는다. TrackId·ClipId는 읽을 때마다 새로 매긴다.
        let (p, _) = ksq::from_str(&text).unwrap();
        text = ksq::to_string(&p);
        assert_eq!(text, first, "쓰기가 결정적이지 않다");
    }
    for (a, b) in [
        ("\"b\": 2", "\"y\": 1"),
        ("\"a\": 1", "\"z\": 0"),
        ("\"aa\": []", "\"zz\": {"),
        ("\"b\": null", "\"y\": ["),
        ("\"c\": 2", "\"d\": 1"),
        ("\"aa\": [{\"a\": 2", "\"zz\": {\"b\": {\"c\": 0"),
        ("\"aa\": {}", "\"mm\": {}"),
    ] {
        let ia = first.find(a).unwrap_or_else(|| panic!("{a} 없음\n{first}"));
        let ib = first.find(b).unwrap_or_else(|| panic!("{b} 없음\n{first}"));
        assert!(ia < ib, "{a}가 {b}보다 앞이어야 한다\n{first}");
    }
    assert!(warnings.is_empty(), "{warnings:?}");
}
