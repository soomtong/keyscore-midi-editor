//! S3-T5 건반 뷰의 창 동작(REQ 5.4 「화면 하단에 2옥타브 건반」, 5.13, 7.1 하단 영역). 헤드리스 창, MIDI 없음.
//! 시험 목록은 `docs/reports/s3-t5/plan.md`.

use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::KeyLayout;
use keyscore_app::actions::view::ToggleBottomPanel;
use keyscore_app::app_state::AppSettings;
use keyscore_app::keyboard_view::KeyCap;
use keyscore_app::mode::{Mode, ModeEvent};
use keyscore_app::pianoroll::Cursor;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Edit, Key, Pitch, PitchClass, Project, Tick, Track, TrackColor};

const WAIT: Duration = Duration::from_millis(300);

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-kbd-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
}

/// 빈 트랙 하나(채널 `channel`).
fn project(channel: u8) -> Project {
    let mut project = Project::new();
    let name = if channel == 10 { "Drums" } else { "Lead" };
    project
        .push_track(Track::new(
            name,
            Channel::new(channel).unwrap(),
            TrackColor::new(0).unwrap(),
        ))
        .unwrap();
    project
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    for key in keys.split_whitespace() {
        cx.simulate_keystrokes(handle, key);
        cx.run_until_parked();
        render(cx, handle);
    }
}

fn wait(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.executor().advance_clock(WAIT);
    cx.run_until_parked();
    render(cx, handle);
}

