//! S2-T5c 적대적 리뷰 4(좁은 확인 리뷰). 깨뜨리지 못한 시도를 남긴다. 깨진 시도는
//! `docs/reports/s2-t5c/review-4.md` 부록에 있다.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, ksq};

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review4-t5c-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
}

fn open(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    (handle, view)
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
    cx.update_window(handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn right_click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
    cx.update_window(handle, |_, window, cx| window.right_click(id, cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    cx.run_until_parked();
    render(cx, handle);
}

fn type_text(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    cx.simulate_input(handle, text);
    cx.run_until_parked();
    render(cx, handle);
}

fn shown(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.value().map(str::to_owned))
    })
    .unwrap()
}

fn label(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.label().map(str::to_owned))
    })
    .unwrap()
}

fn project_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Project {
    cx.read(|cx| view.read(cx).document().read(cx).project().clone())
}

fn undo_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).undo_len())
}

fn two_tracks() -> Project {
    ksq::from_str(
        r#"{"tracks": [{"name": "Lead", "channel": 1, "program": 0},
            {"name": "Bass", "channel": 2, "program": 33, "bank": 5}]}"#,
    )
    .unwrap()
    .0
}

fn dirty_tempo(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a backspace");
    type_text(cx, handle, text);
}

fn dirty_name(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    click(cx, handle, "inspector-name");
    press(cx, handle, "cmd-a backspace");
    type_text(cx, handle, text);
}

fn muted_count(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    project_of(cx, view)
        .tracks()
        .iter()
        .filter(|t| t.mute())
        .count()
}

/// 프리셋의 뮤트 키를 쳐서 뮤트 개수가 바뀌는지(모드 키가 사는지) 본다.
fn mute_toggles(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    preset: Preset,
) -> bool {
    let before = muted_count(cx, view);
    press(
        cx,
        handle,
        if preset == Preset::Vim {
            "shift-m"
        } else {
            "m"
        },
    );
    muted_count(cx, view) != before
}

const PRESETS: [(Preset, &str); 2] = [(Preset::Default, "def"), (Preset::Vim, "vim")];

fn use_preset(cx: &mut TestAppContext, preset: Preset) {
    cx.update(|cx| keymap::set_preset(cx, preset));
}

fn ctx(cx: &mut TestAppContext, handle: AnyWindowHandle) -> String {
    cx.update_window(handle, |_, window, _| {
        format!("{:?}", window.context_stack())
    })
    .unwrap()
}

fn center(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    id: &'static str,
) -> gpui_kit::Point<gpui_kit::Pixels> {
    cx.update_window(handle, |_, window, _| window.find(id).bounds().center())
        .unwrap()
}

fn middle_click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
    let at = center(cx, handle, id);
    let mut vcx = gpui_kit::VisualTestContext::from_window(handle, cx);
    vcx.simulate_mouse_down(
        at,
        gpui_kit::MouseButton::Middle,
        gpui_kit::Modifiers::none(),
    );
    vcx.simulate_mouse_up(
        at,
        gpui_kit::MouseButton::Middle,
        gpui_kit::Modifiers::none(),
    );
    cx.run_until_parked();
    render(cx, handle);
}

const OUTSIDE: [&str; 20] = [
    "track-mute-0",
    "track-solo-0",
    "track-mute-1",
    "track-add",
    "inspector-color",
    "inspector-port",
    "inspector-channel",
    "inspector-mute",
    "inspector-solo",
    "inspector-test",
    "inspector-program",
    "transport-play",
    "transport-stop",
    "transport-panic",
    "transport-record",
    "transport-key",
    "transport-metronome",
    "transport-tap",
    "transport-ai",
    "transport-port",
];

