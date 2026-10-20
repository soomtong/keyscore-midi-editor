//! S2-T6f 적대적 리뷰 2회차. 리뷰 1 지적 1 대응(`fit_side_panels`, `requested`, `record_sizes`가 요청한
//! 너비보다 더 많이 바뀐 패널만 저장)을 깨뜨리려는 시도.

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext, px, size};
use keyscore_app::actions::view::{ToggleInspector, ToggleTrackList};
use keyscore_app::app_state::AppSettings;
use keyscore_app::settings::{LayoutSettings, Settings};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::ksq;

fn dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("ks-review-s2t6f-r2-{}-{name}", std::process::id()));
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

// ---------- 깨진 시도 ----------

/// 창이 좁아 앱이 트랙 목록을 범위 최소(120)보다 작게 줄여 요청하면(저장 200·1200, 창 900에서 약 111)
/// gpui-base가 120으로 묶는다. 그 차이(약 9)가 「바뀜」으로 잡혀, 인스펙터를 5만 끌어도 트랙 목록 저장값 200이
/// 120으로 바뀌고 인스펙터 조절은 버려진다(리뷰 1 지적 1과 같은 결과).
#[gpui_kit::test]
fn clamped_squeezed_track_list_keeps_saved_width_on_inspector_drag(cx: &mut TestAppContext) {
    start_with(cx, "clamped", |l| {
        l.track_list_width = 200.0;
        l.inspector_width = 1200.0;
    });
    let (handle, view) = open(cx);
    cx.simulate_window_resize(handle, size(px(1600.0), px(900.0)));
    draw(cx, handle);
    cx.simulate_window_resize(handle, size(px(900.0), px(600.0)));
    draw(cx, handle);
    let before = sizes(cx, &view);
    let target = before[2] - 5.0;
    resize_panel(cx, handle, &view, false, 2, target);
    let after = sizes(cx, &view);
    let s = saved(cx);
    assert!(
        close(s.track_list_width, 200.0) && close(s.inspector_width, target),
        "조절 전 {before:?}, 뒤 {after:?}, 설정 {s:?}"
    );
}

/// 창이 좁은 동안 트랙 목록을 끌어 저장하면 다음 그리기에서 `requested`는 새 저장값으로 다시 줄인 값이 되지만
/// 크기 상태는 새로 만들지 않아 그린 너비는 그대로다. 이어 인스펙터를 끌면 낡은 `requested`와 비교해 트랙
/// 목록 쪽 차이가 더 커 보이고, 인스펙터 조절은 저장되지 않는다.
#[gpui_kit::test]
fn second_drag_while_squeezed_saves_dragged_inspector(cx: &mut TestAppContext) {
    start_with(cx, "second-drag", |l| {
        l.track_list_width = 500.0;
        l.inspector_width = 500.0;
    });
    let (handle, view) = open(cx);
    cx.simulate_window_resize(handle, size(px(1600.0), px(900.0)));
    draw(cx, handle);
    cx.simulate_window_resize(handle, size(px(900.0), px(600.0)));
    draw(cx, handle);
    let first = sizes(cx, &view);
    resize_panel(cx, handle, &view, false, 0, first[0] + 30.0);
    let mid = sizes(cx, &view);
    let after_track = saved(cx);
    let target = mid[2] - 20.0;
    resize_panel(cx, handle, &view, false, 2, target);
    let end = sizes(cx, &view);
    let s = saved(cx);
    assert!(
        close(s.inspector_width, target) && close(s.track_list_width, after_track.track_list_width),
        "처음 {first:?}, 트랙 조절 뒤 {mid:?}(설정 {after_track:?}), 인스펙터 조절 뒤 {end:?}(설정 {s:?})"
    );
}

// ---------- 못 깨뜨린 시도(회귀 확인) ----------

