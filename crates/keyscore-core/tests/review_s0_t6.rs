//! S0-T6 적대적 리뷰 1회차의 깨뜨리기 테스트. 공개 API만 쓴다.
//!
//! 실패하는 테스트는 지적 사항의 재현이다. 고친 뒤 통과해야 한다.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use keyscore_core::ksq::{self, Compression, KsqError};
use proptest::prelude::*;
use serde_json::Value;

/// 읽기가 받아들인 입력이면, 쓰기 → 읽기 → 쓰기가 같은 바이트여야 한다(계획 「불변식」).
fn assert_stable(text: &str) -> Result<(), TestCaseError> {
    if let Ok((p, _)) = ksq::from_str(text) {
        let first = ksq::to_string(&p);
        let (back, _) = ksq::from_str(&first)
            .map_err(|e| TestCaseError::fail(format!("다시 읽기 실패: {e}\n{first}")))?;
        let second = ksq::to_string(&back);
        prop_assert_eq!(&second, &first, "입력:\n{}", text);
    }
    Ok(())
}

// ---------------------------------------------------------------- 모르는 키 보존(IO-1)

/// `key` 객체 안의 모르는 키도 「모르는 키」다. 지금은 읽을 때 버려진다.
#[test]
fn ksq_unknown_key_inside_key_object_survives_read_write() {
    let text = r#"{"key": {"tonic": 2, "mode": "major", "accidentals": "sharps"}}"#;
    let (p, _) = ksq::from_str(text).unwrap();
    let out = ksq::to_string(&p);
    assert!(
        out.contains(r#""accidentals": "sharps""#),
        "key 객체의 모르는 키가 사라졌다:\n{out}"
    );
}

/// `port` 객체 안의 모르는 키도 보존되어야 한다. 지금은 읽을 때 버려진다.
#[test]
fn ksq_unknown_key_inside_port_object_survives_read_write() {
    let text = r#"{"tracks": [{"port": {"named": "IAC Bus 1", "uid": 12345}}]}"#;
    let (p, _) = ksq::from_str(text).unwrap();
    let out = ksq::to_string(&p);
    assert!(
        out.contains(r#""uid": 12345"#),
        "port 객체의 모르는 키가 사라졌다:\n{out}"
    );
}

// ---------------------------------------------------------------- 왕복 바이트 동일

/// 모르는 키에 담긴 실수. `serde_json` 기본 파서는 실수를 최선 노력 정밀도로 읽으므로
/// (`float_roundtrip` 기능 꺼짐) 쓴 문자열을 다시 읽으면 1 ULP 다른 값이 될 수 있다.
#[test]
fn ksq_unknown_float_write_read_write_is_identical_for_known_hard_cases() {
    // 아래 proptest가 찾은 사례(앞의 셋)와 손으로 고른 경계값.
    for f in [
        -1.883_878_196_413_544_7e-181_f64,
        -1.424_430_446_650_658_7e-59,
        -4.704_460_693_137_356_4e-172,
        0.1,
        -0.0,
        1e300,
        5e-324,
        f64::MAX,
        f64::MIN_POSITIVE,
        2.225_073_858_507_201e-308,
        9_007_199_254_740_993.0,
    ] {
        let text = format!(r#"{{"x": {}}}"#, Value::from(f));
        assert_stable(&text).unwrap();
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    /// 임의 비트 패턴의 유한 실수를 모르는 키에 둔다.
    #[test]
    fn ksq_unknown_float_write_read_write_is_identical(bits in any::<u64>()) {
        let f = f64::from_bits(bits);
        prop_assume!(f.is_finite());
        let text = format!(r#"{{"x": {}, "tracks": [{{"clips": [{{"notes": [[0, 1, 60, 1, {{"y": {}}}]]}}]}}]}}"#,
            Value::from(f), Value::from(f));
        assert_stable(&text)?;
    }
}

// ---------------------------------------------------------------- 퍼즈: 임의 JSON

fn json_leaf() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<u64>().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        (0u64..200).prop_map(Value::from),
        // 실수는 지적 3(실수 왕복)이 따로 다룬다. 여기서 빼야 다른 결함이 가려지지 않는다.
        Just(Value::from(1.5)),
        Just(Value::from(-0.0)),
        Just(Value::from(1e300)),
        ".{0,6}".prop_map(Value::String),
        prop_oneof![
            Just("none"),
            Just("virtual"),
            Just("major"),
            Just(""),
            Just("-")
        ]
        .prop_map(|s| Value::String(s.to_owned())),
    ]
}

fn json_any() -> impl Strategy<Value = Value> {
    let keys = prop_oneof![
        Just("format_version"),
        Just("key"),
        Just("tempo"),
        Just("meter"),
        Just("tracks"),
        Just("name"),
        Just("color"),
        Just("port"),
        Just("named"),
        Just("channel"),
        Just("bank"),
        Just("program"),
        Just("default_velocity"),
        Just("mute"),
        Just("solo"),
        Just("lyric_lang"),
        Just("clips"),
        Just("start"),
        Just("length"),
        Just("notes"),
        Just("offset"),
        Just("lyric"),
        Just("tonic"),
        Just("mode"),
        Just("zz"),
        Just("a"),
    ];
    json_leaf().prop_recursive(5, 64, 6, move |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::vec((keys.clone(), inner), 0..6).prop_map(|kv| {
                Value::Object(kv.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
            }),
        ]
    })
}

/// 거의 유효한 파일: 구조는 맞추고 값만 임의로 둔다.
fn near_valid() -> impl Strategy<Value = String> {
    let note = (
        0u64..2000,
        1u64..2000,
        0u64..130,
        0u64..70000,
        prop::option::of(json_any()),
    )
        .prop_map(|(d, l, p, v, extra)| {
            let mut a = vec![
                Value::from(d),
                Value::from(l),
                Value::from(p),
                Value::from(v),
            ];
            if let Some(e) = extra {
                a.push(e);
            }
            Value::Array(a)
        });
    let clip = (
        prop::option::of(0u64..100_000),
        prop::option::of(1u64..100_000),
        prop::collection::vec(note, 0..6),
        prop::option::of(json_any()),
    )
        .prop_map(|(s, l, notes, extra)| {
            let mut m = serde_json::Map::new();
            if let Some(s) = s {
                m.insert("start".into(), s.into());
            }
            if let Some(l) = l {
                m.insert("length".into(), l.into());
            }
            m.insert("notes".into(), Value::Array(notes));
            if let Some(e) = extra {
                m.insert("zz".into(), e);
            }
            Value::Object(m)
        });
    let track = (
        prop::collection::vec(clip, 0..3),
        prop::option::of(json_any()),
        prop::option::of(json_any()),
    )
        .prop_map(|(clips, port, extra)| {
            let mut m = serde_json::Map::new();
            m.insert("clips".into(), Value::Array(clips));
            if let Some(p) = port {
                m.insert("port".into(), p);
            }
            if let Some(e) = extra {
                m.insert("a".into(), e);
            }
            Value::Object(m)
        });
    (
        prop::collection::vec(track, 0..3),
        prop::option::of(json_any()),
        prop::option::of(json_any()),
    )
        .prop_map(|(tracks, key, extra)| {
            let mut m = serde_json::Map::new();
            m.insert("tracks".into(), Value::Array(tracks));
            if let Some(k) = key {
                m.insert("key".into(), k);
            }
            if let Some(e) = extra {
                m.insert("zz".into(), e);
            }
            Value::Object(m).to_string()
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    /// 임의 JSON: 패닉하지 않고, 받아들였으면 왕복이 같은 바이트다.
    #[test]
    fn ksq_fuzz_any_json_never_panics_and_is_stable(v in json_any()) {
        assert_stable(&v.to_string())?;
        assert_stable(&serde_json::to_string_pretty(&v).unwrap())?;
    }

    #[test]
    fn ksq_fuzz_near_valid_files_never_panic_and_are_stable(text in near_valid()) {
        assert_stable(&text)?;
    }

    /// 유효한 파일의 바이트 하나를 바꾸거나 지운다.
    #[test]
    fn ksq_fuzz_byte_mutation_never_panics(pos in any::<prop::sample::Index>(), byte in any::<u8>(), delete in any::<bool>()) {
        let base = r#"{
  "format_version": 1,
  "key": {"tonic": 9, "mode": "minor"},
  "tempo": [
    [0, 500000],
    [7680, 666667]
  ],
  "meter": [
    [0, 4, 4],
    [7680, 6, 8]
  ],
  "tracks": [
    {
      "name": "Lead",
      "color": 3,
      "port": {"named": "IAC"},
      "channel": 1,
      "bank": 5,
      "program": 73,
      "default_velocity": 51492,
      "mute": false,
      "solo": false,
      "lyric_lang": "ko",
      "clips": [
        {
          "start": 0,
          "length": 960,
          "name": "a",
          "color": 2,
          "notes": [
            [0, 480, 60, 51492],
            [480, 960, 64, 51492, {"offset": 16777216, "lyric": "사", "zz": [1]}]
          ]
        }
      ]
    }
  ]
}"#;
        let mut bytes = base.as_bytes().to_vec();
        let i = pos.index(bytes.len());
        if delete { bytes.remove(i); } else { bytes[i] = byte; }
        if let Ok(text) = String::from_utf8(bytes.clone()) {
            assert_stable(&text)?;
        } else {
            prop_assert!(ksq::from_bytes(&bytes).is_err());
        }
    }
}

// ---------------------------------------------------------------- 경계·악성 입력

#[test]
fn ksq_deep_nesting_in_unknown_key_is_error_not_crash() {
    for depth in [127usize, 128, 129, 10_000, 1_000_000] {
        let text = format!(r#"{{"zz": {}{}}}"#, "[".repeat(depth), "]".repeat(depth));
        // 스택 오버플로로 죽지 않으면 된다. 깊이 제한 안이면 왕복도 같아야 한다.
        assert_stable(&text).unwrap();
    }
}

#[test]
fn ksq_duplicate_keys_resolve_to_one_and_are_stable() {
    for text in [
        r#"{"tracks": [{"name": "a"}], "tracks": []}"#,
        r#"{"tracks": [{"name": "a", "name": "b", "zz": 1, "zz": 2}]}"#,
        r#"{"tracks": [{"clips": [{"notes": [[0, 1, 60, 1, {"lyric": "a", "lyric": "", "offset": 1, "offset": 0}]]}]}]}"#,
        r#"{"format_version": 9, "format_version": 1}"#,
    ] {
        assert_stable(text).unwrap();
    }
}

#[test]
fn ksq_numbers_at_the_edges_never_panic() {
    let max = u64::MAX;
    let cases = [
        format!(r#"{{"tracks": [{{"clips": [{{"notes": [[{max}, 1, 60, 1]]}}]}}]}}"#),
        format!(r#"{{"tracks": [{{"clips": [{{"start": {max}, "length": 1}}]}}]}}"#),
        format!(r#"{{"tracks": [{{"clips": [{{"start": 0, "length": {max}}}]}}]}}"#),
        format!(
            r#"{{"tracks": [{{"clips": [{{"start": 10, "length": {}, "notes": [[{}, 4294967295, 60, 1]]}}]}}]}}"#,
            max - 10,
            max - 20
        ),
        format!(r#"{{"tracks": [{{"clips": [{{"notes": [[{}, 4294967295, 60, 1]]}}]}}]}}"#, max - 1),
        format!(r#"{{"tracks": [{{"clips": [{{"notes": [[0, 1, 60, 1], [{max}, 1, 60, 1]]}}]}}]}}"#),
        r#"{"tracks": [{"clips": [{"notes": [[0, 4294967296, 60, 1]]}]}]}"#.to_owned(),
        r#"{"tracks": [{"clips": [{"notes": [[0, 1, 60, 65536]]}]}]}"#.to_owned(),
        r#"{"tracks": [{"clips": [{"notes": [[0, 1, 60, 1, {"offset": -2147483648}]]}]}]}"#.to_owned(),
        r#"{"tracks": [{"clips": [{"notes": [[0, 1, 60, 1, {"offset": 9223372036854775807}]]}]}]}"#.to_owned(),
        r#"{"tracks": [{"clips": [{"notes": [[0, 1, 0, 1, {"offset": -1}]]}]}]}"#.to_owned(),
        r#"{"tracks": [{"clips": [{"notes": [[0, 1, 127, 1, {"offset": 33554431}]]}]}]}"#.to_owned(),
        r#"{"tracks": [{"clips": [{"notes": [[1.5, 1, 60, 1]]}]}]}"#.to_owned(),
        r#"{"tracks": [{"clips": [{"notes": [[1e3, 1, 60, 1]]}]}]}"#.to_owned(),
        r#"{"tracks": [{"clips": [{"notes": [[-0, 1, 60, 1]]}]}]}"#.to_owned(),
        r#"{"format_version": 1.5}"#.to_owned(),
        r#"{"format_version": -1}"#.to_owned(),
        r#"{"format_version": 18446744073709551616}"#.to_owned(),
        r#"{"tempo": [[0, 16777216]]}"#.to_owned(),
        r#"{"tempo": [[0, 1], [18446744073709551615, 16777215]]}"#.to_owned(),
        r#"{"meter": [[0, 1, 256], [18446744073709551615, 255, 1]]}"#.to_owned(),
        r#"{"tempo": [[0, 500000], [0, 400000]]}"#.to_owned(),
        r#"{"meter": [[5, 4, 4]]}"#.to_owned(),
        r#"{"zz": -0, "zy": 1e3, "zx": 18446744073709551616, "zw": -9223372036854775809}"#
            .to_owned(),
        "{\"tracks\": [{\"name\": \"\\u0000\\u2028\\ud83c\\udfb5\\\"\\\\\", \"lyric_lang\": \"\"}]}"
            .to_owned(),
        r#"{"tracks": [{"clips": [{"notes": [[0, 1, 60, 1, {"lyric": ""}], [0, 1, 60, 1, {}]]}]}]}"#
            .to_owned(),
    ];
    for text in &cases {
        assert_stable(text).unwrap();
    }
}

#[test]
fn ksq_lone_surrogate_is_error() {
    assert!(matches!(
        ksq::from_str(r#"{"zz": "\ud800"}"#),
        Err(KsqError::Json { .. })
    ));
}

// ---------------------------------------------------------------- 성능(악성·대형 파일)

/// 템포 변화가 많은 파일. 앱이 쓴 파일이어도 읽기가 변화 수의 제곱에 비례하면 안 된다.
/// `TempoMap::set`이 매번 전체를 다시 계산해서, 읽기가 항목마다 `set`을 부르면 O(n²)이다.
#[test]
fn ksq_many_tempo_changes_read_in_linear_time() {
    let read_time = |n: u64| {
        let mut text = String::from(r#"{"tempo": ["#);
        for i in 0..n {
            if i > 0 {
                text.push_str(", ");
            }
            let _ = write!(text, "[{}, {}]", i * 10, 400_000 + i % 1000);
        }
        text.push_str("]}");
        let started = Instant::now();
        ksq::from_str(&text).unwrap();
        started.elapsed()
    };
    let small = read_time(2_500);
    let large = read_time(20_000);
    // 선형이면 8배 근처, 제곱이면 64배 근처다. 측정 잡음을 고려해 24배를 경계로 둔다.
    let ratio = large.as_secs_f64() / small.as_secs_f64().max(1e-6);
    assert!(
        ratio < 24.0 || large < Duration::from_millis(50),
        "템포 2,500개 {small:?}, 2만 개 {large:?}: {ratio:.1}배"
    );
}

/// 같은 시작·음높이 노트를 길이 내림차순으로 둔 파일. 노트마다 앞에 끼워 넣으면 O(n²)이다.
#[test]
fn ksq_reverse_ordered_notes_read_in_linear_time() {
    let read_time = |n: u32| {
        let mut text = String::from(r#"{"tracks": [{"clips": [{"notes": ["#);
        for i in 0..n {
            if i > 0 {
                text.push_str(", ");
            }
            let _ = write!(text, "[0, {}, 60, 1]", n - i);
        }
        text.push_str("]}]}]}");
        let started = Instant::now();
        ksq::from_str(&text).unwrap();
        started.elapsed()
    };
    let small = read_time(5_000);
    let large = read_time(40_000);
    let ratio = large.as_secs_f64() / small.as_secs_f64().max(1e-6);
    assert!(
        ratio < 24.0 || large < Duration::from_millis(50),
        "노트 5천 개 {small:?}, 4만 개 {large:?}: {ratio:.1}배"
    );
}

// ---------------------------------------------------------------- 파일 쓰기

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::path::Path::new("/tmp").join(format!(
            "keyscore-review-s0-t6-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 같은 경로에 여러 스레드가 동시에 쓴다. 결과는 어느 한 쓰기의 온전한 내용이어야 하고 임시 파일이
/// 남지 않아야 한다.
#[test]
fn ksq_concurrent_writes_to_same_path_leave_one_whole_file() {
    let dir = TempDir::new("concurrent");
    let path = dir.0.join("song.ksq");
    let texts: Vec<String> = (0..8)
        .map(|i| format!(r#"{{"tracks": [{{"name": "t{i}"}}]}}"#))
        .collect();
    let projects: Vec<_> = texts.iter().map(|t| ksq::from_str(t).unwrap().0).collect();
    std::thread::scope(|s| {
        for p in &projects {
            let path = &path;
            s.spawn(move || {
                for _ in 0..20 {
                    ksq::write_file(path, p, Compression::None).unwrap();
                }
            });
        }
    });
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(
        projects.iter().any(|p| ksq::to_string(p) == written),
        "온전한 내용이 아니다:\n{written}"
    );
    let names: Vec<_> = std::fs::read_dir(&dir.0)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");
}

/// `..`로 끝나는 경로(파일 이름이 없는 경로).
#[test]
fn ksq_write_file_odd_paths_do_not_panic() {
    let dir = TempDir::new("odd");
    let (p, _) = ksq::from_str("{}").unwrap();
    let sub = dir.0.join("sub");
    std::fs::create_dir(&sub).unwrap();
    // `sub/..`은 디렉터리라 실패해야 하고, 임시 파일이 남으면 안 된다.
    assert!(ksq::write_file(&sub.join(".."), &p, Compression::None).is_err());
    assert!(ksq::write_file(std::path::Path::new("/"), &p, Compression::None).is_err());
    let left: Vec<_> = std::fs::read_dir(&dir.0)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(left, vec![std::ffi::OsString::from("sub")]);
}

/// 심볼릭 링크로 연 파일에 저장하면 링크가 가리키는 파일이 바뀌어야 한다. 지금은 rename이 링크
/// 자체를 일반 파일로 덮어써서, 링크는 끊기고 원래 파일은 옛 내용으로 남는다.
#[test]
fn ksq_write_file_through_symlink_updates_target() {
    let dir = TempDir::new("symlink");
    let target = dir.0.join("real.ksq");
    let link = dir.0.join("link.ksq");
    let (old, _) = ksq::from_str(r#"{"tracks": [{"name": "old"}]}"#).unwrap();
    let (new, _) = ksq::from_str(r#"{"tracks": [{"name": "new"}]}"#).unwrap();
    ksq::write_file(&target, &old, Compression::None).unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    ksq::write_file(&link, &new, Compression::None).unwrap();

    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "링크가 일반 파일로 바뀌었다"
    );
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        ksq::to_string(&new),
        "링크 대상이 옛 내용으로 남았다"
    );
}
