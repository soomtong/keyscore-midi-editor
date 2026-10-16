//! S2-T1 적대적 리뷰 2회차. 리뷰 1 지적 2·3·4의 수정과 받아들인 제안(여러 창 병합, `.bak` 백업,
//! 필드 단위 기본값, 종료 때 저장)을 주변 변형으로 깨뜨려 본다.

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext, px};
use keyscore_app::actions::{ToggleBottomPanel, ToggleInspector, ToggleTrackList};
use keyscore_app::app_state::AppSettings;
use keyscore_app::settings::{LayoutSettings, Settings};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::Project;

fn temp_settings(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-s2t1r2-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("settings.json")
}

fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx
        .update(|cx| open_project_window(Project::new(), cx))
        .unwrap();
    draw(cx, handle);
    (handle, view)
}

fn draw(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn region_size(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> (f32, f32) {
    cx.update_window(handle, |_, window, _| {
        let size = window.find(id).bounds().size;
        (f32::from(size.width), f32::from(size.height))
    })
    .unwrap()
}

fn toggle(cx: &mut TestAppContext, handle: AnyWindowHandle, action: Box<dyn gpui_kit::Action>) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(action, cx);
        window.render_frame(cx);
    })
    .unwrap();
    draw(cx, handle);
}

fn resize(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    vertical: bool,
    index: usize,
    size: f32,
) {
    let state = cx.read(|cx| {
        if vertical {
            view.read(cx).vertical_state().clone()
        } else {
            view.read(cx).horizontal_state().clone()
        }
    });
    cx.update_window(handle, |_, window, cx| {
        state.update(cx, |s, cx| s.resize_panel(index, px(size), window, cx));
        window.render_frame(cx);
    })
    .unwrap();
    draw(cx, handle);
}

fn start_with(cx: &mut TestAppContext, name: &str, edit: impl FnOnce(&mut LayoutSettings)) {
    let path = temp_settings(name);
    let mut s = Settings::default();
    edit(&mut s.layout);
    s.save(&path).unwrap();
    cx.update(|cx| init(cx, path));
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 2.0
}

// ---------- 지적 2 변형: 세 패널 각각 ----------

/// 인스펙터가 접힌 채 열고 트랙 목록을 조절한 뒤 인스펙터를 펴면 저장된 너비다. 트랙 목록 조절도
/// 그대로 남는다(새 크기 상태가 형제의 조절을 되돌리지 않는다).
#[gpui_kit::test]
fn inspector_collapsed_at_open_reopens_at_saved_width_after_track_list_resize(
    cx: &mut TestAppContext,
) {
    start_with(cx, "inspector-at-open", |l| {
        l.inspector_open = false;
        l.inspector_width = 300.0;
    });
    let (handle, view) = open(cx);
    resize(cx, handle, &view, false, 0, 260.0);
    toggle(cx, handle, Box::new(ToggleInspector));
    let (inspector, _) = region_size(cx, handle, "inspector");
    let (track, _) = region_size(cx, handle, "track-list");
    assert!(close(inspector, 300.0), "인스펙터 너비 {inspector}");
    assert!(close(track, 260.0), "트랙 목록 너비 {track}");
}

/// 근본 원인 확인: 인스펙터가 접혀 있는 동안 트랙 목록을 260으로 조절하면 그 값이 그려지고 저장돼야
/// 한다. gpui-base의 크기 목록에 숨은 패널의 옛 크기가 남아(`resize_panel_at_handle`은 동기화 전
/// `old_sizes`로 계산한다) 합이 컨테이너를 넘고, 넘친 만큼 조절한 패널에서 깎인다. 끌기도 같은 함수를
/// 거친다. 연 뒤에 접은 경우와 접힌 채 연 경우 둘 다 본다.
/// 인스펙터를 접기만 해도 트랙 목록이 넓어진다(220 → 350). 숨은 패널이 빈 div가 되어 그 몫이 형제
/// 패널의 flex 성장으로 나뉜다. 레이아웃 값(220)과 그린 너비가 어긋나고, 이어 끌면 그 어긋난 값에서
/// 시작한다.
#[gpui_kit::test]
fn collapsing_inspector_keeps_track_list_width(cx: &mut TestAppContext) {
    start_with(cx, "collapse-keeps-sibling", |_| {});
    let (handle, _) = open(cx);
    toggle(cx, handle, Box::new(ToggleInspector));
    let (track, _) = region_size(cx, handle, "track-list");
    assert!(
        close(track, 220.0),
        "인스펙터를 접자 트랙 목록 너비 {track}"
    );
}

