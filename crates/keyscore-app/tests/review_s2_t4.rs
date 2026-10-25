//! S2-T4 적대적 리뷰 1회차의 깨뜨리기 시도 가운데 깨뜨리지 못한 것(통과하는 테스트).
//! 깨진 시도는 `docs/reports/s2-t4/review-1.md` 부록에 있다.

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::commands;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, View};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::Project;

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-s2t4-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx
        .update(|cx| open_project_window(Project::new(), cx))
        .unwrap();
    render(cx, handle);
    (handle, view)
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    render(cx, handle);
}

fn type_text(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    cx.simulate_input(handle, text);
    render(cx, handle);
}

fn palette_open(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).palette_open())
}

fn mode_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Mode {
    cx.read(|cx| view.read(cx).mode_state().mode())
}

fn view_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> View {
    cx.read(|cx| view.read(cx).mode_state().view())
}

// 빈 질의의 Esc가 팔레트만 닫고, 같은 Esc가 문서(Step의 `escape` → `mode::Normal`)로 새지 않는다.
#[gpui_kit::test]
fn palette_escape_on_empty_query_closes_palette_without_leaving_step(cx: &mut TestAppContext) {
    start(cx, "esc-step");
    let (handle, view) = open(cx);
    press(cx, handle, "ctrl-tab");
    assert_eq!(mode_of(cx, &view), Mode::Step);
    press(cx, handle, "cmd-shift-p");
    assert!(palette_open(cx, &view));
    press(cx, handle, "escape");
    assert!(!palette_open(cx, &view));
    assert_eq!(mode_of(cx, &view), Mode::Step, "Esc가 문서까지 내려갔다");
}

// 팔레트의 Enter가 실행 뒤 문서(배치 뷰 `enter` → `clip::Open`)로 한 번 더 가지 않는다.
#[gpui_kit::test]
fn palette_enter_runs_once_and_does_not_reach_document(cx: &mut TestAppContext) {
    start(cx, "enter-leak");
    let (handle, view) = open(cx);
    let before_view = view_of(cx, &view);
    let before = cx.read(|cx| view.read(cx).layout().inspector_open);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "view::toggleinspector");
    press(cx, handle, "enter");
    assert_ne!(cx.read(|cx| view.read(cx).layout().inspector_open), before);
    assert_eq!(
        view_of(cx, &view),
        before_view,
        "Enter가 clip::Open으로 샜다"
    );
    assert_eq!(mode_of(cx, &view), Mode::Normal);
}

// 열린 팔레트에서 친 `:`는 질의 글자다(Normal의 `:` → `palette::OpenCommandLine`이 가로채지 않는다).
// S3-T2(결정 A12): `:`는 팔레트를 `:` 접두가 채워진 채 열므로 질의는 `:a:b`다.
#[gpui_kit::test]
fn colon_typed_in_open_palette_goes_to_query(cx: &mut TestAppContext) {
    start(cx, "colon");
    let (handle, view) = open(cx);
    press(cx, handle, ":");
    assert!(palette_open(cx, &view));
    type_text(cx, handle, "a:b");
    assert_eq!(
        cx.read(|cx| view.read(cx).palette_query(cx)).as_deref(),
        Some(":a:b")
    );
}

// 두 번 열어도 팔레트는 하나이고 질의를 지우지 않는다. Enter 실행은 한 번이다.
#[gpui_kit::test]
fn opening_twice_keeps_one_palette_and_runs_once(cx: &mut TestAppContext) {
    start(cx, "twice");
    let (handle, view) = open(cx);
    let before = cx.read(|cx| view.read(cx).layout().inspector_open);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "view::toggleinspector");
    press(cx, handle, "cmd-shift-p");
    assert_eq!(
        cx.read(|cx| view.read(cx).palette_query(cx)).as_deref(),
        Some("view::toggleinspector")
    );
    press(cx, handle, "enter");
    assert!(!palette_open(cx, &view));
    assert_ne!(
        cx.read(|cx| view.read(cx).layout().inspector_open),
        before,
        "두 번 실행되면 원래대로 돌아온다"
    );
}

