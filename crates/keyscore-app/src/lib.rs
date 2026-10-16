//! KeyScore macOS 앱. GPUI(gpui-kit) UI, 키맵, 모드 FSM, 온보딩, 설정.
//!
//! 바이너리(`main.rs`)는 [`run`]만 부른다. 테스트는 이 라이브러리의 [`init`]과
//! [`open_project_window`]로 창을 연다.

pub mod actions;
pub mod app_state;
pub mod document;
pub mod palette;
pub mod settings;
pub mod theme;
pub mod window;

use std::path::PathBuf;

use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Focusable as _, KeyBinding, Menu, MenuItem,
    TitlebarOptions, WindowOptions,
};
use keyscore_core::Project;

use crate::actions::{
    CloseWindow, NewWindow, Quit, ThemeDark, ThemeLight, ThemeSystem, ToggleBottomPanel,
    ToggleInspector, ToggleTrackList,
};
use crate::app_state::AppSettings;
use crate::document::ProjectDocument;
use crate::settings::ThemeChoice;
use crate::window::ProjectWindow;

/// 앱을 초기화한다. 창을 열기 전에 한 번 부른다. `settings_path`는 설정 파일 위치다(테스트가 주입).
pub fn init(cx: &mut App, settings_path: PathBuf) {
    gpui_kit::init(cx);
    AppSettings::install(settings_path, cx);
    theme::apply(AppSettings::get(cx).theme, cx);
    cx.on_action(|_: &NewWindow, cx| {
        let _ = open_project_window(Project::new(), cx);
    });
    cx.on_action(|_: &Quit, cx| {
        AppSettings::flush(cx);
        cx.quit();
    });
    cx.on_action(|_: &ThemeSystem, cx| theme::choose(ThemeChoice::System, cx));
    cx.on_action(|_: &ThemeLight, cx| theme::choose(ThemeChoice::Light, cx));
    cx.on_action(|_: &ThemeDark, cx| theme::choose(ThemeChoice::Dark, cx));
    // 키 바인딩을 먼저 걸어야 메뉴에 단축키가 표시된다(gpui-kit 코딩 가이드). 키맵 엔진은 S2-T2다.
    cx.bind_keys([
        KeyBinding::new("cmd-n", NewWindow, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-i", ToggleInspector, None),
    ]);
    cx.set_menus([
        Menu {
            name: "KeyScore".into(),
            items: vec![MenuItem::action("KeyScore 끝내기", Quit)],
            disabled: false,
        },
        Menu {
            name: "파일".into(),
            items: vec![
                MenuItem::action("새 창", NewWindow),
                MenuItem::action("창 닫기", CloseWindow),
            ],
            disabled: false,
        },
        Menu {
            name: "보기".into(),
            items: vec![
                MenuItem::action("트랙 목록", ToggleTrackList),
                MenuItem::action("트랙 인스펙터", ToggleInspector),
                MenuItem::action("하단 영역", ToggleBottomPanel),
                MenuItem::separator(),
                MenuItem::action("시스템 테마", ThemeSystem),
                MenuItem::action("라이트 테마", ThemeLight),
                MenuItem::action("다크 테마", ThemeDark),
            ],
            disabled: false,
        },
    ]);
}

/// 창을 열지 못한 이유.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    /// 프로젝트를 문서로 만들 수 없다(히스토리 상한 등).
    #[error("{0}")]
    Model(#[from] keyscore_core::ModelError),
    /// 플랫폼이 창을 만들지 못했다.
    #[error("창을 열 수 없다: {0}")]
    Window(String),
}

/// 프로젝트 하나로 새 창을 연다.
///
/// # Errors
///
/// [`OpenError`].
pub fn open_project_window(
    project: Project,
    cx: &mut App,
) -> Result<(AnyWindowHandle, gpui_kit::Entity<ProjectWindow>), OpenError> {
    let document = ProjectDocument::new(project)?;
    let options = WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some("KeyScore".into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    gpui_kit::open_window(options, cx, |window, cx| {
        let document = cx.new(|_| document);
        let view = cx.new(|cx| {
            let view = ProjectWindow::new(document, cx);
            window.focus(&view.focus_handle(cx), cx);
            view
        });
        window.on_window_should_close(cx, |window, _| {
            let _ = window;
            true
        });
        view
    })
    .map_err(|e| OpenError::Window(e.to_string()))
}

/// 앱을 실행한다(바이너리 진입점).
pub fn run() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            init(cx, settings::Settings::default_path());
            cx.on_action(|_: &CloseWindow, cx| {
                if let Some(window) = cx.active_window() {
                    let _ = window.update(cx, |_, window, _| window.remove_window());
                }
            });
            let _ = open_project_window(Project::new(), cx);
            cx.activate(true);
        });
}