fn event(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    e: ModeEvent,
) {
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |this, cx| this.apply_mode_event(e, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn set_layout(cx: &mut TestAppContext, handle: AnyWindowHandle, layout: KeyLayout) {
    cx.update(|cx| AppSettings::update(cx, |s| s.input.key_layout = layout));
    cx.run_until_parked();
    render(cx, handle);
}

fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
    cx.update_window(handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

/// 창을 연다(배치 뷰, Normal). `piano_roll`이면 피아노롤 뷰로 간다. 커서는 틱 0, C4.
fn open(
    cx: &mut TestAppContext,
    name: &str,
    p: Project,
    layout: KeyLayout,
    piano_roll: bool,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    set_layout(cx, handle, layout);
    if piano_roll {
        event(cx, handle, &view, ModeEvent::ToggleView);
    }
    cx.update(|cx| {
        view.read(cx).piano_roll().set_cursor(Cursor {
            tick: Tick::ZERO,
            pitch: Pitch::new(60).unwrap(),
        });
    });
    render(cx, handle);
    (handle, view)
}

/// 피아노롤 뷰의 Step 모드.
fn step(
    cx: &mut TestAppContext,
    name: &str,
    p: Project,
    layout: KeyLayout,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = open(cx, name, p, layout, true);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    (handle, view)
}

fn label(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &str) -> Option<String> {
    let id = gpui_kit::ElementId::Name(id.to_owned().into());
    cx.update_window(handle, move |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.label().map(str::to_owned))
    })
    .unwrap()
}

fn has(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> bool {
    cx.update_window(handle, |_, window, _| window.try_find(id).is_some())
        .unwrap()
}

fn key_label(cx: &mut TestAppContext, handle: AnyWindowHandle, i: usize) -> Option<String> {
    label(cx, handle, &format!("keyboard-key-{i}"))
}

fn shown(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<Vec<KeyCap>> {
    cx.read(|cx| view.read(cx).keyboard_view(cx))
}

fn octave(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> u8 {
    cx.read(|cx| view.read(cx).step_status().octave)
}

/// 트랙 0의 노트 음높이(시작 순서).
fn pitches(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<u8> {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        let mut v: Vec<(u64, u8)> = track
            .clips()
            .flat_map(|(_, c)| {
                c.notes()
                    .iter()
                    .map(move |n| (c.start().get() + n.start().get(), n.pitch().get()))
            })
            .collect();
        v.sort_unstable();
        v.into_iter().map(|(_, p)| p).collect()
    })
}

fn last_message(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> String {
    cx.read(|cx| {
        view.read(cx)
            .messages()
            .last()
            .map(ToString::to_string)
            .unwrap_or_default()
    })
}

// ---------- 6. 보이는 때 ----------

#[gpui_kit::test]
fn step_shows_keyboard_view_in_bottom_panel(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "shows", project(1), KeyLayout::Piano, true);
    assert!(!has(cx, handle, "keyboard-view"));
    assert_eq!(shown(cx, &view), None);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    assert!(has(cx, handle, "keyboard-view"));
    assert_eq!(shown(cx, &view).map(|k| k.len()), Some(24));
    assert_eq!(key_label(cx, handle, 0).as_deref(), Some("C4 A"));
    assert_eq!(
        label(cx, handle, "keyboard-octave").as_deref(),
        Some("oct4")
    );
    press(cx, handle, "escape");
    assert!(!has(cx, handle, "keyboard-view"));
}

// ---------- 7. 옥타브 키 ----------

#[gpui_kit::test]
fn keyboard_view_follows_octave_keys(cx: &mut TestAppContext) {
    let (handle, _view) = step(cx, "octave-keys", project(1), KeyLayout::Piano);
    press(cx, handle, "]");
    assert_eq!(key_label(cx, handle, 0).as_deref(), Some("C5 A"));
    assert_eq!(
        label(cx, handle, "keyboard-octave").as_deref(),
        Some("oct5")
    );
    press(cx, handle, "z z");
    assert_eq!(key_label(cx, handle, 0).as_deref(), Some("C3 A"));
    // 검은건반은 음 이름을 그리지 않지만 접근성 라벨에는 있다
    assert_eq!(key_label(cx, handle, 1).as_deref(), Some("C#3 W"));
}

// ---------- 8–10. 옥타브 버튼 ----------

#[gpui_kit::test]
fn octave_buttons_run_step_octave_actions(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "buttons", project(1), KeyLayout::Piano);
    click(cx, handle, "keyboard-octave-up");
    assert_eq!(octave(cx, &view), 5);
    // 포커스는 문서에 남아 다음 음 키가 Step으로 간다
    press(cx, handle, "a");
    wait(cx, handle);
    assert_eq!(pitches(cx, &view), vec![72]);
    click(cx, handle, "keyboard-octave-down");
    click(cx, handle, "keyboard-octave-down");
    assert_eq!(octave(cx, &view), 3);
    assert_eq!(key_label(cx, handle, 0).as_deref(), Some("C3 A"));
}

#[gpui_kit::test]
fn octave_buttons_have_action_labels(cx: &mut TestAppContext) {
    let (handle, _view) = step(cx, "button-labels", project(1), KeyLayout::Piano);
    assert_eq!(
        label(cx, handle, "keyboard-octave-up").as_deref(),
        Some("옥타브 올리기")
    );
    assert_eq!(
        label(cx, handle, "keyboard-octave-down").as_deref(),
        Some("옥타브 내리기")
    );
}

#[gpui_kit::test]
fn octave_button_settles_composing_like_key(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "settle", project(1), KeyLayout::Solfege);
    press(cx, handle, "c");
    assert!(cx.read(|cx| view.read(cx).step_status().composing.is_some()));
    click(cx, handle, "keyboard-octave-up");
    assert_eq!(pitches(cx, &view), vec![60]);
    assert_eq!(octave(cx, &view), 5);
    assert!(cx.read(|cx| view.read(cx).step_status().composing.is_none()));
}

#[gpui_kit::test]
fn octave_button_at_bound_says_range(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "bound", project(1), KeyLayout::Piano);
    press(cx, handle, "] ] ] ] ]");
    assert_eq!(octave(cx, &view), 9);
    press(cx, handle, "]");
    let by_key = last_message(cx, &view);
    click(cx, handle, "keyboard-octave-up");
    assert_eq!(octave(cx, &view), 9);
    assert_eq!(last_message(cx, &view), by_key);
    assert!(by_key.contains("0–9"), "{by_key}");
    // 127을 넘는 건반은 없음
    assert_eq!(key_label(cx, handle, 8).as_deref(), Some("없음"));
}

// ---------- 11–12. 배치 뷰, 하단 영역 ----------

#[gpui_kit::test]
fn keyboard_view_in_arrangement_step(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "arrangement", project(1), KeyLayout::Piano, false);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    assert!(has(cx, handle, "keyboard-view"));
    assert_eq!(key_label(cx, handle, 0).as_deref(), Some("C4 A"));
}