// 실제 팔레트 경로(키로 연 팔레트의 Enter)가 쌓인 카운트를 비운다(결정 C). 기존 테스트는
// `run_action`을 직접 불러 팔레트 경로 자체는 보지 않는다.
#[gpui_kit::test]
fn palette_enter_clears_pending_count(cx: &mut TestAppContext) {
    start(cx, "count");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = open(cx);
    press(cx, handle, "3");
    assert_eq!(cx.read(keymap::pending_count), Some(3));
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "view::toggleinspector");
    press(cx, handle, "enter");
    assert!(!palette_open(cx, &view));
    assert_eq!(cx.read(keymap::pending_count), None);
}

// 팔레트로 모드 Action을 실행하면 문서 처리기에 닿는다(포커스 복귀 뒤 디스패치).
#[gpui_kit::test]
fn palette_runs_mode_action_into_document(cx: &mut TestAppContext) {
    start(cx, "mode");
    let (handle, view) = open(cx);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "mode::play");
    press(cx, handle, "enter");
    assert_eq!(mode_of(cx, &view), Mode::Play);
    // 문서 포커스로 돌아왔으므로 Play의 Esc가 Normal로 나간다
    press(cx, handle, "escape");
    assert_eq!(mode_of(cx, &view), Mode::Normal);
}

// 열린 팔레트에서 다른 명령 키(`cmd-i`, 컨텍스트 없는 표준 키가 아니라 Normal 키)는 문서로 가지 않는다.
#[gpui_kit::test]
fn normal_context_keys_do_not_reach_document_while_palette_open(cx: &mut TestAppContext) {
    start(cx, "keys");
    let (handle, view) = open(cx);
    let before = cx.read(|cx| view.read(cx).layout().inspector_open);
    press(cx, handle, "cmd-shift-p");
    press(cx, handle, "cmd-i");
    assert_eq!(cx.read(|cx| view.read(cx).layout().inspector_open), before);
    assert!(palette_open(cx, &view));
}

// 카운트를 친 뒤 팔레트를 열었다가 실행 없이 Esc로 닫아도 카운트가 다음 키 Action에 남지 않는다.
#[gpui_kit::test]
fn count_does_not_survive_palette_opened_and_escaped(cx: &mut TestAppContext) {
    start(cx, "count-esc");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = open(cx);
    press(cx, handle, "3");
    press(cx, handle, "cmd-shift-p");
    press(cx, handle, "escape");
    assert!(!palette_open(cx, &view));
    assert_eq!(cx.read(keymap::pending_count), None);
}

// 사용자 키맵으로 `palette::Close`를 다른 키에 걸어도 질의가 있는 팔레트를 닫는다.
#[gpui_kit::test]
fn user_bound_palette_close_closes_palette_with_query(cx: &mut TestAppContext) {
    let dir = start(cx, "close-key");
    std::fs::write(
        dir.join("keymap.json"),
        r#"[{"context": "Palette", "bindings": {"f7": "palette::Close"}}]"#,
    )
    .unwrap();
    cx.update(keymap::apply);
    let (handle, view) = open(cx);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "zoom");
    press(cx, handle, "f7");
    assert!(!palette_open(cx, &view));
}

// ---------- 리뷰 1 부록(지적 1–5, 제안 1): 고친 뒤 회귀 방지로 옮김 ----------

// A-1 (지적 2): Tab(자동완성) 뒤 Enter가 Tab으로 고른 명령을 실행한다.
#[gpui_kit::test]
fn palette_tab_then_enter_runs_the_completed_command(cx: &mut TestAppContext) {
    start(cx, "r1-tab-enter");
    let (handle, view) = open(cx);
    let before = cx.read(|cx| view.read(cx).layout().track_list_open);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "view::toggletracklist");
    press(cx, handle, "tab");
    assert_eq!(
        cx.read(|cx| view.read(cx).palette_query(cx)).as_deref(),
        Some("트랙 목록")
    );
    press(cx, handle, "enter");
    assert!(!cx.read(|cx| view.read(cx).palette_open()));
    assert_ne!(
        cx.read(|cx| view.read(cx).layout().track_list_open),
        before,
        "Tab으로 채운 「트랙 목록」의 Enter가 view::ToggleTrackList를 실행하지 않았다"
    );
}

