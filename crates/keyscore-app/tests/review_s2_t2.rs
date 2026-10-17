//! S2-T2 적대적 리뷰 1회차: 키맵 엔진을 깨뜨리려는 테스트.
//!
//! 실패하는 테스트는 리뷰 지적 사항이고, 통과하는 테스트는 회귀 방지로 남긴다.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement,
    Keystroke, ParentElement as _, Render, Styled as _, TestAppContext, Window, div,
};
use keyscore_app::actions::cursor::PrevNote;
use keyscore_app::actions::select::Bar;
use keyscore_app::actions::view::{ToggleInspector, ToggleTrackList};
use keyscore_app::app_state::AppSettings;
use keyscore_app::keymap::conflict::{ConflictKind, conflicts};
use keyscore_app::keymap::source::{Layer, parse};
use keyscore_app::keymap::{self, Preset, load_layers, swap_alt_cmd};
use keyscore_app::settings::Settings;
use keyscore_app::{init, open_project_window};
use keyscore_core::Project;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-s2t2-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn start(cx: &mut TestAppContext, name: &str, user_keymap: Option<&str>) -> PathBuf {
    let dir = temp_dir(name);
    if let Some(text) = user_keymap {
        std::fs::write(dir.join("keymap.json"), text).unwrap();
    }
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn open(cx: &mut TestAppContext) -> AnyWindowHandle {
    let (handle, _) = cx
        .update(|cx| open_project_window(Project::new(), cx))
        .unwrap();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    handle
}

fn inspector_open(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    cx.update_window(handle, |root, _, cx| {
        let root = root.downcast::<gpui_kit::component::Root>().unwrap();
        let view = root.read(cx).view().clone();
        view.downcast::<keyscore_app::window::ProjectWindow>()
            .unwrap()
            .read(cx)
            .layout()
            .inspector_open
    })
    .unwrap()
}

fn tick(cx: &mut TestAppContext) {
    cx.executor().advance_clock(Duration::from_millis(1100));
    cx.run_until_parked();
}

/// `outer` 안에 `inner` 컨텍스트가 있는 뷰. 안쪽이 포커스를 가진다.
struct Nested {
    outer: &'static str,
    inner: &'static str,
    focus: FocusHandle,
    fired: Vec<&'static str>,
}

impl Render for Nested {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context(self.outer)
            .size_full()
            .on_action(cx.listener(|this, _: &ToggleTrackList, _, _| {
                this.fired.push("ToggleTrackList");
            }))
            .child(
                div()
                    .key_context(self.inner)
                    .track_focus(&self.focus)
                    .size_full()
                    .on_action(cx.listener(|this, _: &ToggleInspector, _, _| {
                        this.fired.push("ToggleInspector");
                    }))
                    .on_action(cx.listener(|this, _: &PrevNote, _, _| {
                        this.fired.push("cursor::PrevNote");
                    }))
                    .on_action(cx.listener(|this, _: &Bar, _, _| {
                        this.fired.push("select::Bar");
                    })),
            )
    }
}

fn open_nested(
    cx: &mut TestAppContext,
    outer: &'static str,
    inner: &'static str,
) -> (AnyWindowHandle, gpui_kit::Entity<Nested>) {
    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(gpui_kit::WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    let focus = cx.focus_handle();
                    window.focus(&focus, cx);
                    Nested {
                        outer,
                        inner,
                        focus,
                        fired: Vec::new(),
                    }
                })
            })
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    (handle, view)
}

fn strokes(keys: &str) -> Vec<Keystroke> {
    keys.split(' ')
        .map(|k| Keystroke::parse(k).unwrap_or_else(|e| panic!("{keys}: {e}")))
        .collect()
}

// ---------------------------------------------------------------- vim 프리셋과 GPUI NoAction

/// vim.json의 `Normal > PianoRoll` `null`은 기본 프리셋의 피아노롤 `b`(`select::Bar`)를 지우려는 것이다.
/// GPUI의 `NoAction`은 같은 키의 「순위가 낮은 모든」 바인딩을 가리므로 vim 자신의 `Normal` `b`
/// (`cursor::PrevNote`)까지 가린다. REQ 5.8.1은 vim에서 `b`가 이전 노트다.
#[gpui_kit::test]
fn vim_piano_roll_b_moves_to_previous_note(cx: &mut TestAppContext) {
    start(cx, "vim-b", None);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = open_nested(cx, "Normal", "PianoRoll");
    cx.simulate_keystrokes(handle, "b");
    assert_eq!(
        cx.read(|cx| view.read(cx).fired.clone()),
        ["cursor::PrevNote"]
    );
}

