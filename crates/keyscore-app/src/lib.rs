//! KeyScore macOS 앱. GPUI(gpui-kit) UI, 키맵, 모드 FSM, 온보딩, 설정.
//!
//! 바이너리(`main.rs`)는 [`run`]만 부른다. 테스트는 이 라이브러리의 [`init`]과
//! [`open_project_window`]로 창을 연다.

pub mod actions;
pub mod app_state;
pub mod command_palette;
pub mod commands;
pub mod dispatch;
pub mod document;
pub mod engine;
pub mod keymap;
pub mod mode;
pub mod palette;
pub mod settings;
pub mod theme;
mod track_list;
pub mod tracks;
mod transport;
mod transport_bar;
pub mod window;

use std::path::PathBuf;

use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Focusable as _, TitlebarOptions, WindowOptions,
};
use keyscore_core::Project;

use crate::actions::args::ThemeKind;
use crate::actions::view::SetTheme;
use crate::actions::{app::Quit, file::New, window::Close};
use crate::app_state::AppSettings;
use crate::document::ProjectDocument;
use crate::settings::ThemeChoice;
use crate::window::ProjectWindow;

/// 앱을 초기화한다. 창을 열기 전에 한 번 부른다. `settings_path`는 설정 파일 위치다(테스트가 주입).
pub fn init(cx: &mut App, settings_path: PathBuf) {
    gpui_kit::init(cx);
    // 사용자 키맵은 설정 파일 옆에 둔다(KM-6: ~/Library/Application Support/KeyScore/keymap.json).
    let keymap_path = settings_path.with_file_name("keymap.json");
    AppSettings::install(settings_path, cx);
    theme::apply(AppSettings::get(cx).theme, cx);
    cx.on_action(|_: &New, cx| {
        let _ = open_project_window(Project::new(), cx);
    });
    cx.on_action(|_: &Close, cx| {
        if let Some(window) = cx.active_window() {
            let _ = window.update(cx, |_, window, _| window.remove_window());
        }
    });
    cx.on_action(|_: &Quit, cx| {
        AppSettings::flush(cx);
        cx.quit();
    });
    // Quit Action을 거치지 않는 종료(Dock 메뉴, 로그아웃)에서도 디바운스 중인 설정을 저장한다(리뷰 1 제안 9).
    cx.on_app_quit(|cx| {
        AppSettings::flush(cx);
        async {}
    })
    .detach();
    cx.on_action(|action: &SetTheme, cx| {
        let choice = match action.0 {
            ThemeKind::System => ThemeChoice::System,
            ThemeKind::Light => ThemeChoice::Light,
            ThemeKind::Dark => ThemeChoice::Dark,
        };
        theme::choose(choice, cx);
    });
    // 키맵을 건 뒤에 메뉴를 건다(메뉴는 걸 때의 단축키를 구워 둔다). 다시 걸 때마다 메뉴도 다시 건다.
    let km = AppSettings::get(cx).keymap;
    keymap::install(
        cx,
        keymap_path,
        keymap::KeymapOptions {
            preset: km.preset,
            swap_alt_cmd: km.swap_alt_cmd,
        },
        set_menus,
    );
}

/// 앱 메뉴. 메뉴 7개 전체는 S2-T4다.
fn set_menus(cx: &mut App) {
    // 명령 표가 메뉴의 유일한 원천이다(KM-13). 키 등가물은 GPUI가 메뉴를 걸 때 현재 키맵에서 굽는다.
    let menus = commands::menus(cx);
    cx.set_menus(menus);
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
        cx.new(|cx| {
            let view = ProjectWindow::new(document, window, cx);
            window.focus(&view.focus_handle(cx), cx);
            view
        })
    })
    .map_err(|e| OpenError::Window(e.to_string()))
}

/// 앱을 실행한다(바이너리 진입점).
pub fn run() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            init(cx, settings::Settings::default_path());
            // 창보다 먼저, 앱의 첫 CoreMIDI 호출로 엔진을 연다(S1-T2). 실패해도 앱은 뜨고, 재생 Action이
            // 창에 「MIDI 엔진이 없다」를 남긴다.
            if let Err(e) = engine::install(cx, &engine::EngineOptions::default()) {
                eprintln!("KeyScore: MIDI 엔진을 열지 못했다: {e}");
            }
            let _ = open_project_window(Project::new(), cx);
            cx.activate(true);
        });
}
