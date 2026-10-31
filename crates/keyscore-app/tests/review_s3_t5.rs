//! S3-T5 적대적 리뷰 1회차의 깨뜨리기 시험. 헬퍼는 `keyboard_view_ui.rs`와 같다.

use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::KeyLayout;
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

fn dispatch(cx: &mut TestAppContext, handle: AnyWindowHandle, action: Box<dyn gpui_kit::Action>) {
    cx.update_window(handle, |_, window, cx| window.dispatch_action(action, cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn lead_and_drums() -> Project {
    let mut p = project(1);
    p.push_track(Track::new(
        "Drums",
        Channel::new(10).unwrap(),
        TrackColor::new(1).unwrap(),
    ))
    .unwrap();
    p
}

// ---------- 1. 건반형 뒤 8건반: 음 키 + 옥타브 숫자로 닿지만 라벨은 없다 ----------

/// REQ 5.6: 건반 레이아웃도 뒤 숫자가 옥타브다. 건반형 oct4에서 `D` `5`는 E5(76)를 쓴다. 이 리뷰(S3-T5 리뷰 1)는
/// 처음 그 건반에 라벨을 요구했지만, 결정 `2026-10-31-keyboard-view-direct-labels.md` 1(S3-T5b)이 「키 하나로 닿는
/// 건반에만 라벨」로 바꿨다. 지금은 입력이 닿는 것과 라벨이 없는 것을 함께 고정한다.
#[gpui_kit::test]
fn piano_upper_keys_reachable_by_digit_have_no_label(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "r-piano-upper", project(1), KeyLayout::Piano);
    let caps = shown(cx, &view).unwrap();
    assert_eq!(caps[16].pitch.map(Pitch::get), Some(76));
    press(cx, handle, "d 5");
    wait(cx, handle);
    // 실제 입력은 E5를 쓴다
    assert_eq!(pitches(cx, &view), vec![76]);
    // 결정 2026-10-31-keyboard-view-direct-labels 1(S3-T5b): 키 하나로 닿지 않는 건반은 라벨 없음. 리뷰 1 지적 1의
    // 기대(라벨 있음)를 이 결정이 바꿨다
    assert!(
        caps[16..].iter().all(|c| c.key.is_none()),
        "건반형 뒤 8건반 라벨: {:?}",
        caps[16..].iter().map(|c| c.key.clone()).collect::<Vec<_>>()
    );
}

// ---------- 2. 텍스트 입력란에 포커스가 있을 때 옥타브 버튼 ----------

#[gpui_kit::test]
fn octave_button_with_tempo_field_focused(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "r-tempo-focus", project(1), KeyLayout::Piano);
    click(cx, handle, "transport-tempo");
    click(cx, handle, "keyboard-octave-up");
    assert_eq!(octave(cx, &view), 5);
    press(cx, handle, "a");
    wait(cx, handle);
    assert_eq!(pitches(cx, &view), vec![72]);
    assert_eq!(key_label(cx, handle, 0).as_deref(), Some("C5 A"));
}

// ---------- 3. 트랙 전환·채널 변경·undo ----------

#[gpui_kit::test]
fn track_switch_to_drum_updates_labels(cx: &mut TestAppContext) {
    let (handle, _view) = step(cx, "r-track-switch", lead_and_drums(), KeyLayout::Piano);
    press(cx, handle, "[ [");
    assert_eq!(key_label(cx, handle, 0).as_deref(), Some("C2 A"));
    dispatch(cx, handle, Box::new(keyscore_app::actions::track::Next));
    assert_eq!(key_label(cx, handle, 0).as_deref(), Some("bd A"));
    dispatch(cx, handle, Box::new(keyscore_app::actions::track::Prev));
    assert_eq!(key_label(cx, handle, 0).as_deref(), Some("C2 A"));
}

#[gpui_kit::test]
fn channel_change_and_undo_update_drum_labels(cx: &mut TestAppContext) {
    let (handle, _view) = step(cx, "r-channel-undo", project(1), KeyLayout::Piano);
    press(cx, handle, "[ [");
    dispatch(
        cx,
        handle,
        Box::new(keyscore_app::actions::track::SetChannel(
            keyscore_app::actions::args::Int(10),
        )),
    );
    assert_eq!(key_label(cx, handle, 0).as_deref(), Some("bd A"));
    dispatch(cx, handle, Box::new(keyscore_app::actions::edit::Undo));
    assert_eq!(key_label(cx, handle, 0).as_deref(), Some("C2 A"));
}

#[gpui_kit::test]
fn key_edit_undo_restores_shading(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "r-key-undo", project(1), KeyLayout::Piano);
    cx.update(|cx| {
        view.read(cx).document().clone().update(cx, |d, cx| {
            d.apply(vec![Edit::SetKey(Key::Major(PitchClass::new(2).unwrap()))])
                .unwrap();
            cx.notify();
        });
    });
    cx.run_until_parked();
    render(cx, handle);
    assert!(shown(cx, &view).unwrap()[5].out_of_key);
    dispatch(cx, handle, Box::new(keyscore_app::actions::edit::Undo));
    assert!(!shown(cx, &view).unwrap()[5].out_of_key);
}

// ---------- 4. 모드 전환 ----------

