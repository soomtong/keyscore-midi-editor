//! S3-T5b 적대적 리뷰 1회차의 깨뜨리기 시험. 헬퍼는 `keyboard_view_ui.rs`와 같다. 결정
//! `docs/decisions/2026-10-31-keyboard-view-direct-labels.md` 1–3.

use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, Pixels, TestAppContext};
use keyscore_app::actions::args::KeyLayout;
use keyscore_app::app_state::AppSettings;
use keyscore_app::keyboard_view::{KEY_MAX_HEIGHT, KeyCap, WHITE_KEY_MAX_WIDTH, keys};
use keyscore_app::mode::{Mode, ModeEvent};
use keyscore_app::pianoroll::Cursor;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{PROJECT_WINDOW_MIN_SIZE, init, open_project_window};
use keyscore_core::{Channel, Key, Pitch, Project, Tick, Track, TrackColor};

const WAIT: Duration = Duration::from_millis(300);
const PIANO: [&str; 16] = [
    "A", "W", "S", "E", "D", "F", "T", "G", "Y", "H", "U", "J", "K", "O", "L", "P",
];
const SOLFEGE: [&str; 12] = [
    "c", "c#", "d", "d#", "e", "f", "f#", "g", "g#", "a", "a#", "b",
];

// ---------- 픽스처 ----------

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

/// 피아노롤 뷰의 Step 모드. `bottom`이면 하단 영역 높이를 그 값으로 열고, 창을 `w`×`h`로 맞춘다.
fn step(
    cx: &mut TestAppContext,
    name: &str,
    p: Project,
    layout: KeyLayout,
    bottom: Option<f32>,
    window: Option<(f32, f32)>,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let dir = std::env::temp_dir().join(format!("ks-r5b-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    cx.update(|cx| {
        AppSettings::update(cx, |s| {
            s.input.key_layout = layout;
            if let Some(b) = bottom {
                s.layout.bottom_height = b;
            }
        });
    });
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    if let Some((w, h)) = window {
        cx.simulate_window_resize(handle, gpui_kit::size(gpui_kit::px(w), gpui_kit::px(h)));
        cx.run_until_parked();
    }
    render(cx, handle);
    event(cx, handle, &view, ModeEvent::ToggleView);
    cx.update(|cx| {
        view.read(cx).piano_roll().set_cursor(Cursor {
            tick: Tick::ZERO,
            pitch: Pitch::new(60).unwrap(),
        });
    });
    render(cx, handle);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    render(cx, handle);
    (handle, view)
}