/// 같은 원인으로 피아노롤에서 vim 카운트 숫자 `1`–`7`이 죽는다(`8`, `9`만 산다).
#[gpui_kit::test]
fn vim_piano_roll_digits_are_count_prefix(cx: &mut TestAppContext) {
    start(cx, "vim-count", None);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, _view) = open_nested(cx, "Normal", "PianoRoll");
    cx.simulate_keystrokes(handle, "8");
    assert_eq!(drain_count(cx), Some(8), "대조군");
    cx.simulate_keystrokes(handle, "3");
    assert_eq!(drain_count(cx), Some(3));
}

/// 배치 뷰(피아노롤 밖)에서는 vim 카운트가 산다(회귀).
#[gpui_kit::test]
fn vim_arrangement_digits_are_count_prefix(cx: &mut TestAppContext) {
    start(cx, "vim-count-arr", None);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, _view) = open_nested(cx, "Normal", "Arrangement");
    cx.simulate_keystrokes(handle, "4 2");
    assert_eq!(drain_count(cx), Some(42));
}

/// 기본 프리셋 피아노롤의 `b`는 `select::Bar`다(회귀, 대조군).
#[gpui_kit::test]
fn default_piano_roll_b_selects_bar(cx: &mut TestAppContext) {
    start(cx, "default-b", None);
    let (handle, view) = open_nested(cx, "Normal", "PianoRoll");
    cx.simulate_keystrokes(handle, "b");
    assert_eq!(cx.read(|cx| view.read(cx).fired.clone()), ["select::Bar"]);
}

// ---------------------------------------------------------------- 컨텍스트 우선순위

/// GPUI는 컨텍스트 없는(전역) 바인딩의 깊이를 컨텍스트 스택 길이로 친다. 그래서 가장 깊은 컨텍스트
/// 바인딩과 깊이가 같고, 뒤에 있으면 이긴다. 모듈 문서의 「더 깊은 노드가 이긴다」에 이 예외가 없다
/// (현재 동작을 고정하는 회귀 테스트).
#[gpui_kit::test]
fn later_global_binding_ties_with_deepest_context(cx: &mut TestAppContext) {
    start(
        cx,
        "global-tie",
        Some(
            r#"[
  {"context": "Normal > Arrangement", "bindings": {"f9": "view::ToggleInspector"}},
  {"bindings": {"f9": "view::ToggleTrackList"}}
]"#,
        ),
    );
    let (handle, view) = open_nested(cx, "Normal", "Arrangement");
    cx.simulate_keystrokes(handle, "f9");
    assert_eq!(
        cx.read(|cx| view.read(cx).fired.clone()),
        ["ToggleTrackList"]
    );
}