/// 넓은 창(줄이지 않음)에서 트랙 목록·인스펙터를 차례로 조절하면 둘 다 그대로 저장된다.
#[gpui_kit::test]
fn wide_window_plain_drags_are_saved(cx: &mut TestAppContext) {
    start_with(cx, "wide", |_| {});
    let (handle, view) = open(cx);
    cx.simulate_window_resize(handle, size(px(1400.0), px(900.0)));
    draw(cx, handle);
    resize_panel(cx, handle, &view, false, 0, 300.0);
    resize_panel(cx, handle, &view, false, 2, 340.0);
    resize_panel(cx, handle, &view, false, 0, 250.0);
    let s = saved(cx);
    assert!(
        close(s.track_list_width, 250.0) && close(s.inspector_width, 340.0),
        "{s:?}, {:?}",
        sizes(cx, &view)
    );
}

/// 인스펙터만 펴진 채 좁은 창에서 인스펙터를 끌면 그 값이 저장되고 접힌 트랙 목록 값은 그대로다.
#[gpui_kit::test]
fn only_inspector_open_squeezed_drag_is_saved(cx: &mut TestAppContext) {
    start_with(cx, "only-inspector", |l| {
        l.track_list_width = 400.0;
        l.inspector_width = 1000.0;
    });
    let (handle, view) = open(cx);
    run(cx, handle, ToggleTrackList);
    cx.simulate_window_resize(handle, size(px(800.0), px(600.0)));
    draw(cx, handle);
    let (_, w) = region(cx, handle, "inspector");
    resize_panel(cx, handle, &view, false, 1, 500.0);
    let s = saved(cx);
    assert!(
        close(w, 680.0) && close(s.inspector_width, 500.0) && close(s.track_list_width, 400.0),
        "그린 {w}, {s:?}"
    );
}

/// 줄어 보이는 인스펙터를 접고 다시 펴도 저장값은 그대로이고, 트랙 목록은 접는 동안 저장 너비로 그려진다.
#[gpui_kit::test]
fn toggling_squeezed_panel_keeps_saved_widths(cx: &mut TestAppContext) {
    start_with(cx, "toggle-squeezed", |l| {
        l.track_list_width = 500.0;
        l.inspector_width = 500.0;
    });
    let (handle, _view) = open(cx);
    cx.simulate_window_resize(handle, size(px(900.0), px(600.0)));
    draw(cx, handle);
    run(cx, handle, ToggleInspector);
    let (_, track_alone) = region(cx, handle, "track-list");
    run(cx, handle, ToggleInspector);
    let s = saved(cx);
    assert!(
        close(track_alone, 500.0)
            && close(s.track_list_width, 500.0)
            && close(s.inspector_width, 500.0),
        "혼자 {track_alone}, {s:?}"
    );
}

/// 메인이 최소에 닿은 채 트랙 목록을 넓히면 인스펙터가 밀리지만 저장하지 않는다.
#[gpui_kit::test]
fn pushed_inspector_is_not_saved(cx: &mut TestAppContext) {
    start_with(cx, "pushed", |_| {});
    let (handle, view) = open(cx);
    cx.simulate_window_resize(handle, size(px(1000.0), px(700.0)));
    draw(cx, handle);
    resize_panel(cx, handle, &view, false, 0, 700.0);
    let s = saved(cx);
    let now = sizes(cx, &view);
    assert!(
        close(s.track_list_width, 700.0) && close(s.inspector_width, 260.0),
        "{s:?}, {now:?}"
    );
}

/// 좁은 창에서 인스펙터를 조절한 뒤 창을 다시 넓히면 두 패널이 저장 크기로 그려진다.
#[gpui_kit::test]
fn widening_back_restores_saved_widths(cx: &mut TestAppContext) {
    start_with(cx, "widen-back", |l| {
        l.track_list_width = 500.0;
        l.inspector_width = 500.0;
    });
    let (handle, view) = open(cx);
    cx.simulate_window_resize(handle, size(px(900.0), px(600.0)));
    draw(cx, handle);
    resize_panel(cx, handle, &view, false, 2, 300.0);
    cx.simulate_window_resize(handle, size(px(1600.0), px(900.0)));
    draw(cx, handle);
    let (_, track) = region(cx, handle, "track-list");
    let (_, inspector) = region(cx, handle, "inspector");
    let s = saved(cx);
    assert!(
        close(track, 500.0) && close(inspector, 300.0) && close(s.inspector_width, 300.0),
        "트랙 {track}, 인스펙터 {inspector}, {s:?}"
    );
}