fn sibling_resize_while_inspector_collapsed(cx: &mut TestAppContext, collapsed_at_open: bool) {
    start_with(cx, &format!("sibling-{collapsed_at_open}"), |l| {
        l.inspector_open = !collapsed_at_open;
    });
    let (handle, view) = open(cx);
    if !collapsed_at_open {
        toggle(cx, handle, Box::new(ToggleInspector));
    }
    resize(cx, handle, &view, false, 0, 260.0);
    let (track, _) = region_size(cx, handle, "track-list");
    let saved = cx.read(|cx| AppSettings::get(cx).layout);
    assert!(
        close(track, 260.0) && close(saved.track_list_width, 260.0),
        "접힌 채 열기={collapsed_at_open}: 그린 너비 {track}, 저장 {saved:?}"
    );
}

#[gpui_kit::test]
fn sibling_resize_while_panel_collapsed_at_open_is_drawn_and_saved_as_requested(
    cx: &mut TestAppContext,
) {
    sibling_resize_while_inspector_collapsed(cx, true);
}

#[gpui_kit::test]
fn sibling_resize_after_collapsing_panel_is_drawn_and_saved_as_requested(cx: &mut TestAppContext) {
    sibling_resize_while_inspector_collapsed(cx, false);
}

/// 연 뒤에 접은 트랙 목록: 인스펙터를 조절한 뒤 펴면 접기 전 너비이고 인스펙터 조절은 남는다.
#[gpui_kit::test]
fn track_list_collapsed_after_open_reopens_after_inspector_resize(cx: &mut TestAppContext) {
    start_with(cx, "track-after-open", |l| l.track_list_width = 260.0);
    let (handle, view) = open(cx);
    toggle(cx, handle, Box::new(ToggleTrackList));
    // 구현자 수정: 리뷰 2가 낸 근본 수정(펴진 패널만 그룹의 자식)으로 접힌 트랙 목록은 크기 상태에
    // 없다. 자식은 [메인, 인스펙터]라 인스펙터는 인덱스 1이다. 검사 의도는 같다.
    resize(cx, handle, &view, false, 1, 330.0);
    toggle(cx, handle, Box::new(ToggleTrackList));
    let (track, _) = region_size(cx, handle, "track-list");
    let (inspector, _) = region_size(cx, handle, "inspector");
    assert!(close(track, 260.0), "트랙 목록 너비 {track}");
    assert!(close(inspector, 330.0), "인스펙터 너비 {inspector}");
}

/// 하단 영역: 높이를 조절하고 접은 뒤 가로 그룹을 조절하고 펴면 조절한 높이다.
#[gpui_kit::test]
fn bottom_panel_reopens_at_resized_height_after_other_group_resize(cx: &mut TestAppContext) {
    start_with(cx, "bottom", |_| {});
    let (handle, view) = open(cx);
    resize(cx, handle, &view, true, 1, 280.0);
    toggle(cx, handle, Box::new(ToggleBottomPanel));
    resize(cx, handle, &view, false, 0, 250.0);
    toggle(cx, handle, Box::new(ToggleBottomPanel));
    let (_, bottom) = region_size(cx, handle, "bottom-panel");
    let (track, _) = region_size(cx, handle, "track-list");
    assert!(close(bottom, 280.0), "하단 높이 {bottom}");
    assert!(close(track, 250.0), "트랙 목록 너비 {track}");
}