// A-2 (지적 2·3): 실행할 수 있는 모든 줄에서, 그 이름으로 거른 첫 줄이 자기 자신이다(팔레트의 순위 함수).
#[gpui_kit::test]
fn every_palette_label_selects_its_own_row_first(cx: &mut TestAppContext) {
    start(cx, "r1-tab-data");
    cx.update(|cx| {
        let entries = commands::palette_entries(cx);
        let mut bad = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            if e.action().is_none() {
                continue;
            }
            let first = commands::filter_entries(&entries, e.label())[0];
            if first != i {
                bad.push(format!(
                    "{} [{}] -> {}",
                    e.label(),
                    e.name(),
                    entries[first].name()
                ));
            }
        }
        assert!(bad.is_empty(), "{}건: {:?}", bad.len(), bad);
    });
}

// A-3 (지적 4): 변형 줄까지 포함한 팔레트 이름은 겹치지 않는다.
#[gpui_kit::test]
fn palette_labels_are_unique_including_variants(cx: &mut TestAppContext) {
    start(cx, "r1-labels");
    cx.update(|cx| {
        let mut labels: std::collections::BTreeMap<String, Vec<&str>> =
            std::collections::BTreeMap::new();
        let entries = commands::palette_entries(cx);
        for e in &entries {
            labels
                .entry(e.label().to_string())
                .or_default()
                .push(e.name());
        }
        let dups: Vec<_> = labels.iter().filter(|(_, v)| v.len() > 1).collect();
        assert!(dups.is_empty(), "{}건: {:?}", dups.len(), dups);
    });
}

// A-4 (지적 5): 팔레트가 열린 채 문서가 포커스를 가져가면(클릭) 팔레트가 닫힌다.
#[gpui_kit::test]
fn palette_closes_when_document_takes_focus_or_on_escape(cx: &mut TestAppContext) {
    start(cx, "r1-click");
    let (handle, view) = open(cx);
    press(cx, handle, "cmd-shift-p");
    cx.update_window(handle, |_, window, cx| window.click("status-bar", cx))
        .unwrap();
    render(cx, handle);
    press(cx, handle, "escape");
    assert!(
        !cx.read(|cx| view.read(cx).palette_open()),
        "문서를 누른 뒤 팔레트가 열린 채 남고 Esc로도 닫히지 않는다"
    );
}

// A-5 (지적 1): UI-11. 「다른 곳」은 화면 면(Surface)이고, 자리 설명이 키만 가리키지 않는다.
#[test]
fn elsewhere_places_name_a_visible_ui() {
    let bad: Vec<String> = commands::specs()
        .iter()
        .filter_map(|s| match s.place() {
            commands::Place::Elsewhere { detail, .. } if detail.ends_with("키") => {
                Some(format!("{} ({detail})", s.name()))
            }
            _ => None,
        })
        .collect();
    assert!(bad.is_empty(), "{}건: {:?}", bad.len(), bad);
}

// A-6 (제안 1): 팔레트가 열린 동안 ctrl-tab이 뒤의 모드를 바꾸지 않는다.
#[gpui_kit::test]
fn ctrl_tab_does_not_cycle_mode_behind_open_palette(cx: &mut TestAppContext) {
    start(cx, "r1-ctrltab");
    let (handle, view) = open(cx);
    press(cx, handle, "cmd-shift-p");
    press(cx, handle, "ctrl-tab");
    assert!(cx.read(|cx| view.read(cx).palette_open()));
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().mode()),
        Mode::Normal
    );
}