/// 사용자 `Normal`의 바인딩은 프리셋 `Normal > PianoRoll`의 같은 키를 덮지 못한다(회귀, 문서화 필요).
#[gpui_kit::test]
fn user_shallow_context_does_not_override_deeper_preset(cx: &mut TestAppContext) {
    start(
        cx,
        "shallow-user",
        Some(r#"[{"context": "Normal", "bindings": {"b": "view::ToggleInspector"}}]"#),
    );
    let (handle, view) = open_nested(cx, "Normal", "PianoRoll");
    cx.simulate_keystrokes(handle, "b");
    assert_eq!(cx.read(|cx| view.read(cx).fired.clone()), ["select::Bar"]);
}

// ---------------------------------------------------------------- 표준 단축키(KM-12, 7.5)

/// KM-12: 표준 `Cmd` 단축키는 사용자 오버라이드로 바꿀 수 있되 경고를 띄운다.
#[test]
fn user_override_of_standard_shortcut_warns() {
    let (layers, errors) = load_layers(
        Preset::Default,
        Some(r#"[{"bindings": {"cmd-s": "edit::Undo"}}]"#),
    );
    assert_eq!(layers.user.len(), 1, "덮어쓰기 자체는 허용");
    assert!(!errors.is_empty(), "KM-12 경고가 없다");
}

/// 표준 키 보호는 문자열 비교다. 프리셋 키를 GPUI 키 입력으로 정규화해도 표준 키와 겹치지 않는지,
/// 수식키 교환 뒤에도 겹치지 않는지 본다(회귀).
#[test]
fn preset_keys_never_equal_standard_keys_after_normalization() {
    for preset in [Preset::Default, Preset::Vim] {
        let (layers, errors) = load_layers(preset, None);
        assert!(errors.is_empty(), "{errors:?}");
        let standard: Vec<Vec<Keystroke>> =
            layers.standard.iter().map(|b| strokes(&b.keys)).collect();
        for b in &layers.preset {
            for swap in [false, true] {
                let keys = if swap {
                    swap_alt_cmd(&b.keys)
                } else {
                    b.keys.clone()
                };
                assert!(
                    !standard.contains(&strokes(&keys)),
                    "{preset:?} swap={swap}: {} 가 표준 키를 덮는다",
                    b.keys
                );
            }
        }
    }
}

/// 수식키 교환은 GPUI가 받는 수식키 별칭(`super`, `win`, 대문자 `Alt`/`CMD`)도 바꿔야 한다.
#[test]
fn swap_alt_cmd_handles_modifier_aliases() {
    for (input, expected) in [
        ("super-j", "alt-j"),
        ("Alt-j", "cmd-j"),
        ("CMD-shift-k", "alt-shift-k"),
    ] {
        assert_eq!(
            strokes(&swap_alt_cmd(input)),
            strokes(expected),
            "{input} -> {}",
            swap_alt_cmd(input)
        );
    }
}

// ---------------------------------------------------------------- 파일 읽기

/// context가 문자열이 아니면 오류를 내면서도 그 절의 바인딩을 전역으로 건다. 전역 바인딩은 모든
/// 컨텍스트에서 가장 깊은 깊이로 매칭되므로 원래 의도보다 훨씬 넓게 적용된다.
#[test]
fn non_string_context_does_not_make_bindings_global() {
    let (b, e) = parse(
        r#"[{"context": 5, "bindings": {"x": "edit::Undo"}}]"#,
        Layer::User,
    );
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(b.is_empty(), "잘못된 context의 바인딩이 전역이 됐다: {b:?}");
}

/// 여러 바이트 글자는 한 열로 센다(편집기 열 번호와 맞음).
#[test]
fn error_column_counts_multibyte_characters_as_one() {
    let (_, e) = parse(r#"[{"bindings": {"키": 1}}]"#, Layer::User);
    assert_eq!((e[0].pos.line, e[0].pos.column), (1, 21), "{e:?}");
    let (_, e) = parse(r#"[{"bindings": {"🎹": 1}}]"#, Layer::User);
    assert_eq!((e[0].pos.line, e[0].pos.column), (1, 21), "{e:?}");
    let (_, e) = parse("// 한글 주석\n[1]", Layer::User);
    assert_eq!((e[0].pos.line, e[0].pos.column), (2, 2), "{e:?}");
}

/// CRLF 줄끝에서도 줄·열이 맞다.
#[test]
fn error_position_is_correct_with_crlf() {
    let (_, e) = parse("[\r\n  {\"bindings\": {\"a\": 1}}\r\n]", Layer::User);
    assert_eq!((e[0].pos.line, e[0].pos.column), (2, 22), "{e:?}");
    let (b, e) = parse(
        "[\r\n  // 주석\r\n  {\"bindings\": {\"a\": \"edit::Undo\"}},\r\n]\r\n",
        Layer::User,
    );
    assert!(e.is_empty(), "{e:?}");
    assert_eq!((b[0].pos.line, b[0].pos.column), (3, 17));
}

/// 아주 깊은 중첩(배열·객체·인자 안)도 스택을 넘기지 않는다.
#[test]
fn very_deep_nesting_is_rejected_without_overflow() {
    for text in [
        "[".repeat(200_000),
        "{\"a\":".repeat(200_000),
        format!(
            r#"[{{"bindings": {{"x": ["edit::Undo", {}]}}}}]"#,
            "[".repeat(100_000)
        ),
    ] {
        let (b, e) = parse(&text, Layer::User);
        assert!(b.is_empty());
        assert_eq!(e.len(), 1, "{e:?}");
    }
}

/// 큰 파일(바인딩 5만 개)도 읽는다.
#[test]
fn huge_flat_file_parses() {
    let mut text = String::from("[{\"bindings\": {");
    for i in 0..50_000 {
        let _ = writeln!(text, "\"f{i}\": \"edit::Undo\",");
    }
    text.push_str("}}]");
    let (b, e) = parse(&text, Layer::User);
    assert!(e.is_empty());
    assert_eq!(b.len(), 50_000);
    assert_eq!(b[49_999].pos.line, 50_000);
}

/// 결정적 의사 난수(xorshift). proptest는 이 crate의 개발 의존이 아니라 직접 둔다.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).unwrap()
    }
}

fn assert_positions_in_bounds(text: &str) {
    let (bindings, errors) = parse(text, Layer::User);
    let lines: Vec<&str> = text.split('\n').collect();
    let check = |line: usize, column: usize| {
        assert!(line >= 1 && line <= lines.len(), "{line} in {text:?}");
        assert!(
            column >= 1 && column <= lines[line - 1].chars().count() + 1,
            "{line}:{column} in {text:?}"
        );
    };
    for e in &errors {
        check(e.pos.line, e.pos.column);
    }
    for b in &bindings {
        check(b.pos.line, b.pos.column);
    }
}

/// 망가진 입력(무작위 글자열, REQ 예시의 무작위 변형)에서 패닉이 없고 위치가 파일 안을 가리킨다.
#[test]
fn malformed_input_never_panics_and_positions_stay_in_bounds() {
    const ALPHABET: &[char] = &[
        '[', ']', '{', '}', '"', ':', ',', '/', '*', '\\', 'u', 'n', 'l', 't', 'r', 'e', 'f', 'a',
        '1', '0', '-', '+', '.', 'E', ' ', '\n', '\r', '\t', '한', '🎹', '\u{feff}', '(', ')', 'x',
        'd', '\u{0}',
    ];
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for _ in 0..20_000 {
        let len = rng.below(48);
        let text: String = (0..len)
            .map(|_| ALPHABET[rng.below(ALPHABET.len())])
            .collect();
        assert_positions_in_bounds(&text);
    }
    let base = r#"[
  // 주석
  {"context": "Normal > Arrangement", "bindings": {"한": "edit::Undo", "g g": ["note::SetLength", "eighth"], "x": null,}},
  {"context": "Step", "bindings": {"5": "note::SetLength(sixteenth)", "A": "edit::Redo"}}
]"#;
    let chars: Vec<char> = base.chars().collect();
    for _ in 0..20_000 {
        let mut c = chars.clone();
        for _ in 0..=rng.below(4) {
            let i = rng.below(c.len());
            match rng.below(3) {
                0 => {
                    c.remove(i);
                }
                1 => c.insert(i, ALPHABET[rng.below(ALPHABET.len())]),
                _ => c[i] = ALPHABET[rng.below(ALPHABET.len())],
            }
        }
        assert_positions_in_bounds(&c.into_iter().collect::<String>());
    }
}

/// 이상한 이스케이프와 숫자.
#[test]
fn odd_escapes_and_numbers_do_not_panic() {
    for text in [
        r#"["🎹"]"#,
        r#"["\ud800"]"#,
        r#"["\u12"]"#,
        r#"["\x"]"#,
        "[1e999999999]",
        "[-]",
        "[01]",
        "[1.]",
        "[--1]",
        "[nul]",
        "[truex]",
        "\"",
        "/",
        "/*",
        "[/* */ /",
        "\u{feff}",
        "",
    ] {
        assert_positions_in_bounds(text);
    }
}

// ---------------------------------------------------------------- 충돌 감지(KM-4)

/// 같은 컨텍스트를 다른 표기(`Normal > PianoRoll`과 `Normal>PianoRoll`)로 쓰면 충돌을 놓친다.
#[test]
fn conflicts_detected_across_context_spellings() {
    let (b, e) = parse(
        r#"[
  {"context": "Normal > PianoRoll", "bindings": {"x": "edit::Undo"}},
  {"context": "Normal>PianoRoll", "bindings": {"x": "edit::Redo"}}
]"#,
        Layer::User,
    );
    assert!(e.is_empty());
    let found = conflicts(&b);
    assert!(
        found.iter().any(|c| c.kind == ConflictKind::SameKeys),
        "{found:?}"
    );
}

