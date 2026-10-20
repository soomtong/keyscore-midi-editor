//! S2-T6f 적대적 리뷰 3회차. 리뷰 2 대응(`baseline`, `fit_side_panels`의 패널 최소)을 깨뜨리려는 시도.

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext, px, size};
use keyscore_app::actions::view::ToggleInspector;
use keyscore_app::app_state::AppSettings;
use keyscore_app::settings::{LayoutSettings, Settings};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::ksq;

fn dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("ks-review-s2t6f-r3-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn start_with(cx: &mut TestAppContext, name: &str, edit: impl FnOnce(&mut LayoutSettings)) {
    let dir = dir(name);
    let path = dir.join("settings.json");
    let mut s = Settings::default();
    edit(&mut s.layout);
    s.save(&path).unwrap();
    cx.update(|cx| init(cx, path));
}

fn one_track() -> keyscore_core::Project {
    ksq::from_str(r#"{"tracks": [{"name": "A", "channel": 1}]}"#)
        .unwrap()
        .0
}

fn draw(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    for _ in 0..3 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
    }
}

fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx
        .update(|cx| open_project_window(one_track(), cx))
        .unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    draw(cx, handle);
    (handle, view)
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl gpui_kit::Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    draw(cx, handle);
}

fn region(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> (f32, f32) {
    cx.update_window(handle, |_, window, _| {
        let b = window.find(id).bounds();
        (f32::from(b.origin.x), f32::from(b.size.width))
    })
    .unwrap()
}

fn resize_panel(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    vertical: bool,
    index: usize,
    to: f32,
) {
    let state = cx.read(|cx| {
        if vertical {
            view.read(cx).vertical_state().clone()
        } else {
            view.read(cx).horizontal_state().clone()
        }
    });
    cx.update_window(handle, |_, window, cx| {
        state.update(cx, |s, cx| s.resize_panel(index, px(to), window, cx));
        window.render_frame(cx);
    })
    .unwrap();
    draw(cx, handle);
}

fn sizes(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<f32> {
    cx.read(|cx| {
        view.read(cx)
            .horizontal_state()
            .read(cx)
            .sizes()
            .iter()
            .map(|p| f32::from(*p))
            .collect()
    })
}

fn saved(cx: &mut TestAppContext) -> LayoutSettings {
    cx.read(|cx| AppSettings::get(cx).layout)
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= 2.0
}

fn squeezed(
    cx: &mut TestAppContext,
    name: &str,
    w: f32,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start_with(cx, name, |l| {
        l.track_list_width = 500.0;
        l.inspector_width = 500.0;
    });
    let (handle, view) = open(cx);
    cx.simulate_window_resize(handle, size(px(1600.0), px(900.0)));
    draw(cx, handle);
    cx.simulate_window_resize(handle, size(px(w), px(700.0)));
    draw(cx, handle);
    (handle, view)
}

/// 창 300(남는 자리 180, 창 최소 720보다 좁아 시험 플랫폼에서만 닿는다)이면 두 패널이 다 최소 120으로 요청되어
/// 넘친다. gpui-base 크기 상태는 컨테이너에 맞춰 비율로 줄어(약 106·88·106) 기준(120·120)과 어긋난다. 인스펙터를
/// 넓혀도 그린 너비는 120 그대로이고 저장값도 그대로다. 이어서 트랙 목록을 넓히면 저장값이 120으로 바뀌는
/// 것은 리뷰 3 제안 1(창 최소 아래라 실제로는 닿지 않음)로 적고 여기서는 첫 조절만 본다.
#[gpui_kit::test]
fn overflow_both_min_inspector_drag_keeps_saved_widths(cx: &mut TestAppContext) {
    let (handle, view) = squeezed(cx, "overflow", 300.0);
    let before = sizes(cx, &view);
    resize_panel(cx, handle, &view, false, 2, before[2] + 30.0);
    let after = sizes(cx, &view);
    let drawn = (
        region(cx, handle, "track-list"),
        region(cx, handle, "inspector"),
    );
    let s = saved(cx);
    assert!(
        close(s.track_list_width, 500.0) && close(s.inspector_width, 500.0),
        "{before:?} → {after:?}(그린 {drawn:?}), 설정 {s:?}"
    );
}

/// 창 최소 너비 720에서 저장 200·1200이면 트랙 목록 120·인스펙터 480으로 요청된다(리뷰 2 제안 3). 메인은
/// 120 이상이고, 인스펙터 5 줄이기와 트랙 목록 10 넓히기가 각각 그 패널만 저장된다.
#[gpui_kit::test]
fn min_window_skewed_widths_small_drags_are_saved(cx: &mut TestAppContext) {
    start_with(cx, "min-window", |l| {
        l.track_list_width = 200.0;
        l.inspector_width = 1200.0;
    });
    let (handle, view) = open(cx);
    cx.simulate_window_resize(handle, size(px(1600.0), px(900.0)));
    draw(cx, handle);
    cx.simulate_window_resize(handle, size(px(720.0), px(600.0)));
    draw(cx, handle);
    let (_, track) = region(cx, handle, "track-list");
    let (_, main) = region(cx, handle, "main-view");
    let before = sizes(cx, &view);
    let target = before[2] - 5.0;
    resize_panel(cx, handle, &view, false, 2, target);
    let a = saved(cx);
    let mid = sizes(cx, &view);
    resize_panel(cx, handle, &view, false, 0, mid[0] + 10.0);
    let s = saved(cx);
    assert!(
        close(track, 120.0)
            && main >= 118.0
            && close(a.inspector_width, target)
            && close(a.track_list_width, 200.0)
            && close(s.track_list_width, mid[0] + 10.0)
            && close(s.inspector_width, target),
        "그린 트랙 {track}, 메인 {main}, {before:?} → {mid:?}({a:?}) → {:?}({s:?})",
        sizes(cx, &view)
    );
}

/// 줄어 보이는 창에서 아래 패널을 조절한 뒤 인스펙터를 20 줄이면 인스펙터만 저장된다.
#[gpui_kit::test]
fn vertical_then_horizontal_while_squeezed(cx: &mut TestAppContext) {
    let (handle, view) = squeezed(cx, "vert-then-horiz", 900.0);
    resize_panel(cx, handle, &view, true, 0, 400.0);
    let before = sizes(cx, &view);
    let target = before[2] - 20.0;
    resize_panel(cx, handle, &view, false, 2, target);
    let s = saved(cx);
    assert!(
        close(s.inspector_width, target) && close(s.track_list_width, 500.0),
        "전 {before:?}, 뒤 {:?}, 설정 {s:?}",
        sizes(cx, &view)
    );
}

/// 줄어 보이는 창에서 인스펙터를 접고 트랙 목록을 조절, 다시 펴서 인스펙터를 줄인다.
#[gpui_kit::test]
fn collapse_drag_expand_drag_while_squeezed(cx: &mut TestAppContext) {
    let (handle, view) = squeezed(cx, "collapse-drag", 700.0);
    run(cx, handle, ToggleInspector);
    resize_panel(cx, handle, &view, false, 0, 450.0);
    let mid = saved(cx);
    run(cx, handle, ToggleInspector);
    let before = sizes(cx, &view);
    let target = before[2] - 10.0;
    resize_panel(cx, handle, &view, false, 2, target);
    let s = saved(cx);
    assert!(
        close(mid.track_list_width, 450.0)
            && close(s.track_list_width, 450.0)
            && close(s.inspector_width, target),
        "접고 조절 뒤 {mid:?}, 편 뒤 {before:?}, 설정 {s:?}"
    );
}

/// 줄어 보이는 창에서 트랙 목록을 넓혀 인스펙터를 밀고(메인 최소), 이어 트랙 목록을 줄이고, 인스펙터를 줄인다.
#[gpui_kit::test]
fn push_then_shrink_then_inspector_while_squeezed(cx: &mut TestAppContext) {
    let (handle, view) = squeezed(cx, "push-shrink", 900.0);
    let first = sizes(cx, &view);
    resize_panel(cx, handle, &view, false, 0, first[0] + 30.0);
    let a = saved(cx);
    let mid = sizes(cx, &view);
    resize_panel(cx, handle, &view, false, 0, mid[0] - 50.0);
    let b = saved(cx);
    let mid2 = sizes(cx, &view);
    let target = mid2[2] - 15.0;
    resize_panel(cx, handle, &view, false, 2, target);
    let s = saved(cx);
    assert!(
        close(a.inspector_width, 500.0)
            && close(b.track_list_width, mid2[0])
            && close(b.inspector_width, 500.0)
            && close(s.inspector_width, target)
            && close(s.track_list_width, mid2[0]),
        "{first:?} → {mid:?}({a:?}) → {mid2:?}({b:?}) → {:?}({s:?})",
        sizes(cx, &view)
    );
}

/// 창 크기를 바꾼 직후(그리기 한 번) 조절해도 끈 패널만 저장된다.
#[gpui_kit::test]
fn drag_right_after_resize_single_frame(cx: &mut TestAppContext) {
    start_with(cx, "single-frame", |l| {
        l.track_list_width = 500.0;
        l.inspector_width = 500.0;
    });
    let (handle, view) = open(cx);
    cx.simulate_window_resize(handle, size(px(900.0), px(700.0)));
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    let before = sizes(cx, &view);
    let target = before[2] - 20.0;
    resize_panel(cx, handle, &view, false, 2, target);
    let s = saved(cx);
    assert!(
        close(s.inspector_width, target) && close(s.track_list_width, 500.0),
        "전 {before:?}, 설정 {s:?}"
    );
}

/// 한쪽만 펴진 좁은 창에서 두 번 조절하면 둘 다 저장된다.
#[gpui_kit::test]
fn only_track_open_two_drags(cx: &mut TestAppContext) {
    start_with(cx, "only-track", |l| {
        l.track_list_width = 1000.0;
        l.inspector_width = 300.0;
    });
    let (handle, view) = open(cx);
    run(cx, handle, ToggleInspector);
    cx.simulate_window_resize(handle, size(px(800.0), px(600.0)));
    draw(cx, handle);
    let (_, w) = region(cx, handle, "track-list");
    resize_panel(cx, handle, &view, false, 0, 500.0);
    let a = saved(cx);
    resize_panel(cx, handle, &view, false, 0, 450.0);
    let s = saved(cx);
    assert!(
        close(w, 680.0)
            && close(a.track_list_width, 500.0)
            && close(s.track_list_width, 450.0)
            && close(s.inspector_width, 300.0),
        "그린 {w}, {a:?}, {s:?}"
    );
}

/// 다른 창이 저장값을 바꾼 뒤 좁은 창에서 인스펙터를 조절해도 기준이 그린 너비라 인스펙터만 저장된다.
#[gpui_kit::test]
fn other_window_save_then_squeezed_drag(cx: &mut TestAppContext) {
    let (ha, a) = squeezed(cx, "other-window", 900.0);
    let (hb, b) = open(cx);
    cx.simulate_window_resize(hb, size(px(1600.0), px(900.0)));
    draw(cx, hb);
    resize_panel(cx, hb, &b, false, 0, 300.0);
    draw(cx, ha);
    let before = sizes(cx, &a);
    let target = before[2] - 20.0;
    resize_panel(cx, ha, &a, false, 2, target);
    let s = saved(cx);
    assert!(
        close(s.track_list_width, 300.0) && close(s.inspector_width, target),
        "전 {before:?}, 설정 {s:?}"
    );
}
