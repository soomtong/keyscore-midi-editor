//! S2-T6a 사용자 키맵 편집(GPUI 없음): 실효 바인딩, 충돌, 사용자 레이어 편집과 왕복.

use keyscore_app::keymap::source::{ActionRef, Layer, parse};
use keyscore_app::keymap::user::{Effective, UserKeymap, binding_conflicts, effective};
use keyscore_app::keymap::{LayerSet, Preset, load_layers};

fn layers(preset: &str, user: &str) -> LayerSet {
    LayerSet {
        standard: parse(
            r#"[{"bindings": {"cmd-s": "file::Save"}}]"#,
            Layer::Standard,
        )
        .0,
        preset: parse(preset, Layer::Preset).0,
        user: parse(user, Layer::User).0,
    }
}

fn keys_of<'a>(eff: &'a [Effective], action: &str) -> Vec<(Option<&'a str>, &'a str)> {
    eff.iter()
        .filter(|e| e.action == action)
        .map(|e| (e.context.as_deref(), e.keys.as_str()))
        .collect()
}

const PRESET: &str = r#"[
  {"context": "Normal", "bindings": {"m": "track::ToggleMute", "s": "track::ToggleSolo", "g g": "cursor::FirstTrack"}},
  {"context": "Step", "bindings": {"m": "step::Rest"}}
]"#;

// ---------- 2. 실효 바인딩과 충돌 ----------

#[test]
fn effective_bindings_layer_and_unbind() {
    let l = layers(
        PRESET,
        r#"[{"context": "Normal", "bindings": {"m": "track::ToggleSolo", "s": null}}]"#,
    );
    let eff = effective(&l);
    // 사용자가 `m`을 덮었다: ToggleMute는 키를 잃고 ToggleSolo는 `m`을 얻는다. `s`는 지워졌다.
    assert!(keys_of(&eff, "track::ToggleMute").is_empty());
    assert_eq!(
        keys_of(&eff, "track::ToggleSolo"),
        vec![(Some("Normal"), "m")]
    );
    // 다른 컨텍스트의 같은 키는 그대로
    assert_eq!(keys_of(&eff, "step::Rest"), vec![(Some("Step"), "m")]);
    assert_eq!(keys_of(&eff, "file::Save"), vec![(None, "cmd-s")]);
    let m = eff
        .iter()
        .find(|e| e.action == "track::ToggleSolo")
        .unwrap();
    assert_eq!(m.layer, Layer::User);
    // 컨텍스트 공백과 키 표기 차이는 같은 자리로 본다
    let l = layers(
        PRESET,
        r#"[{"context": "Normal ", "bindings": {"G G": "track::New"}}]"#,
    );
    let eff = effective(&l);
    assert!(keys_of(&eff, "cursor::FirstTrack").is_empty(), "{eff:?}");
}

#[test]
fn binding_conflicts_same_keys_and_prefix() {
    let eff = effective(&layers(PRESET, "[]"));
    // 같은 컨텍스트·같은 키, 다른 Action
    let c = binding_conflicts(&eff, Some("Normal"), "m", "track::New");
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].action, "track::ToggleMute");
    // 같은 Action이면 충돌이 아니다
    assert!(binding_conflicts(&eff, Some("Normal"), "m", "track::ToggleMute").is_empty());
    // 다른 컨텍스트는 충돌이 아니다
    assert!(binding_conflicts(&eff, Some("Lyric"), "m", "track::New").is_empty());
    // 접두: 새 `g`는 `g g`의 앞, 새 `s x`는 `s`로 시작
    let c = binding_conflicts(&eff, Some("Normal"), "g", "track::New");
    assert_eq!(c[0].action, "cursor::FirstTrack");
    let c = binding_conflicts(&eff, Some("Normal"), "s x", "track::New");
    assert_eq!(c[0].action, "track::ToggleSolo");
}

// ---------- 1. 사용자 레이어 편집과 왕복 ----------

#[test]
fn user_keymap_round_trips_and_edits() {
    let preset = parse(PRESET, Layer::Preset).0;
    let text = r#"[{"context": "Normal", "bindings": {"x": "track::New", "y": ["track::SetChannel", "3"]}}]"#;
    let user = UserKeymap::parse(text).unwrap();
    let again = UserKeymap::parse(&user.to_json()).unwrap();
    assert_eq!(user, again);
    // bind: 같은 컨텍스트의 그 Action 키를 새 키로 바꾼다. 프리셋 키는 `null`로 지운다
    let mut user = UserKeymap::parse("[]").unwrap();
    let eff = effective(&layers(PRESET, "[]"));
    user.bind("track::ToggleMute", Some("Normal"), "u", &eff);
    let l = layers(PRESET, &user.to_json());
    let eff = effective(&l);
    assert_eq!(
        keys_of(&eff, "track::ToggleMute"),
        vec![(Some("Normal"), "u")]
    );
    // 다시 bind: 사용자 키를 바꾼다(항목이 쌓이지 않는다)
    user.bind("track::ToggleMute", Some("Normal"), "v", &eff);
    let eff = effective(&layers(PRESET, &user.to_json()));
    assert_eq!(
        keys_of(&eff, "track::ToggleMute"),
        vec![(Some("Normal"), "v")]
    );
    // 새 키가 다른 Action의 키면 그 Action은 그 키를 잃는다(충돌 시트에서 「새 바인딩 유지」)
    user.bind("track::ToggleMute", Some("Normal"), "s", &eff);
    let eff = effective(&layers(PRESET, &user.to_json()));
    assert_eq!(
        keys_of(&eff, "track::ToggleMute"),
        vec![(Some("Normal"), "s")]
    );
    assert!(keys_of(&eff, "track::ToggleSolo").is_empty());
    // clear: 모든 실효 키를 지운다
    user.clear("step::Rest", &eff);
    let eff = effective(&layers(PRESET, &user.to_json()));
    assert!(keys_of(&eff, "step::Rest").is_empty());
    // restore: 그 Action에 관한 사용자 항목을 빼서 프리셋으로
    user.restore("track::ToggleMute", &preset);
    user.restore("step::Rest", &preset);
    let eff = effective(&layers(PRESET, &user.to_json()));
    assert_eq!(
        keys_of(&eff, "track::ToggleMute"),
        vec![(Some("Normal"), "m")]
    );
    assert_eq!(keys_of(&eff, "step::Rest"), vec![(Some("Step"), "m")]);
    // ToggleSolo의 `s`는 ToggleMute 복원과 함께 돌아온다(그 `s`를 가리던 항목이 ToggleMute 것이었다)
    assert_eq!(
        keys_of(&eff, "track::ToggleSolo"),
        vec![(Some("Normal"), "s")]
    );
    // 쓴 파일은 엔진이 오류 없이 읽는다
    let (_, errors) = load_layers(Preset::Default, Some(&user.to_json()));
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn user_keymap_parse_rejects_syntax_errors_and_keeps_unbind() {
    assert!(UserKeymap::parse("[{").is_err());
    let user =
        UserKeymap::parse(r#"[{"bindings": {"q": null}}, {"context": "Step", "bindings": {}}]"#)
            .unwrap();
    let back = parse(&user.to_json(), Layer::User).0;
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].action, ActionRef::Unbind);
    assert_eq!(back[0].context, None);
}
