//! S2-T1 앱 창 테스트. GPUI 테스트 컨텍스트에서 실제 창 진입점(`open_project_window`)을 쓴다.

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{
    ThemeDark, ThemeLight, ToggleBottomPanel, ToggleInspector, ToggleTrackList,
};
use keyscore_app::app_state::AppSettings;
use keyscore_app::palette::track_color;
use keyscore_app::settings::{LayoutSettings, Settings, ThemeChoice};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Edit, Project, Track, TrackColor};

fn temp_settings(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-app-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("settings.json")
}

fn open(cx: &mut TestAppContext, project: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(project, cx)).unwrap();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    (handle, view)
}

fn start(cx: &mut TestAppContext, settings: PathBuf) {
    cx.update(|cx| init(cx, settings));
}

// ---------- 1–2. 설정 파일 ----------

#[test]
fn settings_round_trip_keeps_unknown_keys() {
    let path = temp_settings("round-trip");
    std::fs::write(
        &path,
        r#"{"future": {"a": [1, 2]}, "theme": "dark", "layout": {"inspector_open": false, "later": 3}}"#,
    )
    .unwrap();
    let (mut s, warnings) = Settings::load(&path);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(s.theme, ThemeChoice::Dark);
    assert!(!s.layout.inspector_open);
    assert!(
        (s.layout.track_list_width - LayoutSettings::default().track_list_width).abs()
            < f32::EPSILON
    );
    s.layout.bottom_height = 300.0;
    s.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"future\""), "{text}");
    let (again, _) = Settings::load(&path);
    assert_eq!(again, s);
}

#[test]
fn settings_missing_or_broken_file_falls_back_to_defaults() {
    let path = temp_settings("broken");
    let (s, warnings) = Settings::load(&path);
    assert_eq!(s, Settings::default());
    assert!(warnings.is_empty());
    for text in [
        "not json",
        "[1, 2]",
        r#"{"theme": "purple"}"#,
        r#"{"layout": {"track_list_width": "wide"}}"#,
    ] {
        std::fs::write(&path, text).unwrap();
        let (s, warnings) = Settings::load(&path);
        assert_eq!(s.layout, LayoutSettings::default(), "{text}");
        assert_eq!(s.theme, ThemeChoice::System, "{text}");
        assert_eq!(warnings.len(), 1, "{text}: {warnings:?}");
    }
    std::fs::write(
        &path,
        r#"{"layout": {"track_list_width": 5, "inspector_width": 1e9}}"#,
    )
    .unwrap();
    let (s, warnings) = Settings::load(&path);
    assert_eq!(s.layout, LayoutSettings::default());
    assert_eq!(warnings.len(), 2, "{warnings:?}");
}

// ---------- 3–7. 창 ----------

#[gpui_kit::test]
fn opening_window_shows_all_layout_regions(cx: &mut TestAppContext) {
    start(cx, temp_settings("regions"));
    let (handle, _) = open(cx, Project::new());
    cx.update_window(handle, |_, window, _| {
        for id in [
            "transport",
            "track-list",
            "main-view",
            "inspector",
            "bottom-panel",
            "status-bar",
        ] {
            assert!(window.find(id).visible(), "{id}");
        }
    })
    .unwrap();
}

#[gpui_kit::test]
fn toggle_actions_collapse_and_restore_panels(cx: &mut TestAppContext) {
    let path = temp_settings("toggle");
    start(cx, path.clone());
    let (handle, view) = open(cx, Project::new());
    let dispatch = |cx: &mut TestAppContext, action: Box<dyn gpui_kit::Action>| {
        cx.update_window(handle, |_, window, cx| {
            window.dispatch_action(action, cx);
            window.render_frame(cx);
        })
        .unwrap();
    };
    dispatch(cx, Box::new(ToggleTrackList));
    dispatch(cx, Box::new(ToggleInspector));
    dispatch(cx, Box::new(ToggleBottomPanel));
    let layout = cx.read(|cx| view.read(cx).layout());
    assert!(!layout.track_list_open && !layout.inspector_open && !layout.bottom_open);
    cx.update_window(handle, |_, window, _| {
        for id in ["track-list", "inspector", "bottom-panel"] {
            assert!(window.try_find(id).is_none_or(|s| !s.visible()), "{id}");
        }
        assert!(window.find("main-view").visible());
    })
    .unwrap();
    // 설정에 반영되고 저장된다.
    cx.update(AppSettings::flush);
    let (saved, _) = Settings::load(&path);
    assert!(!saved.layout.inspector_open);
    dispatch(cx, Box::new(ToggleInspector));
    assert!(cx.read(|cx| view.read(cx).layout().inspector_open));
    assert!(cx.read(|cx| AppSettings::get(cx).layout.inspector_open));
}

