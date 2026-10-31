//! S3-T5 적대적 리뷰 2회차의 깨뜨리기 시험. 헬퍼는 `keyboard_view_ui.rs`·`review_s3_t5.rs`와 같다.

use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::KeyLayout;
use keyscore_app::app_state::AppSettings;
use keyscore_app::keyboard_view::KeyCap;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, ModeEvent};
use keyscore_app::pianoroll::Cursor;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Pitch, Project, Tick, Track, TrackColor};

const WAIT: Duration = Duration::from_millis(300);

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-r2-{}-{name}", std::process::id()));
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

/// 옥타브를 `to`로 맞춘다(키 `[` `]`).
fn go_octave(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    to: u8,
) {
    while octave(cx, view) > to {
        press(cx, handle, "[");
    }
    while octave(cx, view) < to {
        press(cx, handle, "]");
    }
}

/// 옥타브 0–9 모두에서 라벨이 있는 건반마다 그 라벨을 치면 그 건반 음이 써진다(구현자 시험 16은 옥타브 4만).
fn type_every_label_every_octave(cx: &mut TestAppContext, name: &str, layout: KeyLayout) {
    let (handle, view) = step(cx, name, project(1), layout);
    let mut expected = Vec::new();
    for o in 0..=9u8 {
        go_octave(cx, handle, &view, o);
        let caps = shown(cx, &view).unwrap();
        for cap in &caps {
            let (Some(key), Some(pitch)) = (&cap.key, cap.pitch) else {
                // 없는 건반에는 라벨이 없다. 있는 건반도 키 하나로 닿지 않으면 라벨이 없다(S3-T5b 결정)
                assert!(cap.pitch.is_some() || cap.key.is_none(), "oct{o} {cap:?}");
                continue;
            };
            let keys: Vec<String> = key.to_lowercase().chars().map(|c| c.to_string()).collect();
            press(cx, handle, &keys.join(" "));
            wait(cx, handle);
            expected.push(pitch.get());
            let got = pitches(cx, &view);
            assert_eq!(
                got.len(),
                expected.len(),
                "oct{o} {key}: {}",
                last_message(cx, &view)
            );
            assert_eq!(got.last(), expected.last(), "oct{o} {key}");
            go_octave(cx, handle, &view, o);
        }
    }
}

// ---------- 1–2. 모든 옥타브에서 라벨 = 실제 입력 ----------

#[gpui_kit::test]
fn typed_labels_write_shown_pitches_every_octave_piano(cx: &mut TestAppContext) {
    type_every_label_every_octave(cx, "r2-every-piano", KeyLayout::Piano);
}

#[gpui_kit::test]
fn typed_labels_write_shown_pitches_every_octave_solfege(cx: &mut TestAppContext) {
    type_every_label_every_octave(cx, "r2-every-solfege", KeyLayout::Solfege);
}

// ---------- 옥타브 8·9 경계 ----------

#[gpui_kit::test]
fn piano_octave_eight_upper_keys_unlabeled_and_stop_at_127(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "r2-oct8", project(1), KeyLayout::Piano);
    go_octave(cx, handle, &view, 8);
    let caps = shown(cx, &view).unwrap();
    let keys: Vec<Option<&str>> = caps[16..].iter().map(|c| c.key.as_deref()).collect();
    // S3-T5b 결정: 키 하나로 닿지 않는 건반은 라벨 없음
    assert_eq!(keys, vec![None; 8]);
    assert_eq!(caps[19].pitch.map(Pitch::get), Some(127));
}

// ---------- 3. 접근성 라벨 = 모델 ----------

#[gpui_kit::test]
fn accessible_labels_match_model_for_every_key(cx: &mut TestAppContext) {
    for (name, layout) in [
        ("r2-a11y-p", KeyLayout::Piano),
        ("r2-a11y-s", KeyLayout::Solfege),
    ] {
        let (handle, view) = step(cx, name, project(1), layout);
        for o in [0u8, 4, 8, 9] {
            go_octave(cx, handle, &view, o);
            let caps = shown(cx, &view).unwrap();
            for (i, cap) in caps.iter().enumerate() {
                let want = if cap.pitch.is_none() {
                    "없음".to_owned()
                } else {
                    [cap.name.as_deref(), cap.key.as_deref()]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(" ")
                };
                assert_eq!(key_label(cx, handle, i), Some(want), "oct{o} {i}");
            }
        }
    }
}

// ---------- 7. 라벨 입력 뒤 건반 뷰가 옮겨 간다 ----------

#[gpui_kit::test]
fn typed_digit_input_moves_view_and_labels_reached_key(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "r2-move", project(1), KeyLayout::Piano);
    let before = shown(cx, &view).unwrap();
    // S3-T5b 결정: E5 건반에는 라벨이 없다(`D` `5`로 닿지만 키 하나가 아니다)
    assert_eq!(before[16].key, None);
    press(cx, handle, "d 5");
    wait(cx, handle);
    assert_eq!(pitches(cx, &view), vec![76]);
    assert_eq!(
        label(cx, handle, "keyboard-octave").as_deref(),
        Some("oct5")
    );
    let after = shown(cx, &view).unwrap();
    // 방금 쓴 E5는 이제 첫 옥타브 `D`
    assert_eq!(after[4].pitch.map(Pitch::get), Some(76));
    assert_eq!(after[4].key.as_deref(), Some("D"));
    assert_eq!(key_label(cx, handle, 4).as_deref(), Some("E5 D"));
}

// ---------- 5. 배치 뷰·드럼 트랙의 옥타브 버튼 ----------

