//! 앱 전역 상태: 설정과 그 저장.
//!
//! 설정은 GPUI 전역(`Global`)에 둔다. 바꿀 때마다 1초 디바운스로 저장하고, 종료 때 바로 저장한다.

use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::{App, BorrowAppContext as _, Global, Task};

use crate::settings::Settings;

/// 저장 디바운스. 패널 크기를 끄는 동안 매 프레임 쓰지 않게 한다.
const SAVE_DEBOUNCE: Duration = Duration::from_secs(1);

/// 앱 설정과 파일 위치.
pub struct AppSettings {
    settings: Settings,
    path: PathBuf,
    pending: Option<Task<()>>,
}

impl Global for AppSettings {}

impl AppSettings {
    /// 설정을 읽어 전역에 둔다. 경고는 표준 오류에 쓴다(상태줄 알림은 S2-T5).
    pub fn install(path: PathBuf, cx: &mut App) {
        let (settings, warnings) = Settings::load(&path);
        for w in warnings {
            eprintln!("{w}");
        }
        cx.set_global(AppSettings {
            settings,
            path,
            pending: None,
        });
    }

    #[must_use]
    pub fn get(cx: &App) -> &Settings {
        &cx.global::<AppSettings>().settings
    }

    /// 설정을 바꾸고 저장을 예약한다. 같은 값이면 아무것도 하지 않는다.
    pub fn update(cx: &mut App, edit: impl FnOnce(&mut Settings)) {
        let changed = cx.update_global::<AppSettings, _>(|this, _| {
            let before = this.settings.clone();
            edit(&mut this.settings);
            this.settings != before
        });
        if !changed {
            return;
        }
        let task = cx.spawn(async move |cx| {
            cx.background_executor().timer(SAVE_DEBOUNCE).await;
            cx.update(AppSettings::flush);
        });
        cx.global_mut::<AppSettings>().pending = Some(task);
    }

    /// 지금 저장한다(종료 때, 테스트).
    pub fn flush(cx: &mut App) {
        let this = cx.global_mut::<AppSettings>();
        this.pending = None;
        if let Err(e) = this.settings.save(&this.path) {
            eprintln!("설정을 저장하지 못했다: {e}");
        }
    }
}