// ---------------------------------------------------------------- 카운트(KM-5)

/// 카운트를 쓰지 않는 Action이 지나가면 카운트는 버려져야 한다. 아니면 `3`, `Esc`, … 한참 뒤의
/// `x`가 3번 실행된다.
#[gpui_kit::test]
fn count_is_cleared_when_another_action_runs(cx: &mut TestAppContext) {
    start(
        cx,
        "count-stale",
        Some(r#"[{"bindings": {"3": "operator::Count(3)", "f5": "view::ToggleInspector"}}]"#),
    );
    let handle = open(cx);
    cx.simulate_keystrokes(handle, "3 f5");
    assert_eq!(drain_count(cx), None);
}

/// 카운트는 9999에서 멈추고 넘치지 않는다. 인자 범위 밖 숫자도 패닉하지 않는다(회귀).
#[gpui_kit::test]
fn count_saturates_and_clamps_odd_digits(cx: &mut TestAppContext) {
    start(
        cx,
        "count-sat",
        Some(
            r#"[{"bindings": {"9": "operator::Count(9)", "f1": "operator::Count(-7)", "f2": "operator::Count(9223372036854775807)"}}]"#,
        ),
    );
    let handle = open(cx);
    cx.simulate_keystrokes(handle, "9 9 9 9 9 9 9 9 9 9 9 9");
    assert_eq!(drain_count(cx), Some(9999));
    cx.simulate_keystrokes(handle, "f1 f2");
    assert_eq!(drain_count(cx), Some(9));
    assert!(cx.update(|cx| keymap::errors(cx).is_empty()));
}

// ---------------------------------------------------------------- 핫리로드(KM-6)

#[gpui_kit::test]
fn hot_reload_survives_delete_and_recreate(cx: &mut TestAppContext) {
    let dir = start(
        cx,
        "reload-delete",
        Some(r#"[{"bindings": {"f6": "view::ToggleInspector"}}]"#),
    );
    let path = dir.join("keymap.json");
    let handle = open(cx);
    cx.simulate_keystrokes(handle, "f6");
    assert!(!inspector_open(cx, handle));
    std::fs::remove_file(&path).unwrap();
    tick(cx);
    cx.simulate_keystrokes(handle, "f6");
    assert!(!inspector_open(cx, handle), "지운 파일의 바인딩이 남았다");
    std::fs::write(&path, r#"[{"bindings": {"f6": "view::ToggleInspector"}}]"#).unwrap();
    tick(cx);
    cx.simulate_keystrokes(handle, "f6");
    assert!(inspector_open(cx, handle));
}

#[gpui_kit::test]
fn hot_reload_invalid_then_valid(cx: &mut TestAppContext) {
    let dir = start(cx, "reload-invalid", None);
    let path = dir.join("keymap.json");
    let handle = open(cx);
    std::fs::write(
        &path,
        "[\n  {\"bindings\": {\"f6\" \"view::ToggleInspector\"}}\n]",
    )
    .unwrap();
    tick(cx);
    let errors = cx.update(|cx| keymap::errors(cx).to_vec());
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].pos.line, 2);
    assert!(keymap::describe(&errors[0]).starts_with("keymap.json 2:"));
    // 사용자 파일이 깨져도 표준·프리셋은 산다.
    let save_bound = cx.update(|cx| {
        cx.key_bindings()
            .borrow()
            .bindings_for_action(&keyscore_app::actions::file::Save)
            .count()
    });
    assert_eq!(save_bound, 1);
    let before = inspector_open(cx, handle);
    // 빠르게 두 번 고치면 마지막 내용이 이긴다.
    std::fs::write(&path, r#"[{"bindings": {"f7": "view::ToggleInspector"}}]"#).unwrap();
    std::fs::write(&path, r#"[{"bindings": {"f6": "view::ToggleInspector"}}]"#).unwrap();
    tick(cx);
    assert!(cx.update(|cx| keymap::errors(cx).is_empty()));
    cx.simulate_keystrokes(handle, "f6");
    assert_ne!(inspector_open(cx, handle), before);
}

// ---------------------------------------------------------------- 설정·프리셋 전환

#[gpui_kit::test]
fn keymap_settings_drive_engine_at_boot(cx: &mut TestAppContext) {
    let dir = temp_dir("settings-boot");
    std::fs::write(
        dir.join("settings.json"),
        r#"{"keymap": {"preset": "vim", "swap_alt_cmd": true}}"#,
    )
    .unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let options = cx.update(|cx| keymap::options(cx));
    assert_eq!(options.preset, Preset::Vim);
    assert!(options.swap_alt_cmd);
}

#[test]
fn keymap_settings_round_trip_keeps_unknown_keys() {
    let dir = temp_dir("settings-rt");
    let path = dir.join("settings.json");
    std::fs::write(
        &path,
        r#"{"keymap": {"preset": "vim", "swap_alt_cmd": true, "sequence_timeout_ms": 500}}"#,
    )
    .unwrap();
    let (settings, warnings) = Settings::load(&path);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(settings.keymap.preset, Preset::Vim);
    settings.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("sequence_timeout_ms"), "{text}");
    let (again, _) = Settings::load(&path);
    assert_eq!(again, settings);
}

/// 프리셋 전환은 설정(`settings.json`의 keymap.preset)에도 남아야 다음 실행에서 유지된다. 지금은
/// 엔진 상태와 설정이 따로 놀아 두 진실이 생긴다.
#[gpui_kit::test]
fn set_preset_persists_to_settings(cx: &mut TestAppContext) {
    start(cx, "preset-persist", None);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    assert_eq!(
        cx.update(|cx| AppSettings::get(cx).keymap.preset),
        Preset::Vim
    );
}

/// S2-T1의 `cmd-i` 인스펙터 토글이 두 프리셋에서 산다(이름 바꾸기 뒤 회귀).
#[gpui_kit::test]
fn s2_t1_inspector_shortcut_survives_in_both_presets(cx: &mut TestAppContext) {
    start(cx, "s2t1", None);
    let handle = open(cx);
    for preset in [Preset::Default, Preset::Vim] {
        cx.update(|cx| keymap::set_preset(cx, preset));
        let before = inspector_open(cx, handle);
        cx.simulate_keystrokes(handle, "cmd-i");
        assert_ne!(inspector_open(cx, handle), before, "{preset:?}");
    }
}

// ---------------------------------------------------------------- 물리 키와 Shift 숫자(KM-7, B25)

/// GPUI macOS(`gpui-pre-macos` events.rs `parse_keystroke`)는 Shift+3을 US/ABC 자판에서
/// `key: "#"`, `shift: false`로 만든다(Shift가 소문자가 아닌 글자를 내면 Shift를 떼고 그 글자를 키로
/// 쓴다). 그래서 프리셋의 `shift-1`…`shift-7`(점음표 길이, 14개)은 실제 입력과 맞지 않는다.
#[gpui_kit::test]
fn shift_digit_dotted_length_matches_macos_keystroke(cx: &mut TestAppContext) {
    use std::cell::Cell;
    use std::rc::Rc;

    start(cx, "shift-digit", None);
    let fired = Rc::new(Cell::new(false));
    let seen = fired.clone();
    cx.update(|cx| {
        cx.on_action(move |_: &keyscore_app::actions::step::SetDottedLength, _| {
            seen.set(true);
        });
    });
    let (handle, _view) = open_nested(cx, "Step", "Dummy");
    // 대조군: 바인딩 표기 그대로의 입력은 닿는다.
    cx.dispatch_keystroke(handle, Keystroke::parse("shift-3").unwrap());
    assert!(fired.replace(false), "대조군");
    cx.dispatch_keystroke(
        handle,
        Keystroke {
            modifiers: gpui_kit::Modifiers::default(),
            key: "#".into(),
            key_char: Some("#".into()),
        },
    );
    assert!(
        fired.get(),
        "Shift+3이 step::SetDottedLength(quarter)에 닿지 않는다"
    );
}

/// 쌓인 카운트를 보고 비운다. `take_count`는 키 디스패치 안에서만 주므로(S2-T2 리뷰 2 지적 2·3)
/// 처리기 밖의 테스트는 이 도우미를 쓴다.
fn drain_count(cx: &mut TestAppContext) -> Option<u32> {
    cx.update(|cx| {
        let c = keymap::pending_count(cx);
        keymap::clear_count(cx);
        c
    })
}