#[gpui_kit::test]
fn arrangement_step_octave_button_then_key(cx: &mut TestAppContext) {
    let (handle, view) = open(cx, "r2-arr-btn", project(1), KeyLayout::Piano, false);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    click(cx, handle, "keyboard-octave-up");
    assert_eq!(octave(cx, &view), 5);
    press(cx, handle, "a");
    wait(cx, handle);
    assert_eq!(pitches(cx, &view), vec![72]);
    click(cx, handle, "keyboard-octave-down");
    click(cx, handle, "keyboard-octave-down");
    assert_eq!(octave(cx, &view), 3);
    assert_eq!(key_label(cx, handle, 0).as_deref(), Some("C3 A"));
}

#[gpui_kit::test]
fn drum_track_octave_button_then_label(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "r2-drum-btn", project(10), KeyLayout::Piano);
    click(cx, handle, "keyboard-octave-down");
    click(cx, handle, "keyboard-octave-down");
    assert_eq!(key_label(cx, handle, 0).as_deref(), Some("bd A"));
    assert_eq!(key_label(cx, handle, 4).as_deref(), Some("n40 D"));
    press(cx, handle, "d");
    wait(cx, handle);
    assert_eq!(pitches(cx, &view), vec![40]);
}

// ---------- 6. 건반형 조합 중 버튼 = 키 ----------

fn compose_then(cx: &mut TestAppContext, name: &str, button: bool) -> (Vec<u8>, u8, String) {
    let (handle, view) = step(cx, name, project(1), KeyLayout::Piano);
    press(cx, handle, "d");
    if button {
        click(cx, handle, "keyboard-octave-up");
    } else {
        press(cx, handle, "]");
    }
    wait(cx, handle);
    press(cx, handle, "a");
    wait(cx, handle);
    (
        pitches(cx, &view),
        octave(cx, &view),
        last_message(cx, &view),
    )
}

#[gpui_kit::test]
fn octave_button_while_piano_composing_matches_key(cx: &mut TestAppContext) {
    let by_key = compose_then(cx, "r2-comp-key", false);
    let by_button = compose_then(cx, "r2-comp-btn", true);
    assert_eq!(by_key.0, vec![64, 72]);
    assert_eq!(by_button, by_key);
}

// ---------- 9. vim 프리셋 ----------

#[gpui_kit::test]
fn typed_labels_under_vim_preset(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "r2-vim", project(1), KeyLayout::Piano);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    cx.run_until_parked();
    render(cx, handle);
    let caps = shown(cx, &view).unwrap();
    let mut expected = Vec::new();
    for cap in &caps {
        let (Some(key), Some(pitch)) = (&cap.key, cap.pitch) else {
            continue;
        };
        let keys: Vec<String> = key.to_lowercase().chars().map(|c| c.to_string()).collect();
        press(cx, handle, &keys.join(" "));
        wait(cx, handle);
        expected.push(pitch.get());
        go_octave(cx, handle, &view, 4);
    }
    assert_eq!(pitches(cx, &view), expected);
}

// ---------- 8. 건반형 윗줄 + 숫자의 뜻 ----------

/// 결정 `2026-10-29-s3-t4-step-input-details.md` 2절(사람 결정): 건반 레이아웃의 뒤 숫자는 줄 기준 옥타브다
/// (oct4 `K` 뒤 `3` = C4). `K` `5`는 C6이고 그 뒤 건반 뷰는 oct5, C6 건반의 라벨은 `K`.
#[gpui_kit::test]
fn piano_upper_row_key_with_digit_follows_row_octave(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "r2-k5", project(1), KeyLayout::Piano);
    press(cx, handle, "k 5");
    wait(cx, handle);
    assert_eq!(pitches(cx, &view), vec![84], "K 5");
    let caps = shown(cx, &view).unwrap();
    assert_eq!(caps[12].pitch.map(Pitch::get), Some(84));
    assert_eq!(caps[12].key.as_deref(), Some("K"));
}

// ---------- 7. Step 안에서 뷰 전환 ----------

#[gpui_kit::test]
fn view_toggle_in_step_keeps_view_consistent_with_mode(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "r2-toggle", project(1), KeyLayout::Piano);
    event(cx, handle, &view, ModeEvent::ToggleView);
    let in_step = cx.read(|cx| view.read(cx).mode_state().mode() == Mode::Step);
    assert_eq!(has(cx, handle, "keyboard-view"), in_step);
    assert_eq!(shown(cx, &view).is_some(), in_step);
    assert_eq!(!has(cx, handle, "bottom-panel"), in_step);
}

// ---------- 옥타브 대기 0 ms(설정 파일로만 가능)에서 둘째 옥타브 라벨 ----------

/// 설정 `input.octave_wait_ms`는 0–2000을 받는다(`settings.rs`). 0이면 음 키가 곧바로 확정돼 뒤 숫자가 옥타브가
/// 되지 못한다. 그때 `D` `5`를 치면 E4가 써지고 `5`는 길이 코드(16분음표)가 된다. S3-T5b 결정으로 E5 건반에는
/// 라벨이 없으므로 건반 뷰가 이 입력을 약속하지 않는다(리뷰 2 제안 2는 이 결정으로 해소된다).
#[gpui_kit::test]
fn zero_octave_wait_upper_label_commits_before_digit(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "r2-wait0", project(1), KeyLayout::Piano);
    cx.update(|cx| AppSettings::update(cx, |s| s.input.octave_wait_ms = 0));
    cx.run_until_parked();
    render(cx, handle);
    let caps = shown(cx, &view).unwrap();
    assert_eq!(caps[16].key, None);
    press(cx, handle, "d 5");
    wait(cx, handle);
    assert_eq!(pitches(cx, &view), vec![64], "D 5");
}