/// 두 패널을 접고 하나씩 펴는 사이에 조절해도 각자 저장된 너비로 돌아온다.
#[gpui_kit::test]
fn two_collapsed_panels_reopen_one_by_one_at_their_widths(cx: &mut TestAppContext) {
    start_with(cx, "two-collapsed", |l| {
        l.track_list_open = false;
        l.track_list_width = 240.0;
        l.inspector_open = false;
        l.inspector_width = 310.0;
    });
    let (handle, view) = open(cx);
    toggle(cx, handle, Box::new(ToggleTrackList));
    resize(cx, handle, &view, false, 0, 270.0);
    toggle(cx, handle, Box::new(ToggleInspector));
    let (track, _) = region_size(cx, handle, "track-list");
    let (inspector, _) = region_size(cx, handle, "inspector");
    assert!(close(track, 270.0), "트랙 목록 너비 {track}");
    assert!(close(inspector, 310.0), "인스펙터 너비 {inspector}");
    let saved = cx.read(|cx| AppSettings::get(cx).layout);
    assert!(close(saved.track_list_width, 270.0), "{saved:?}");
    assert!(close(saved.inspector_width, 310.0), "{saved:?}");
}

// ---------- 지적 3 변형: 포커스 ----------

/// 포커스가 트리에 없는 핸들로 옮겨진 뒤 그 핸들이 사라진다(포커스를 가진 요소가 제거된 경우와
/// 같은 디스패치 경로: 포커스 경로가 빈다). 그래도 패널 토글이 닿는다.
#[gpui_kit::test]
fn panel_toggle_works_after_focused_element_is_removed(cx: &mut TestAppContext) {
    cx.update(|cx| init(cx, temp_settings("focus-removed")));
    let (handle, view) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        let child = cx.focus_handle();
        window.focus(&child, cx);
        window.render_frame(cx);
        drop(child);
        window.render_frame(cx);
    })
    .unwrap();
    toggle(cx, handle, Box::new(ToggleInspector));
    assert!(
        !cx.read(|cx| view.read(cx).layout().inspector_open),
        "포커스를 가진 요소가 사라진 뒤 ToggleInspector가 무시됐다"
    );
    // 연달아 blur해도(포커스 되돌림이 한 번만 동작하지 않는가) 계속 닿는다.
    for expected in [true, false] {
        cx.update_window(handle, |_, window, cx| {
            window.blur(cx);
            window.render_frame(cx);
        })
        .unwrap();
        toggle(cx, handle, Box::new(ToggleInspector));
        assert_eq!(
            cx.read(|cx| view.read(cx).layout().inspector_open),
            expected
        );
    }
}

// ---------- 지적 4 변형: layout 안의 모르는 키 ----------