/// 겹 바깥 버튼·토글·메뉴 버튼을 오른쪽/가운데 버튼으로 눌러도(입력란이 깨끗하든 고치는 중이든) 모드 키가 산다.
#[gpui_kit::test]
fn outside_buttons_right_middle_keep_mode_keys(cx: &mut TestAppContext) {
    let mut fails = Vec::new();
    for (preset, pn) in PRESETS {
        for dirty in [false, true] {
            for button in OUTSIDE {
                for middle in [false, true] {
                    let tag = format!("{pn}-{dirty}-{button}-{middle}");
                    start(cx, &tag.replace('/', "_"));
                    use_preset(cx, preset);
                    let (handle, view) = open(cx, two_tracks());
                    if dirty {
                        dirty_name(cx, handle, "Zz");
                    }
                    if middle {
                        middle_click(cx, handle, button);
                    } else {
                        right_click(cx, handle, button);
                    }
                    let before = muted_count(cx, &view) > 0;
                    let c = ctx(cx, handle);
                    if before || !mute_toggles(cx, handle, &view, preset) {
                        fails.push(format!("{tag} ctx={c} muted_by_click={before}"));
                    }
                    cx.update_window(handle, |_, window, _| window.remove_window())
                        .unwrap();
                }
            }
        }
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

/// 입력란을 왼쪽/오른쪽으로 누르면 포커스가 거기 남고 글자가 들어간다(두 프리셋).
#[gpui_kit::test]
fn inputs_keep_focus_on_left_and_right_click(cx: &mut TestAppContext) {
    let mut fails = Vec::new();
    for (preset, pn) in PRESETS {
        for id in [
            "transport-tempo",
            "transport-meter",
            "inspector-name",
            "inspector-bank",
            "inspector-velocity",
        ] {
            for right in [false, true] {
                start(cx, &format!("in-{pn}-{id}-{right}"));
                use_preset(cx, preset);
                let (handle, view) = open(cx, two_tracks());
                if right {
                    right_click(cx, handle, id);
                } else {
                    click(cx, handle, id);
                }
                press(cx, handle, "cmd-a backspace");
                type_text(cx, handle, "m");
                let s = shown(cx, handle, id);
                if s.as_deref() != Some("m") || muted_count(cx, &view) > 0 {
                    fails.push(format!("{pn} {id} right={right} shown={s:?}"));
                }
                cx.update_window(handle, |_, window, _| window.remove_window())
                    .unwrap();
            }
        }
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

/// 한 입력란이 고쳐지는 중 다른 입력란을 왼쪽/오른쪽으로 누른다: 새 입력란이 포커스를 받고 글자가 들어간다.
#[gpui_kit::test]
fn switch_between_dirty_inputs_left_and_right(cx: &mut TestAppContext) {
    let mut out = Vec::new();
    for right in [false, true] {
        start(cx, &format!("sw-{right}"));
        let (handle, view) = open(cx, two_tracks());
        let len = undo_len(cx, &view);
        dirty_tempo(cx, handle, "95");
        if right {
            right_click(cx, handle, "inspector-name");
        } else {
            click(cx, handle, "inspector-name");
        }
        press(cx, handle, "cmd-a backspace");
        type_text(cx, handle, "Qm");
        out.push(format!(
            "right={right} name={:?} tempo={:?} undo+={} muted={}",
            shown(cx, handle, "inspector-name"),
            shown(cx, handle, "transport-tempo"),
            undo_len(cx, &view) - len,
            muted_count(cx, &view) > 0
        ));
        cx.update_window(handle, |_, window, _| window.remove_window())
            .unwrap();
    }
    assert_eq!(
        out[0].replace("right=false", ""),
        out[1].replace("right=true", ""),
        "{out:?}"
    );
}

/// 고치던 이름에서 트랙 목록 버튼을 왼쪽 클릭하면 이름은 취소되고 모드 키가 산다.
#[gpui_kit::test]
fn left_click_track_button_cancels_dirty_input(cx: &mut TestAppContext) {
    for (preset, pn) in PRESETS {
        for id in ["track-mute-1", "track-solo-1", "track-add"] {
            start(cx, &format!("lt-{pn}-{id}"));
            use_preset(cx, preset);
            let (handle, view) = open(cx, two_tracks());
            dirty_name(cx, handle, "Zz");
            click(cx, handle, id);
            let c = ctx(cx, handle);
            assert_eq!(
                project_of(cx, &view).tracks()[0].name(),
                "Lead",
                "{pn} {id}"
            );
            assert!(mute_toggles(cx, handle, &view, preset), "{pn} {id} ctx={c}");
            cx.update_window(handle, |_, window, _| window.remove_window())
                .unwrap();
        }
    }
}

/// 고치던 이름에서 인스펙터/트랜스포트 버튼을 왼쪽 클릭한 뒤 모드 키가 산다.
#[gpui_kit::test]
fn left_click_other_buttons_with_dirty_input(cx: &mut TestAppContext) {
    let mut fails = Vec::new();
    for (preset, pn) in PRESETS {
        for id in [
            "inspector-mute",
            "inspector-solo",
            "transport-metronome",
            "transport-tap",
            "transport-stop",
        ] {
            start(cx, &format!("lo-{pn}-{id}"));
            use_preset(cx, preset);
            let (handle, view) = open(cx, two_tracks());
            dirty_name(cx, handle, "Zz");
            click(cx, handle, id);
            let c = ctx(cx, handle);
            let before = muted_count(cx, &view);
            let name = project_of(cx, &view).tracks()[0].name().to_owned();
            // 인스펙터 뮤트/솔로 클릭은 그 자체로 뮤트를 바꾼다
            let toggled = mute_toggles(cx, handle, &view, preset);
            if name != "Lead" || !toggled {
                fails.push(format!(
                    "{pn} {id} name={name} before={before} toggled={toggled} ctx={c}"
                ));
            }
            cx.update_window(handle, |_, window, _| window.remove_window())
                .unwrap();
        }
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

/// 고치던 입력란에서 패널의 빈 자리를 누르면 취소되고 모드 키가 산다.
#[gpui_kit::test]
fn click_empty_panel_space_cancels_dirty(cx: &mut TestAppContext) {
    for (preset, pn) in PRESETS {
        for panel in ["inspector", "track-list"] {
            start(cx, &format!("em-{pn}-{panel}"));
            use_preset(cx, preset);
            let (handle, view) = open(cx, two_tracks());
            dirty_name(cx, handle, "Zz");
            // 패널 바닥은 창 밖일 수 있어, 마지막 줄(테스트 노트 버튼/트랙 줄) 아래 빈 곳을 누른다
            let anchor = if panel == "inspector" {
                "inspector-test"
            } else {
                "track-row-1"
            };
            let b = cx
                .update_window(handle, |_, window, _| window.find(anchor).bounds())
                .unwrap();
            let at = gpui_kit::point(
                b.origin.x + gpui_kit::px(30.),
                b.origin.y + b.size.height + gpui_kit::px(60.),
            );
            gpui_kit::VisualTestContext::from_window(handle, cx)
                .simulate_click(at, gpui_kit::Modifiers::none());
            cx.run_until_parked();
            render(cx, handle);
            let c = ctx(cx, handle);
            assert_eq!(
                project_of(cx, &view).tracks()[0].name(),
                "Lead",
                "{pn} {panel}"
            );
            assert!(
                mute_toggles(cx, handle, &view, preset),
                "{pn} {panel} ctx={c}"
            );
            cx.update_window(handle, |_, window, _| window.remove_window())
                .unwrap();
        }
    }
}

/// 상태줄 모드 표시를 두 프리셋에서 왼쪽 클릭하면 메뉴가 열리고, 고르면 바뀐다.
#[gpui_kit::test]
fn status_mode_left_click_opens_menu_both_presets(cx: &mut TestAppContext) {
    for (preset, pn) in PRESETS {
        start(cx, &format!("sm-{pn}"));
        use_preset(cx, preset);
        let (handle, view) = open(cx, two_tracks());
        click(cx, handle, "status-mode");
        assert!(cx.read(|cx| view.read(cx).mode_menu_open()), "{pn}");
        cx.update_window(handle, |_, window, cx| {
            window
                .within("popup-menu")
                .click(gpui_kit::ElementId::Integer(1), cx);
        })
        .unwrap();
        cx.run_until_parked();
        render(cx, handle);
        assert_eq!(
            label(cx, handle, "status-mode").as_deref(),
            Some("STEP ♩ oct4"),
            "{pn}"
        );
        cx.update_window(handle, |_, window, _| window.remove_window())
            .unwrap();
    }
}

/// 프로그램 Select: 왼쪽 클릭으로 열고 검색 글자를 치고 고른다. 오른쪽 클릭 뒤에도 모드 키가 산다.
#[gpui_kit::test]
fn program_select_search_and_pick(cx: &mut TestAppContext) {
    start(cx, "prog-search");
    let (handle, view) = open(cx, two_tracks());
    let len = undo_len(cx, &view);
    click(cx, handle, "inspector-program");
    type_text(cx, handle, "Bright");
    press(cx, handle, "enter");
    assert_eq!(
        project_of(cx, &view).tracks()[0].program(),
        Some(keyscore_core::Program::new(1).unwrap())
    );
    assert_eq!(undo_len(cx, &view), len + 1);
    assert!(mute_toggles(cx, handle, &view, Preset::Default));
}