#[gpui_kit::test]
fn panel_layout_is_restored_from_settings(cx: &mut TestAppContext) {
    let path = temp_settings("restore");
    let mut s = Settings::default();
    s.layout.track_list_open = false;
    s.layout.inspector_width = 333.0;
    s.save(&path).unwrap();
    start(cx, path);
    let (handle, view) = open(cx, Project::new());
    let layout = cx.read(|cx| view.read(cx).layout());
    assert!(!layout.track_list_open);
    assert!((layout.inspector_width - 333.0).abs() < f32::EPSILON);
    cx.update_window(handle, |_, window, _| {
        assert!(window.try_find("track-list").is_none_or(|s| !s.visible()));
        let width = f32::from(window.find("inspector").bounds().size.width);
        assert!((width - 333.0).abs() < 2.0, "인스펙터 너비 {width}");
    })
    .unwrap();
}

#[gpui_kit::test]
fn theme_choice_switches_mode_and_palette(cx: &mut TestAppContext) {
    start(cx, temp_settings("theme"));
    let _ = open(cx, Project::new());
    let palette = |cx: &mut TestAppContext| {
        cx.read(|cx| {
            (0..12)
                .map(|i| track_color(TrackColor::new(i).unwrap(), cx))
                .collect::<Vec<_>>()
        })
    };
    cx.update(|cx| cx.dispatch_action(&ThemeLight));
    let light = palette(cx);
    cx.update(|cx| cx.dispatch_action(&ThemeDark));
    let dark = palette(cx);
    assert_ne!(light, dark);
    for colors in [&light, &dark] {
        for i in 0..12 {
            for j in i + 1..12 {
                assert_ne!(colors[i], colors[j], "{i} {j}");
            }
        }
    }
    assert_eq!(cx.read(|cx| AppSettings::get(cx).theme), ThemeChoice::Dark);
}

#[gpui_kit::test]
fn each_window_owns_its_own_document(cx: &mut TestAppContext) {
    start(cx, temp_settings("documents"));
    let (_, a) = open(cx, Project::new());
    let (_, b) = open(cx, Project::new());
    let doc_a = cx.read(|cx| a.read(cx).document().clone());
    let doc_b = cx.read(|cx| b.read(cx).document().clone());
    let track = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    cx.update(|cx| {
        doc_a.update(cx, |d, _| {
            d.apply(vec![Edit::InsertTrack {
                index: 0,
                track: Box::new(track),
            }])
            .unwrap();
        });
    });
    cx.read(|cx| {
        assert_eq!(doc_a.read(cx).project().tracks().len(), 1);
        assert_eq!(doc_b.read(cx).project().tracks().len(), 0);
        assert!(doc_a.read(cx).is_dirty());
        assert!(!doc_b.read(cx).is_dirty());
    });
    cx.update(|cx| {
        doc_a.update(cx, |d, _| assert!(d.undo().unwrap()));
    });
    cx.read(|cx| assert_eq!(doc_a.read(cx).project().tracks().len(), 0));
}

// ---------- 8. 문서 ----------