/// 중첩 객체·배열·null을 가진 모르는 키가 저장 뒤에도 같은 값으로 남고, 두 번째부터 바이트가 같다.
#[test]
fn nested_unknown_layout_values_survive_and_are_byte_stable() {
    let path = temp_settings("nested-layout");
    std::fs::write(
        &path,
        r#"{"layout": {"track_list_open": false, "zz": {"b": [1, {"c": null}], "a": 2}, "inspector_width": 300, "aa": null}, "theme": "dark"}"#,
    )
    .unwrap();
    let (s, warnings) = Settings::load(&path);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(!s.layout.track_list_open);
    s.save(&path).unwrap();
    let first = std::fs::read_to_string(&path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(
        v["layout"]["zz"],
        serde_json::json!({"b": [1, {"c": null}], "a": 2})
    );
    assert!(
        v["layout"]
            .get("aa")
            .is_some_and(serde_json::Value::is_null),
        "{first}"
    );
    assert_eq!(v["layout"]["inspector_width"], serde_json::json!(300.0));
    for _ in 0..3 {
        let (s, _) = Settings::load(&path);
        s.save(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
    }
}

/// `layout`의 알려진 필드가 null이면 그 필드만 기본값이고 나머지는 남는다. `layout` 자체가 null이어도
/// 앱이 뜬다.
#[test]
fn null_layout_fields_fall_back_alone() {
    let path = temp_settings("null-field");
    std::fs::write(
        &path,
        r#"{"layout": {"inspector_width": null, "bottom_open": null, "track_list_width": 250}}"#,
    )
    .unwrap();
    let (s, warnings) = Settings::load(&path);
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    let d = LayoutSettings::default();
    assert!(close(s.layout.inspector_width, d.inspector_width));
    assert_eq!(s.layout.bottom_open, d.bottom_open);
    assert!(close(s.layout.track_list_width, 250.0));
    std::fs::write(&path, r#"{"layout": null, "theme": "light"}"#).unwrap();
    let (s, warnings) = Settings::load(&path);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(s.layout, d);
    s.save(&path).unwrap();
    let (again, w) = Settings::load(&path);
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(again, s);
}

// ---------- 제안 5: 여러 창 병합 ----------

/// 창 B에서 인스펙터를 끌면 B의 트랙 목록 너비(조절하지 않음)가 창 A에서 바꾼 값을 되돌리지 않는다.
#[gpui_kit::test]
fn dragging_in_one_window_keeps_other_windows_untouched_widths(cx: &mut TestAppContext) {
    cx.update(|cx| init(cx, temp_settings("merge-drag")));
    let (a, a_view) = open(cx);
    let (b, b_view) = open(cx);
    resize(cx, a, &a_view, false, 0, 310.0);
    resize(cx, b, &b_view, false, 2, 330.0);
    let layout = cx.read(|cx| AppSettings::get(cx).layout);
    assert!(close(layout.track_list_width, 310.0), "{layout:?}");
    assert!(close(layout.inspector_width, 330.0), "{layout:?}");
    // B가 패널을 다시 펴(새 크기 상태) 다시 그려도 A의 값을 되돌리지 않는다.
    toggle(cx, b, Box::new(ToggleBottomPanel));
    toggle(cx, b, Box::new(ToggleBottomPanel));
    let layout = cx.read(|cx| AppSettings::get(cx).layout);
    assert!(close(layout.track_list_width, 310.0), "{layout:?}");
}

// ---------- 제안 8: 백업 ----------

/// JSON 객체가 아닌 여러 모양(배열, 빈 파일, UTF-8이 아닌 바이트)의 설정 파일은 첫 변경 때 덮어쓰기
/// 전에 `.bak`으로 남는다(보고서: 「JSON 객체가 아니면 settings.json.bak으로 복사」).
#[test]
fn non_object_settings_files_are_backed_up() {
    let cases: [(&str, &[u8]); 3] = [
        ("array", b"[1, 2]"),
        ("empty", b""),
        ("not-utf8", b"{\"theme\": \"\xff\xfe\"}"),
    ];
    for (name, bytes) in cases {
        let path = temp_settings(&format!("bak-{name}"));
        std::fs::write(&path, bytes).unwrap();
        let (_, warnings) = Settings::load(&path);
        assert!(!warnings.is_empty(), "{name}");
        let backup = path.with_extension("json.bak");
        assert_eq!(
            std::fs::read(&backup).ok().as_deref(),
            Some(bytes),
            "{name}: 백업이 없다. 경고 {warnings:?}"
        );
    }
}

// ---------- 제안 9: 종료 때 저장 ----------

/// Quit Action을 거치지 않는 종료(앱 shutdown)에서도 디바운스 중인 변경이 파일에 쓰인다.
#[gpui_kit::test]
fn app_shutdown_flushes_pending_settings(cx: &mut TestAppContext) {
    let path = temp_settings("quit-flush");
    cx.update(|cx| init(cx, path.clone()));
    let (handle, _) = open(cx);
    toggle(cx, handle, Box::new(ToggleBottomPanel));
    assert!(!path.exists(), "디바운스 전에 썼다");
    cx.update(gpui_kit::App::shutdown);
    let (saved, _) = Settings::load(&path);
    assert!(!saved.layout.bottom_open, "{saved:?}");
}