/// 아주 좁은 창(room 0, 시험 플랫폼은 최소 크기를 강제하지 않는다)에서도 패닉 없이 그리고 저장값은 그대로다.
#[gpui_kit::test]
fn tiny_window_keeps_saved_widths(cx: &mut TestAppContext) {
    start_with(cx, "tiny", |_| {});
    let (handle, _view) = open(cx);
    cx.simulate_window_resize(handle, size(px(100.0), px(100.0)));
    draw(cx, handle);
    cx.simulate_window_resize(handle, size(px(1200.0), px(800.0)));
    draw(cx, handle);
    let (_, track) = region(cx, handle, "track-list");
    let s = saved(cx);
    assert!(
        close(track, 220.0) && close(s.track_list_width, 220.0) && close(s.inspector_width, 260.0),
        "트랙 {track}, {s:?}"
    );
}

/// 범위 밖(최소 120 아래)으로 끌어도 120에 묶여 저장되고, 다른 패널은 그대로다.
#[gpui_kit::test]
fn drag_below_range_is_clamped(cx: &mut TestAppContext) {
    start_with(cx, "below-range", |_| {});
    let (handle, view) = open(cx);
    cx.simulate_window_resize(handle, size(px(1200.0), px(800.0)));
    draw(cx, handle);
    resize_panel(cx, handle, &view, false, 0, 50.0);
    let s = saved(cx);
    assert!(
        close(s.track_list_width, 120.0) && close(s.inspector_width, 260.0),
        "{s:?}"
    );
}

/// 좁은 창에서 아래 패널을 조절하면 아래 높이만 저장되고 사이드 패널 저장값은 그대로다.
#[gpui_kit::test]
fn bottom_drag_in_squeezed_window_keeps_side_widths(cx: &mut TestAppContext) {
    start_with(cx, "bottom-squeezed", |l| {
        l.track_list_width = 500.0;
        l.inspector_width = 500.0;
    });
    let (handle, view) = open(cx);
    cx.simulate_window_resize(handle, size(px(900.0), px(700.0)));
    draw(cx, handle);
    resize_panel(cx, handle, &view, true, 0, 400.0);
    let s = saved(cx);
    assert!(
        close(s.track_list_width, 500.0)
            && close(s.inspector_width, 500.0)
            && s.bottom_height > 160.0,
        "{s:?}"
    );
}

/// 두 창: A는 좁고 B는 넓다. A에서 인스펙터를 끌어도 B가 저장한 트랙 목록 값이 되돌아가지 않는다.
#[gpui_kit::test]
fn two_windows_squeezed_and_wide(cx: &mut TestAppContext) {
    start_with(cx, "two", |l| {
        l.track_list_width = 500.0;
        l.inspector_width = 500.0;
    });
    let (ha, a) = open(cx);
    let (hb, b) = open(cx);
    cx.simulate_window_resize(hb, size(px(1600.0), px(900.0)));
    draw(cx, hb);
    resize_panel(cx, hb, &b, false, 0, 450.0);
    cx.simulate_window_resize(ha, size(px(900.0), px(600.0)));
    draw(cx, ha);
    let now = sizes(cx, &a);
    resize_panel(cx, ha, &a, false, 2, now[2] - 60.0);
    let s = saved(cx);
    assert!(
        close(s.track_list_width, 450.0) && close(s.inspector_width, now[2] - 60.0),
        "A {now:?}, {s:?}"
    );
}