#[test]
fn document_changes_only_through_edits() {
    use keyscore_app::document::ProjectDocument;
    let mut d = ProjectDocument::new(Project::new()).unwrap();
    let track = Track::new("A", Channel::new(2).unwrap(), TrackColor::new(3).unwrap());
    let id = track.id();
    d.apply(vec![Edit::InsertTrack {
        index: 0,
        track: Box::new(track),
    }])
    .unwrap();
    assert_eq!(d.undo_len(), 1);
    // 불변식을 어기는 편집은 거절되고 문서는 그대로다.
    let dup = d.project().tracks()[0].clone();
    assert!(
        d.apply(vec![Edit::InsertTrack {
            index: 1,
            track: Box::new(dup)
        }])
        .is_err()
    );
    assert_eq!(d.project().tracks().len(), 1);
    assert_eq!(d.undo_len(), 1);
    assert!(d.undo().unwrap());
    assert!(d.project().track(id).is_none());
    assert!(d.redo().unwrap());
    assert!(d.project().track(id).is_some());
}

#[gpui_kit::test]
fn resizing_panels_is_saved_for_next_window(cx: &mut TestAppContext) {
    let path = temp_settings("resize");
    start(cx, path.clone());
    let (handle, view) = open(cx, Project::new());
    let (h, v) = cx.read(|cx| {
        let w = view.read(cx);
        (w.horizontal_state().clone(), w.vertical_state().clone())
    });
    cx.update_window(handle, |_, window, cx| {
        h.update(cx, |s, cx| {
            s.resize_panel(0, gpui_kit::px(300.), window, cx);
        });
        v.update(cx, |s, cx| {
            s.resize_panel(1, gpui_kit::px(250.), window, cx);
        });
        window.render_frame(cx);
    })
    .unwrap();
    let layout = cx.read(|cx| AppSettings::get(cx).layout);
    assert!((layout.track_list_width - 300.0).abs() < 1.0, "{layout:?}");
    assert!((layout.bottom_height - 250.0).abs() < 1.0, "{layout:?}");
    cx.update(AppSettings::flush);
    let (saved, _) = Settings::load(&path);
    assert!((saved.layout.track_list_width - 300.0).abs() < 1.0);
    // 새 창은 저장된 크기로 열린다.
    let (second, _) = open(cx, Project::new());
    cx.update_window(second, |_, window, _| {
        let width = f32::from(window.find("track-list").bounds().size.width);
        assert!((width - 300.0).abs() < 2.0, "트랙 목록 너비 {width}");
    })
    .unwrap();
}

#[test]
fn broken_settings_are_backed_up_and_bad_fields_fall_back_alone() {
    let path = temp_settings("backup");
    std::fs::write(&path, "{ not json").unwrap();
    let (_, warnings) = Settings::load(&path);
    let backup = path.with_extension("json.bak");
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), "{ not json");
    assert!(warnings[0].contains(".bak"), "{warnings:?}");
    // 필드 하나만 틀리면 그 필드만 기본값이다.
    std::fs::write(
        &path,
        r#"{"layout": {"track_list_open": "yes", "inspector_width": 300}}"#,
    )
    .unwrap();
    let (s, warnings) = Settings::load(&path);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(s.layout.track_list_open);
    assert!((s.layout.inspector_width - 300.0).abs() < f32::EPSILON);
}

#[gpui_kit::test]
fn toggling_in_one_window_keeps_other_windows_changes(cx: &mut TestAppContext) {
    start(cx, temp_settings("two-windows"));
    let (a, a_view) = open(cx, Project::new());
    let (b, _) = open(cx, Project::new());
    let h = cx.read(|cx| a_view.read(cx).horizontal_state().clone());
    cx.update_window(a, |_, window, cx| {
        h.update(cx, |s, cx| {
            s.resize_panel(0, gpui_kit::px(310.), window, cx);
        });
        window.render_frame(cx);
    })
    .unwrap();
    cx.update_window(b, |_, window, cx| {
        window.dispatch_action(Box::new(ToggleBottomPanel), cx);
        window.render_frame(cx);
    })
    .unwrap();
    let layout = cx.read(|cx| AppSettings::get(cx).layout);
    assert!((layout.track_list_width - 310.0).abs() < 1.0, "{layout:?}");
    assert!(!layout.bottom_open);
}

#[test]
fn empty_edit_does_not_mark_document_dirty() {
    let mut d = keyscore_app::document::ProjectDocument::new(Project::new()).unwrap();
    d.apply(vec![]).unwrap();
    assert!(!d.is_dirty());
    assert_eq!(d.undo_len(), 0);
}