fn shown(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<KeyCap> {
    cx.read(|cx| view.read(cx).keyboard_view(cx)).unwrap()
}

fn octave(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> u8 {
    cx.read(|cx| view.read(cx).step_status().octave)
}

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

fn bounds(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &str) -> Option<Bounds<Pixels>> {
    let id = gpui_kit::ElementId::Name(id.to_owned().into());
    cx.update_window(handle, move |_, window, _| {
        window.try_find(id).map(|e| e.bounds())
    })
    .unwrap()
}

fn key_bounds(cx: &mut TestAppContext, handle: AnyWindowHandle, i: usize) -> Bounds<Pixels> {
    bounds(cx, handle, &format!("keyboard-key-{i}")).unwrap()
}

/// (왼쪽, 위, 오른쪽, 아래)
fn edges(b: Bounds<Pixels>) -> (f32, f32, f32, f32) {
    let l = f32::from(b.origin.x);
    let t = f32::from(b.origin.y);
    (
        l,
        t,
        l + f32::from(b.size.width),
        t + f32::from(b.size.height),
    )
}

const EPS: f32 = 0.5;

// ---------- 결정 1: 모든 옥타브·두 레이아웃·드럼에서 라벨 자리 ----------

/// 리뷰 1 시도 1. `review_s3_t5_r2`의 모든 옥타브 시험은 이제 「라벨이 하나도 없어도」 통과한다. 그 빈자리를
/// 메운다: 옥타브 0–9, 두 레이아웃, 드럼 여부마다 라벨은 「레이아웃 표의 자리이고 그 음이 있다」와 정확히 같다.
#[test]
fn labels_exactly_on_direct_keys_every_octave_layout_drum() {
    for drum in [false, true] {
        for (layout, table) in [
            (KeyLayout::Piano, &PIANO[..]),
            (KeyLayout::Solfege, &SOLFEGE[..]),
        ] {
            for o in 0..=9u8 {
                let caps = keys(layout, o, Key::None, drum);
                for (i, cap) in caps.iter().enumerate() {
                    let want = (i < table.len() && cap.pitch.is_some()).then(|| table[i]);
                    assert_eq!(
                        cap.key.as_deref(),
                        want,
                        "{layout:?} oct{o} drum={drum} {i}"
                    );
                }
            }
        }
    }
}

/// 리뷰 1 시도 2. 드럼 트랙(채널 10)에서 모든 옥타브의 라벨을 치면 그 건반 음이 써지고, 친 라벨 수가 결정 1의
/// 수와 같다(구현자 시험은 멜로디 트랙만, 수는 옥타브 4만 센다).
fn type_every_label_every_octave_counted(
    cx: &mut TestAppContext,
    name: &str,
    channel: u8,
    layout: KeyLayout,
) {
    let (handle, view) = step(cx, name, project(channel), layout, None, None);
    let table = if layout == KeyLayout::Piano { 16 } else { 12 };
    let mut expected = Vec::new();
    for o in 0..=9u8 {
        go_octave(cx, handle, &view, o);
        let caps = shown(cx, &view);
        let mut typed = 0usize;
        for cap in &caps {
            let (Some(key), Some(pitch)) = (&cap.key, cap.pitch) else {
                continue;
            };
            let keys: Vec<String> = key.to_lowercase().chars().map(|c| c.to_string()).collect();
            press(cx, handle, &keys.join(" "));
            wait(cx, handle);
            expected.push(pitch.get());
            typed += 1;
            assert_eq!(pitches(cx, &view).last(), expected.last(), "oct{o} {key}");
            assert_eq!(
                octave(cx, &view),
                o,
                "키 하나 입력은 옥타브를 바꾸지 않는다: oct{o} {key}"
            );
        }
        let present = caps[..table].iter().filter(|c| c.pitch.is_some()).count();
        assert_eq!(typed, present, "oct{o} 라벨 수");
    }
    assert_eq!(pitches(cx, &view).len(), expected.len());
}

#[gpui_kit::test]
fn typed_labels_every_octave_drum_piano(cx: &mut TestAppContext) {
    type_every_label_every_octave_counted(cx, "drum-piano", 10, KeyLayout::Piano);
}

#[gpui_kit::test]
fn typed_labels_every_octave_drum_solfege(cx: &mut TestAppContext) {
    type_every_label_every_octave_counted(cx, "drum-solfege", 10, KeyLayout::Solfege);
}

#[gpui_kit::test]
fn typed_labels_every_octave_melody_piano_counted(cx: &mut TestAppContext) {
    type_every_label_every_octave_counted(cx, "mel-piano", 1, KeyLayout::Piano);
}

// ---------- 결정 3: 크기 상한 ----------

/// 리뷰 1 시도 3. 하단 영역을 높이 600으로 열어도 건반 높이는 120 이하다.
#[gpui_kit::test]
fn tall_bottom_panel_caps_key_height(cx: &mut TestAppContext) {
    let (handle, _view) = step(
        cx,
        "tall",
        project(1),
        KeyLayout::Piano,
        Some(600.),
        Some((1800., 1200.)),
    );
    let view = bounds(cx, handle, "keyboard-view").unwrap();
    assert!(f32::from(view.size.height) > 400., "하단 영역 {view:?}");
    for i in 0..24 {
        let (_, t, r, b) = edges(key_bounds(cx, handle, i));
        let h = b - t;
        assert!(h <= KEY_MAX_HEIGHT + EPS, "건반 {i} 높이 {h}");
        // 건반은 건반 뷰 안에 있다
        let (_, vt, vr, vb) = edges(view);
        assert!(t >= vt - EPS && b <= vb + EPS && r <= vr + EPS, "건반 {i}");
    }
}

/// 리뷰 1 시도 4. 넓은 창에서 판이 왼쪽 정렬되고(옥타브 버튼 바로 오른쪽), 검은건반이 앞 흰건반 경계에 가운데를
/// 맞춘다(판 너비 상한이 걸려도 `relative` 위치가 판 기준이다). 모든 흰건반이 40 px 이하로 같은 너비.
#[gpui_kit::test]
fn wide_window_board_left_aligned_and_black_keys_on_boundaries(cx: &mut TestAppContext) {
    let (handle, view) = step(
        cx,
        "wide",
        project(1),
        KeyLayout::Piano,
        None,
        Some((2400., 1000.)),
    );
    let caps = shown(cx, &view);
    let kv = edges(bounds(cx, handle, "keyboard-view").unwrap());
    let octave_label = edges(bounds(cx, handle, "keyboard-octave").unwrap());
    let first = edges(key_bounds(cx, handle, 0));
    // 왼쪽 정렬: 첫 건반은 옥타브 표시 바로 오른쪽(버튼 열 여백 이내)
    assert!(
        first.0 > octave_label.2 && first.0 - octave_label.2 < 60.,
        "{first:?} {octave_label:?}"
    );
    let mut prev_white_right = None;
    let mut last_white_right = 0.;
    for (i, cap) in caps.iter().enumerate() {
        let (l, _, r, _) = edges(key_bounds(cx, handle, i));
        if cap.black {
            let boundary: f32 = prev_white_right.unwrap();
            let center = f32::midpoint(l, r);
            assert!(
                (center - boundary).abs() <= 1.0,
                "검은건반 {i} 가운데 {center}, 경계 {boundary}"
            );
        } else {
            let w = r - l;
            assert!(
                (WHITE_KEY_MAX_WIDTH - 2.0..=WHITE_KEY_MAX_WIDTH + EPS).contains(&w),
                "흰건반 {i} 너비 {w}"
            );
            prev_white_right = Some(r);
            last_white_right = r;
        }
    }
    // 남는 자리는 오른쪽에 빈다
    assert!(
        kv.2 - last_white_right > 1000.,
        "오른쪽 빈자리 {}",
        kv.2 - last_white_right
    );
}

/// 리뷰 1 시도 5. 최소 창(720×480)에서 건반은 줄어들어도 건반 뷰 안에 있고, 검은건반이 판 밖으로 나가지 않는다.
#[gpui_kit::test]
fn min_window_keys_stay_inside_view(cx: &mut TestAppContext) {
    let min_w = f32::from(PROJECT_WINDOW_MIN_SIZE.width);
    let min_h = f32::from(PROJECT_WINDOW_MIN_SIZE.height);
    let (handle, view) = step(
        cx,
        "min",
        project(1),
        KeyLayout::Piano,
        None,
        Some((min_w, min_h)),
    );
    let caps = shown(cx, &view);
    let kv = edges(bounds(cx, handle, "keyboard-view").unwrap());
    let last_white = edges(key_bounds(cx, handle, 23));
    for (i, _) in caps.iter().enumerate() {
        let (left, top, right, bottom) = edges(key_bounds(cx, handle, i));
        assert!(
            left >= kv.0 - EPS && right <= kv.2 + EPS,
            "건반 {i} 가로 {left}..{right} / {kv:?}"
        );
        assert!(
            top >= kv.1 - EPS && bottom <= kv.3 + EPS,
            "건반 {i} 세로 {top}..{bottom} / {kv:?}"
        );
        assert!(
            right <= last_white.2 + EPS,
            "건반 {i}가 판 오른쪽 끝을 넘음"
        );
    }
}

// ---------- 결정 2: 키캡이 건반 안에 ----------

/// 키캡 상자와 음 이름 자리가 건반 안에 들어가는지. 넘치는 건반 번호를 돌려준다.
fn overflowing_keycaps(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    caps: &[KeyCap],
) -> Vec<String> {
    let mut out = Vec::new();
    let mut found = 0usize;
    for (i, cap) in caps.iter().enumerate() {
        let Some(kc) = bounds(cx, handle, &format!("keyboard-keycap-{i}")) else {
            continue;
        };
        found += 1;
        // 크기가 0인 키캡은 넘치지 않는 것처럼 보인다. 그려졌는지 함께 본다
        assert!(
            f32::from(kc.size.width) >= 16. - EPS && f32::from(kc.size.height) > 0.,
            "키캡 {i} {kc:?}"
        );
        let (kl, kt, kr, kb) = edges(key_bounds(cx, handle, i));
        let (cl, ct, cr, cb) = edges(kc);
        if cl < kl - EPS || cr > kr + EPS || ct < kt - EPS || cb > kb + EPS {
            out.push(format!(
                "{i}({:?} {}) 키캡 {cl:.1}..{cr:.1}×{ct:.1}..{cb:.1} 건반 {kl:.1}..{kr:.1}×{kt:.1}..{kb:.1}",
                cap.key,
                if cap.black { "black" } else { "white" }
            ));
        }
    }
    assert_eq!(
        found,
        caps.iter().filter(|c| c.key.is_some()).count(),
        "그려진 키캡 수"
    );
    out
}

/// 리뷰 1 시도 6. 기본 창·상한 크기에서 키캡이 건반(특히 검은건반, 계이름형 두 글자 `c#`) 안에 들어간다.
#[gpui_kit::test]
fn keycaps_fit_inside_keys_default_window(cx: &mut TestAppContext) {
    for (name, layout) in [
        ("fit-piano", KeyLayout::Piano),
        ("fit-solfege", KeyLayout::Solfege),
    ] {
        let (handle, view) = step(cx, name, project(1), layout, None, Some((1600., 900.)));
        let caps = shown(cx, &view);
        let bad = overflowing_keycaps(cx, handle, &caps);
        assert!(bad.is_empty(), "{layout:?}: {bad:#?}");
    }
}

/// 리뷰 1 시도 7. 하단 영역 최소 높이(120)·최소 창에서 키캡이 건반 안에 들어간다.
#[gpui_kit::test]
fn keycaps_fit_inside_keys_low_bottom_min_window(cx: &mut TestAppContext) {
    let w = f32::from(PROJECT_WINDOW_MIN_SIZE.width);
    let h = f32::from(PROJECT_WINDOW_MIN_SIZE.height);
    for (name, layout) in [
        ("low-piano", KeyLayout::Piano),
        ("low-solfege", KeyLayout::Solfege),
    ] {
        let (handle, view) = step(cx, name, project(1), layout, Some(120.), Some((w, h)));
        let caps = shown(cx, &view);
        let bad = overflowing_keycaps(cx, handle, &caps);
        assert!(bad.is_empty(), "{layout:?}: {bad:#?}");
    }
}
