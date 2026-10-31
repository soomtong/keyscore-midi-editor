//! S3-T5c 적대적 리뷰 2회차 시험(`docs/reports/s3-t5c/review-2.md`). 헤드리스 창, MIDI 없음.
//! 헬퍼는 `review_s3_t5c.rs`와 같은 꼴이다.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::KeyLayout;
use keyscore_app::app_state::AppSettings;
use keyscore_app::keyboard_view::KeyCap;
use keyscore_app::mode::{Mode, ModeEvent};
use keyscore_app::pianoroll::Cursor;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Pitch, Project, Tick, Track, TrackColor};

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-rv-t5c-r2-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
}

fn one_track(channel: u8) -> Project {
    let mut p = Project::new();
    p.push_track(Track::new(
        "T",
        Channel::new(channel).unwrap(),
        TrackColor::new(0).unwrap(),
    ))
    .unwrap();
    p
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

/// 피아노롤 뷰의 Step 모드. `size`가 있으면 창 크기를 바꾼다.
fn step(
    cx: &mut TestAppContext,
    name: &str,
    p: Project,
    layout: KeyLayout,
    size: Option<(f32, f32)>,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| AppSettings::update(cx, |s| s.input.key_layout = layout));
    cx.run_until_parked();
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
    if let Some((w, h)) = size {
        cx.simulate_window_resize(handle, gpui_kit::size(gpui_kit::px(w), gpui_kit::px(h)));
        cx.run_until_parked();
        render(cx, handle);
    }
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

/// 건반 `i`의 (건반 너비, 그린 이름 너비, 접근성 라벨).
fn key_info(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    i: usize,
) -> (f32, f32, Option<String>) {
    let key_id = gpui_kit::ElementId::Name(format!("keyboard-key-{i}").into());
    let name_id = gpui_kit::ElementId::Name(format!("keyboard-name-{i}").into());
    cx.update_window(handle, move |_, window, _| {
        let key = window.find(key_id);
        let name = window
            .try_find(name_id)
            .map_or(0.0, |e| f32::from(e.bounds().size.width));
        (
            f32::from(key.bounds().size.width),
            name,
            key.label().map(str::to_owned),
        )
    })
    .unwrap()
}

fn expected_label(cap: &KeyCap) -> String {
    if cap.pitch.is_none() {
        return "없음".to_owned();
    }
    [cap.name.as_deref(), cap.key.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ")
}

const WINDOWS: [(&str, Option<(f32, f32)>); 4] = [
    ("default", None),
    ("wide", Some((2400., 1400.))),
    ("mid", Some((1000., 700.))),
    ("min", Some((720., 480.))),
];

const LAYOUTS: [(&str, KeyLayout); 2] = [("p", KeyLayout::Piano), ("s", KeyLayout::Solfege)];

/// 옥타브 0–9, 흰건반·검은건반 모두에서 그린 이름이 「글 + 4 px ≤ 건반」인지(구현 시험과 같은 기준).
fn all_names_fit(cx: &mut TestAppContext, channel: u8) {
    let mut bad = Vec::new();
    for (wname, size) in WINDOWS {
        for (lname, layout) in LAYOUTS {
            let (handle, view) = step(
                cx,
                &format!("fit-{channel}-{wname}-{lname}"),
                one_track(channel),
                layout,
                size,
            );
            for o in 0..=9u8 {
                go_octave(cx, handle, &view, o);
                let caps = shown(cx, &view);
                for (i, cap) in caps.iter().enumerate() {
                    let (kw, nw, _) = key_info(cx, handle, i);
                    if nw > 0.5 && nw + 4.0 > kw {
                        bad.push(format!(
                            "{wname} {lname} oct{o} {i} black={} {:?}: 글 {nw} 건반 {kw}",
                            cap.black, cap.name
                        ));
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

// ---------- 1. 넘침: 모든 창 크기 × 레이아웃 × 옥타브 × 흰·검은건반 ----------

#[gpui_kit::test]
fn drum_names_fit_every_key_window_layout_octave(cx: &mut TestAppContext) {
    all_names_fit(cx, 10);
}

#[gpui_kit::test]
fn melody_names_fit_every_key_window_layout_octave(cx: &mut TestAppContext) {
    all_names_fit(cx, 1);
}

// ---------- 2. 그리지 않는 이름도 접근성 라벨에 남는다(드럼, 모든 옥타브) ----------

/// 드럼 검은건반: 세 글자 이하면 그리고 네 글자면 그리지 않는다. 흰건반 드럼 라벨은 길이와 무관하게 그린다.
/// 접근성 라벨은 그리기와 무관하게 「이름 키」 그대로다. 구현 보고서는 이 근거로 `review_s3_t5_r2`의
/// `accessible_labels_match_model_for_every_key`를 들었지만 그 시험은 멜로디(채널 1)만 본다.
#[gpui_kit::test]
fn drum_hidden_black_names_keep_accessible_label(cx: &mut TestAppContext) {
    let mut hidden = 0usize;
    let mut white_four = 0usize;
    let mut black_three = 0usize;
    for (lname, layout) in LAYOUTS {
        let (handle, view) = step(cx, &format!("a11y-{lname}"), one_track(10), layout, None);
        for o in 0..=9u8 {
            go_octave(cx, handle, &view, o);
            let caps = shown(cx, &view);
            for (i, cap) in caps.iter().enumerate() {
                let (_, nw, label) = key_info(cx, handle, i);
                assert_eq!(label, Some(expected_label(cap)), "{lname} oct{o} {i}");
                let Some(name) = cap.name.as_deref() else {
                    assert!(nw < 0.5, "{lname} oct{o} {i}: 이름 없는 건반에 글");
                    continue;
                };
                let len = name.chars().count();
                let drawn = nw > 0.5;
                if cap.black && len > 3 {
                    assert!(!drawn, "{lname} oct{o} {i} {name}: 네 글자를 그렸다");
                    hidden += 1;
                } else {
                    assert!(drawn, "{lname} oct{o} {i} {name}: 그려야 할 이름이 없다");
                    if cap.black {
                        black_three += usize::from(len == 3);
                    } else {
                        white_four += usize::from(len == 4);
                    }
                }
            }
        }
    }
    // 시험이 빈 집합을 보지 않았음을 확인한다
    assert!(
        hidden > 0 && white_four > 0 && black_three > 0,
        "{hidden} {white_four} {black_three}"
    );
}
