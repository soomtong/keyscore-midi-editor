//! S2-T1 적대적 리뷰 1회차. 설정 저장·복원, 접힌 패널, 포커스 밖 Action, 디바운스, 팔레트를 깨뜨려 본다.

use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext, px};
use keyscore_app::actions::{ThemeDark, ThemeLight, ToggleBottomPanel, ToggleInspector};
use keyscore_app::app_state::AppSettings;
use keyscore_app::palette::track_color;
use keyscore_app::settings::{LayoutSettings, Settings};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, TrackColor};

fn temp_settings(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-s2t1-{}-{name}", std::process::id()));
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

fn region_width(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> f32 {
    cx.update_window(handle, |_, window, _| {
        f32::from(window.find(id).bounds().size.width)
    })
    .unwrap()
}

// ---------- 설정 파일 ----------

/// 모듈 문서는 「모르는 키는 보존한다」고 한다. 뒤 버전이 `layout` 안에 키를 더하면 이 버전이
/// 저장할 때 지워진다(최상위 키만 보존한다).
#[test]
fn unknown_layout_keys_survive_save() {
    let path = temp_settings("nested-unknown");
    std::fs::write(
        &path,
        r#"{"layout": {"inspector_open": false, "piano_roll_height": 321}}"#,
    )
    .unwrap();
    let (s, warnings) = Settings::load(&path);
    assert!(warnings.is_empty(), "{warnings:?}");
    s.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("piano_roll_height"), "{text}");
}

/// 쓸 수 없는 곳에 저장하면 오류이고, 임시 파일이 남지 않는다.
#[test]
fn save_to_unwritable_place_fails_without_leftovers() {
    let path = temp_settings("unwritable");
    let dir = path.parent().unwrap().to_path_buf();
    // 부모가 파일이다.
    let blocker = dir.join("file");
    std::fs::write(&blocker, "x").unwrap();
    assert!(
        Settings::default()
            .save(&blocker.join("settings.json"))
            .is_err()
    );
    // 대상이 디렉터리다(rename 실패).
    let as_dir = dir.join("settings-dir.json");
    std::fs::create_dir_all(&as_dir).unwrap();
    assert!(Settings::default().save(&as_dir).is_err());
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

/// 같은 값으로 저장 → 읽기를 반복해도 파일 바이트가 같다(키 순서가 흔들리지 않는다).
#[test]
fn save_is_byte_stable_across_round_trips() {
    let path = temp_settings("stable");
    std::fs::write(
        &path,
        r#"{"zeta": 1, "alpha": {"b": 2, "a": 1}, "theme": "light"}"#,
    )
    .unwrap();
    let (s, _) = Settings::load(&path);
    s.save(&path).unwrap();
    let first = std::fs::read_to_string(&path).unwrap();
    let (s, _) = Settings::load(&path);
    s.save(&path).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
}

// ---------- 디바운스 ----------

/// 변경 직후에는 쓰지 않고, 연달아 바꾸면 앞 예약이 취소되어 마지막 상태가 한 번 쓰인다.
#[gpui_kit::test]
fn debounce_coalesces_and_writes_final_state(cx: &mut TestAppContext) {
    let path = temp_settings("debounce");
    cx.update(|cx| init(cx, path.clone()));
    let (handle, _) = open(cx);
    let toggle = |cx: &mut TestAppContext| {
        cx.update_window(handle, |_, window, cx| {
            window.dispatch_action(Box::new(ToggleBottomPanel), cx);
        })
        .unwrap();
    };
    toggle(cx);
    cx.executor().advance_clock(Duration::from_millis(600));
    cx.run_until_parked();
    assert!(!path.exists(), "디바운스 전에 썼다");
    toggle(cx);
    cx.executor().advance_clock(Duration::from_millis(600));
    cx.run_until_parked();
    assert!(!path.exists(), "앞 예약이 취소되지 않았다");
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    let (saved, _) = Settings::load(&path);
    assert!(saved.layout.bottom_open, "{saved:?}");
}

// ---------- 접힌 패널 ----------

/// 트랙 목록이 접힌 채로 창을 열고 다른 패널을 조절한 뒤 트랙 목록을 펴면, 저장된 너비(300)로
/// 돌아와야 한다. 크기 조절이 숨은 패널의 크기 상태를 0(최소 120)으로 고정한다.
#[gpui_kit::test]
fn collapsed_panel_reopens_at_saved_width_after_sibling_resize(cx: &mut TestAppContext) {
    let path = temp_settings("collapsed-reopen");
    let mut s = Settings::default();
    s.layout.track_list_open = false;
    s.layout.track_list_width = 300.0;
    s.save(&path).unwrap();
    cx.update(|cx| init(cx, path));
    let (handle, view) = open(cx);
    let h = cx.read(|cx| view.read(cx).horizontal_state().clone());
    cx.update_window(handle, |_, window, cx| {
        h.update(cx, |s, cx| s.resize_panel(2, px(320.), window, cx));
        window.render_frame(cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(keyscore_app::actions::ToggleTrackList), cx);
        window.render_frame(cx);
    })
    .unwrap();
    draw(cx, handle);
    let width = region_width(cx, handle, "track-list");
    assert!((width - 300.0).abs() < 2.0, "트랙 목록 너비 {width}");
}

/// 접은 패널을 다시 펴면 접기 전 너비로 돌아온다(창을 연 뒤에 접은 경우).
#[gpui_kit::test]
fn panel_collapsed_after_open_reopens_at_previous_width(cx: &mut TestAppContext) {
    let path = temp_settings("collapse-after-open");
    let mut s = Settings::default();
    s.layout.inspector_width = 310.0;
    s.save(&path).unwrap();
    cx.update(|cx| init(cx, path));
    let (handle, _) = open(cx);
    for _ in 0..2 {
        cx.update_window(handle, |_, window, cx| {
            window.dispatch_action(Box::new(ToggleInspector), cx);
            window.render_frame(cx);
        })
        .unwrap();
    }
    draw(cx, handle);
    let width = region_width(cx, handle, "inspector");
    assert!((width - 310.0).abs() < 2.0, "인스펙터 너비 {width}");
    let saved = cx.read(|cx| AppSettings::get(cx).layout);
    assert!((saved.inspector_width - 310.0).abs() < 1.0, "{saved:?}");
}

// ---------- 포커스 ----------

/// 창에 포커스가 없을 때(gpui-component `Input`은 Esc에서 `window.blur()`를 부른다) `Cmd+I`·보기
/// 메뉴의 패널 토글이 아무 일도 하지 않는다. Action 처리기가 `ProjectWindow`의 자식 div에만 있어
/// 포커스가 없을 때의 디스패치 경로(루트 노드)에 걸리지 않는다.
#[gpui_kit::test]
fn panel_toggle_works_when_window_has_no_focus(cx: &mut TestAppContext) {
    cx.update(|cx| init(cx, temp_settings("no-focus")));
    let (handle, view) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        window.blur(cx);
        window.render_frame(cx);
        window.dispatch_action(Box::new(ToggleInspector), cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert!(
        !cx.read(|cx| view.read(cx).layout().inspector_open),
        "포커스가 없을 때 ToggleInspector가 무시됐다"
    );
}

// ---------- 팔레트 ----------

fn rgb_distance(a: gpui_kit::Hsla, b: gpui_kit::Hsla) -> f32 {
    let (a, b) = (a.to_rgb(), b.to_rgb());
    ((a.r - b.r).powi(2) + (a.g - b.g).powi(2) + (a.b - b.b).powi(2)).sqrt()
}

/// 12색이 서로 「다른 값」일 뿐 아니라 눈으로 구별되고, 배경과도 구별된다(두 테마 모두).
#[gpui_kit::test]
fn palette_colors_are_visibly_distinct_in_both_themes(cx: &mut TestAppContext) {
    cx.update(|cx| init(cx, temp_settings("palette")));
    let _ = open(cx);
    for dark in [false, true] {
        if dark {
            cx.update(|cx| cx.dispatch_action(&ThemeDark));
        } else {
            cx.update(|cx| cx.dispatch_action(&ThemeLight));
        }
        let (colors, background) = cx.read(|cx| {
            use gpui_kit::component::ActiveTheme as _;
            (
                (0..12)
                    .map(|i| track_color(TrackColor::new(i).unwrap(), cx))
                    .collect::<Vec<_>>(),
                cx.theme().background,
            )
        });
        for i in 0..12 {
            for j in i + 1..12 {
                let d = rgb_distance(colors[i], colors[j]);
                assert!(d > 0.08, "dark={dark} {i}·{j} 거리 {d}");
            }
            let bg = rgb_distance(colors[i], background);
            assert!(bg > 0.15, "dark={dark} {i}와 배경 거리 {bg}");
        }
    }
}

/// 범위 밖 크기를 저장한 설정으로 열어도 창의 레이아웃은 기본값이고 설정 범위 안이다.
#[gpui_kit::test]
fn out_of_range_sizes_open_with_defaults(cx: &mut TestAppContext) {
    let path = temp_settings("out-of-range");
    std::fs::write(
        &path,
        r#"{"layout": {"track_list_width": -5, "inspector_width": 1e30, "bottom_height": 0}}"#,
    )
    .unwrap();
    cx.update(|cx| init(cx, path));
    let (_, view) = open(cx);
    assert_eq!(
        cx.read(|cx| view.read(cx).layout()),
        LayoutSettings::default()
    );
}