#[gpui_kit::test]
fn closed_bottom_panel_hides_keyboard_view(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "closed", project(1), KeyLayout::Piano);
    assert!(has(cx, handle, "keyboard-view"));
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(ToggleBottomPanel), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    assert!(!has(cx, handle, "keyboard-view"));
    assert_eq!(shown(cx, &view), None);
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(ToggleBottomPanel), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    assert!(has(cx, handle, "keyboard-view"));
}

// ---------- 13–15. 레이아웃·드럼·조성 ----------

#[gpui_kit::test]
fn layout_setting_change_updates_labels(cx: &mut TestAppContext) {
    let (handle, _view) = step(cx, "layout", project(1), KeyLayout::Piano);
    assert_eq!(key_label(cx, handle, 1).as_deref(), Some("C#4 W"));
    set_layout(cx, handle, KeyLayout::Solfege);
    assert_eq!(key_label(cx, handle, 0).as_deref(), Some("C4 c"));
    assert_eq!(key_label(cx, handle, 1).as_deref(), Some("C#4 c#"));
    assert_eq!(key_label(cx, handle, 12).as_deref(), Some("C5 c5"));
}

#[gpui_kit::test]
fn drum_track_shows_gm_labels(cx: &mut TestAppContext) {
    let (handle, _view) = step(cx, "drum", project(10), KeyLayout::Piano);
    press(cx, handle, "[ [");
    assert_eq!(key_label(cx, handle, 0).as_deref(), Some("bd A"));
    assert_eq!(key_label(cx, handle, 2).as_deref(), Some("sd S"));
    // 표에 없는 음(40)은 `n40`
    assert_eq!(key_label(cx, handle, 4).as_deref(), Some("n40 D"));
}

#[gpui_kit::test]
fn key_edit_updates_shading(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "shading", project(1), KeyLayout::Piano);
    assert!(!shown(cx, &view).unwrap()[5].out_of_key);
    cx.update(|cx| {
        view.read(cx).document().clone().update(cx, |d, cx| {
            d.apply(vec![Edit::SetKey(Key::Major(PitchClass::new(2).unwrap()))])
                .unwrap();
            cx.notify();
        });
    });
    cx.run_until_parked();
    render(cx, handle);
    let caps = shown(cx, &view).unwrap();
    assert!(caps[5].out_of_key, "F4");
    assert!(!caps[6].out_of_key, "F#4");
}

// ---------- 16. 라벨 = 실제 입력 ----------

/// 라벨이 있는 건반마다 그 라벨을 Step에 치면 그 건반 음이 써진다.
fn type_every_label(cx: &mut TestAppContext, name: &str, layout: KeyLayout) {
    let (handle, view) = step(cx, name, project(1), layout);
    let caps = shown(cx, &view).unwrap();
    let mut expected = Vec::new();
    for cap in &caps {
        let (Some(key), Some(pitch)) = (&cap.key, cap.pitch) else {
            continue;
        };
        // `c#5` → `c # 5`, 건반형 `A` → `a`
        let keys: Vec<String> = key.to_lowercase().chars().map(|c| c.to_string()).collect();
        press(cx, handle, &keys.join(" "));
        wait(cx, handle);
        expected.push(pitch.get());
        // 둘째 옥타브의 숫자 입력은 옥타브를 바꾼다. 다음 라벨을 위해 되돌린다
        if octave(cx, &view) != 4 {
            press(cx, handle, "[");
        }
        assert_eq!(octave(cx, &view), 4, "{key}");
    }
    assert_eq!(pitches(cx, &view), expected);
    assert!(caps.iter().all(|c| c.key.is_some()));
}

#[gpui_kit::test]
fn typed_labels_write_shown_pitches_piano(cx: &mut TestAppContext) {
    type_every_label(cx, "typed-piano", KeyLayout::Piano);
}

#[gpui_kit::test]
fn typed_labels_write_shown_pitches_solfege(cx: &mut TestAppContext) {
    type_every_label(cx, "typed-solfege", KeyLayout::Solfege);
}