#[gpui_kit::test]
fn leaving_step_by_cycle_removes_buttons(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "r-cycle", project(1), KeyLayout::Piano);
    assert!(has(cx, handle, "keyboard-octave-up"));
    press(cx, handle, "ctrl-tab");
    assert_ne!(cx.read(|cx| view.read(cx).mode_state().mode()), Mode::Step);
    assert!(!has(cx, handle, "keyboard-octave-up"));
    assert!(!has(cx, handle, "keyboard-view"));
    assert!(has(cx, handle, "bottom-panel"));
}

// ---------- 5. 스티키 옥타브의 다른 길 ----------

#[gpui_kit::test]
fn mml_octave_updates_labels(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "r-mml", project(1), KeyLayout::Solfege);
    press(cx, handle, "o 6");
    assert_eq!(octave(cx, &view), 6);
    assert_eq!(key_label(cx, handle, 0).as_deref(), Some("C6 c"));
    // S3-T5b 결정: 둘째 옥타브는 키 하나로 닿지 않아 음 이름만
    assert_eq!(key_label(cx, handle, 12).as_deref(), Some("C7"));
}

#[gpui_kit::test]
fn piano_digit_suffix_moves_view_octave(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "r-piano-digit", project(1), KeyLayout::Piano);
    press(cx, handle, "a 5");
    assert_eq!(octave(cx, &view), 5);
    assert_eq!(
        label(cx, handle, "keyboard-octave").as_deref(),
        Some("oct5")
    );
    assert_eq!(key_label(cx, handle, 0).as_deref(), Some("C5 A"));
}

#[gpui_kit::test]
fn octave_button_with_mml_pending_matches_key(cx: &mut TestAppContext) {
    let (h1, v1) = step(cx, "r-mml-key", project(1), KeyLayout::Solfege);
    press(cx, h1, "o ]");
    let by_key = (octave(cx, &v1), last_message(cx, &v1));
    let (h2, v2) = step(cx, "r-mml-btn", project(1), KeyLayout::Solfege);
    press(cx, h2, "o");
    click(cx, h2, "keyboard-octave-up");
    let by_button = (octave(cx, &v2), last_message(cx, &v2));
    assert_eq!(by_key, by_button);
}

// ---------- 6. 화음 ----------

#[gpui_kit::test]
fn octave_button_closes_chord_like_key(cx: &mut TestAppContext) {
    let run = |cx: &mut TestAppContext, name: &str, button: bool| {
        let (h, v) = step(cx, name, project(1), KeyLayout::Piano);
        press(cx, h, "ctrl-a ctrl-d");
        wait(cx, h);
        if button {
            click(cx, h, "keyboard-octave-up");
        } else {
            press(cx, h, "]");
        }
        let status = cx.read(|cx| v.read(cx).step_status());
        let tick = cx.read(|cx| v.read(cx).piano_roll().cursor().tick);
        (status.chord_open, status.octave, tick, pitches(cx, &v))
    };
    let by_key = run(cx, "r-chord-key", false);
    let by_button = run(cx, "r-chord-btn", true);
    assert!(!by_key.0);
    assert_eq!(by_key, by_button);
}

// ---------- 7. 경계: 없음 건반은 실제로도 못 쓴다 ----------

#[gpui_kit::test]
fn absent_keys_cannot_be_written(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "r-absent", project(1), KeyLayout::Piano);
    press(cx, handle, "] ] ] ] ]");
    let caps = shown(cx, &view).unwrap();
    // 라벨이 있는 건반은 모두 0–127 안이고, `K`(C10)는 없음이다
    assert!(caps[12].pitch.is_none() && caps[12].key.is_none());
    press(cx, handle, "k");
    wait(cx, handle);
    assert!(pitches(cx, &view).is_empty());
    press(cx, handle, "g");
    wait(cx, handle);
    assert_eq!(pitches(cx, &view), vec![127]);
}

// ---------- 8. 드럼 트랙: 라벨 없는 음 ----------

/// REQ 5.13 「음이름 대신 GM 퍼커션 라벨」 「표에 없는 노트 번호는 `n<번호>`로 보인다」, 텍스트 내보내기
/// (`keyscore_text::export`)도 `n<번호>`를 쓴다. 건반 뷰는 그 음을 아무 이름 없이 비운다. 기본 옥타브 4의 드럼
/// 트랙에서 24건반 가운데 23개가 이름 없이 키 글자만 남는다.
#[gpui_kit::test]
fn drum_unlabeled_notes_show_number(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "r-drum-n", project(10), KeyLayout::Piano);
    let caps = shown(cx, &view).unwrap();
    let nameless = caps.iter().filter(|c| c.name.is_none()).count();
    assert_eq!(
        key_label(cx, handle, 0).as_deref(),
        Some("n60 A"),
        "이름 없는 건반 {nameless}개"
    );
}

#[gpui_kit::test]
fn drum_label_matches_written_note(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "r-drum-write", project(10), KeyLayout::Piano);
    press(cx, handle, "[ [");
    assert_eq!(key_label(cx, handle, 2).as_deref(), Some("sd S"));
    press(cx, handle, "s");
    wait(cx, handle);
    assert_eq!(pitches(cx, &view), vec![38]);
}
