//! 키맵 엔진(REQ 7.3, KM-1·2·5·6·7·11, 7.5).
//!
//! 레이어 셋을 차례로 GPUI에 건다: 표준(`Cmd` 단축키, 프리셋 불변) → 프리셋(`default`·`vim`) →
//! 사용자(`keymap.json`). GPUI는 나중에 건 바인딩을 앞 바인딩보다 먼저 보므로 뒤 레이어가 이긴다.
//! 컨텍스트 우선순위는 GPUI 규칙을 그대로 쓴다: 키 컨텍스트 트리에서 더 깊은 노드에 맞는 바인딩이
//! 이기므로 `Normal > Arrangement`가 `Normal`보다 우선한다. 깊이가 같으면 뒤 레이어·뒤 항목이 이긴다.
//!
//! 물리 키 매칭(KM-7): GPUI macOS는 바인딩을 `Keystroke.key`로 맞추고, 이 값은 현재 자판
//! 레이아웃(한글 입력 중에도 ABC/U.S.)과 keyCode로 만든다. 입력된 글자는 `key_char`에 따로 담긴다.
//! 그래서 한/영 전환과 무관하게 같은 자리의 키가 같은 Action을 낸다. 문자 기준이 필요한 곳(텍스트
//! 입력, 가사 모드)은 그 컨텍스트에 바인딩을 두지 않고 입력 요소가 IME로 받는다.

pub mod conflict;
pub mod source;

use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use gpui_kit::{App, Global, KeyBinding, KeyBindingContextPredicate, NoAction, Task};
use serde::{Deserialize, Serialize};

use self::source::{ActionRef, Binding, KeymapError, Layer, Pos, parse};

/// 표준 `Cmd` 단축키(7.5). 프리셋과 무관하다.
pub const STANDARD_KEYMAP: &str = include_str!("../../../../assets/keymaps/standard.json");
/// 기본 프리셋(5.8).
pub const DEFAULT_KEYMAP: &str = include_str!("../../../../assets/keymaps/default.json");
/// vim 프리셋(5.8.1). 기본 프리셋 위에 얹는다.
pub const VIM_KEYMAP: &str = include_str!("../../../../assets/keymaps/vim.json");

/// 사용자 키맵 파일을 살피는 주기(KM-6 「저장 즉시」에 가깝게, 디스크를 과하게 두드리지 않게).
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// 프리셋(KM-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    #[default]
    Default,
    Vim,
}

/// 엔진 옵션.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeymapOptions {
    pub preset: Preset,
    /// 수식키 교환(KM-11): 프리셋·사용자 바인딩의 `alt`와 `cmd`를 바꾼다. 표준 레이어는 그대로다.
    pub swap_alt_cmd: bool,
}

/// 세 레이어의 바인딩(아직 GPUI에 걸지 않은 원문).
#[derive(Debug, Clone, Default)]
pub struct LayerSet {
    pub standard: Vec<Binding>,
    pub preset: Vec<Binding>,
    pub user: Vec<Binding>,
}

impl LayerSet {
    /// 레이어 순서대로 모든 바인딩.
    pub fn iter(&self) -> impl Iterator<Item = &Binding> {
        self.standard.iter().chain(&self.preset).chain(&self.user)
    }
}

/// 레이어를 읽는다. 프리셋이 표준 레이어의 키(7.5의 macOS 표준 단축키)를 다시 정의하려 하면 그
/// 항목을 거부한다(표준 단축키는 프리셋 불변). 표준 레이어에 없는 `Cmd` 조합(`cmd-j` 합치기 등)은
/// 프리셋이 쓴다. 사용자 레이어는 무엇이든 덮을 수 있다.
#[must_use]
pub fn load_layers(preset: Preset, user_text: Option<&str>) -> (LayerSet, Vec<KeymapError>) {
    let mut errors = Vec::new();
    let (standard, e) = parse(STANDARD_KEYMAP, Layer::Standard);
    errors.extend(e);
    let standard_keys: std::collections::HashSet<&str> =
        standard.iter().map(|b| b.keys.as_str()).collect();
    let mut preset_bindings = Vec::new();
    let texts: &[(&str, Layer)] = match preset {
        Preset::Default => &[(DEFAULT_KEYMAP, Layer::Preset)],
        Preset::Vim => &[
            (DEFAULT_KEYMAP, Layer::Preset),
            (VIM_KEYMAP, Layer::PresetOverlay),
        ],
    };
    for &(text, layer) in texts {
        let (bindings, e) = parse(text, layer);
        errors.extend(e);
        for b in bindings {
            if standard_keys.contains(b.keys.as_str()) {
                errors.push(KeymapError {
                    layer,
                    pos: b.pos,
                    message: format!("프리셋은 표준 단축키 '{}'를 바꿀 수 없다", b.keys),
                });
            } else {
                preset_bindings.push(b);
            }
        }
    }
    let user = match user_text {
        Some(text) => {
            let (bindings, e) = parse(text, Layer::User);
            errors.extend(e);
            bindings
        }
        None => Vec::new(),
    };
    (
        LayerSet {
            standard,
            preset: preset_bindings,
            user,
        },
        errors,
    )
}

/// 수식키 `alt`와 `cmd`를 바꾼다(KM-11). 키 이름(마지막 토큰)은 그대로 둔다.
#[must_use]
pub fn swap_alt_cmd(keys: &str) -> String {
    keys.split(' ')
        .map(|stroke| {
            let mut parts: Vec<&str> = stroke.split('-').collect();
            let last = parts.len().saturating_sub(1);
            for part in &mut parts[..last] {
                *part = match *part {
                    "alt" => "cmd",
                    "cmd" => "alt",
                    other => other,
                };
            }
            parts.join("-")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// 레이어를 GPUI 바인딩으로 만든다. 모르는 Action·잘못된 키·잘못된 컨텍스트·잘못된 인자는 그 항목만
/// 빼고 오류로 돌려준다.
#[must_use]
pub fn build(
    layers: &LayerSet,
    options: KeymapOptions,
    cx: &App,
) -> (Vec<KeyBinding>, Vec<KeymapError>) {
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for b in layers.iter() {
        let error = |message: String| KeymapError {
            layer: b.layer,
            pos: b.pos,
            message,
        };
        let keys = if options.swap_alt_cmd && b.layer != Layer::Standard {
            swap_alt_cmd(&b.keys)
        } else {
            b.keys.clone()
        };
        let predicate = match b.context.as_deref() {
            None => None,
            Some(context) => match KeyBindingContextPredicate::parse(context) {
                Ok(p) => Some(Rc::new(p)),
                Err(e) => {
                    errors.push(error(format!("컨텍스트 '{context}'가 잘못됐다: {e}")));
                    continue;
                }
            },
        };
        let action: Box<dyn gpui_kit::Action> = match &b.action {
            // 지우기: GPUI의 NoAction은 같은 (컨텍스트, 키)의 앞 바인딩을 가린다.
            ActionRef::Unbind => Box::new(NoAction),
            ActionRef::Named { name, args } => match cx.build_action(name, args.clone()) {
                Ok(action) => action,
                Err(e) => {
                    errors.push(error(format!("Action '{name}'을 만들 수 없다: {e}")));
                    continue;
                }
            },
        };
        match KeyBinding::load(
            &keys,
            action,
            predicate,
            false,
            None,
            cx.keyboard_mapper().as_ref(),
        ) {
            Ok(binding) => out.push(binding),
            Err(e) => errors.push(error(format!("키 '{keys}'가 잘못됐다: {e}"))),
        }
    }
    (out, errors)
}

/// 앱 전역 키맵 상태.
pub struct KeymapState {
    options: KeymapOptions,
    user_path: PathBuf,
    /// 마지막으로 읽은 사용자 파일의 (수정 시각, 크기). 바뀌면 다시 읽는다.
    seen: Option<(SystemTime, u64)>,
    errors: Vec<KeymapError>,
    /// 다시 건 뒤에 부를 일(메뉴 다시 걸기. 메뉴는 단축키를 구워 둔다).
    after_apply: Option<fn(&mut App)>,
    poll: Option<Task<()>>,
    /// 카운트 접두(KM-5). 다음 Action 처리기가 `take_count`로 꺼낸다.
    count: Option<u32>,
}

impl Global for KeymapState {}

/// 엔진을 설치하고 키맵을 건다. `after_apply`는 걸 때마다 부른다(메뉴 다시 걸기).
pub fn install(
    cx: &mut App,
    user_path: PathBuf,
    options: KeymapOptions,
    after_apply: fn(&mut App),
) {
    cx.set_global(KeymapState {
        options,
        user_path,
        seen: None,
        errors: Vec::new(),
        after_apply: Some(after_apply),
        poll: None,
        count: None,
    });
    apply(cx);
    let poll = cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(POLL_INTERVAL).await;
            cx.update(|cx| {
                if user_file_changed(cx) {
                    apply(cx);
                }
            });
        }
    });
    cx.global_mut::<KeymapState>().poll = Some(poll);
    register_count_actions(cx);
}

fn file_stamp(path: &PathBuf) -> Option<(SystemTime, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

fn user_file_changed(cx: &App) -> bool {
    let state = cx.global::<KeymapState>();
    file_stamp(&state.user_path) != state.seen
}

/// 레이어를 다시 읽어 건다(부트, 핫리로드, 프리셋 전환).
pub fn apply(cx: &mut App) {
    let (options, path) = {
        let s = cx.global::<KeymapState>();
        (s.options, s.user_path.clone())
    };
    let stamp = file_stamp(&path);
    let user_text = std::fs::read_to_string(&path).ok();
    let (layers, mut errors) = load_layers(options.preset, user_text.as_deref());
    let (bindings, build_errors) = build(&layers, options, cx);
    errors.extend(build_errors);
    cx.clear_key_bindings();
    cx.bind_keys(bindings);
    let after = {
        let s = cx.global_mut::<KeymapState>();
        s.seen = stamp;
        s.errors = errors;
        s.after_apply
    };
    if let Some(after) = after {
        after(cx);
    }
}

/// 프리셋을 바꾼다. 사용자 오버라이드는 그대로다(KM-2).
pub fn set_preset(cx: &mut App, preset: Preset) {
    cx.global_mut::<KeymapState>().options.preset = preset;
    apply(cx);
}

/// 수식키 교환을 켜거나 끈다(KM-11).
pub fn set_swap_alt_cmd(cx: &mut App, on: bool) {
    cx.global_mut::<KeymapState>().options.swap_alt_cmd = on;
    apply(cx);
}

/// 마지막으로 걸 때의 오류(상태줄 표시는 S2-T5).
#[must_use]
pub fn errors(cx: &App) -> &[KeymapError] {
    &cx.global::<KeymapState>().errors
}

#[must_use]
pub fn options(cx: &App) -> KeymapOptions {
    cx.global::<KeymapState>().options
}

/// 쌓인 카운트를 꺼낸다(KM-5). 없으면 `None`.
pub fn take_count(cx: &mut App) -> Option<u32> {
    cx.global_mut::<KeymapState>().count.take()
}

/// 카운트 숫자 Action(`operator::Count(n)`, vim 프리셋의 `1`–`9`). 상한을 두어 넘치지 않게 한다.
fn register_count_actions(cx: &mut App) {
    const MAX_COUNT: u32 = 9999;
    cx.on_action(|digit: &crate::actions::operator::Count, cx| {
        let d = u32::try_from(digit.0.0.clamp(0, 9)).unwrap_or(0);
        let state = cx.global_mut::<KeymapState>();
        state.count = Some(
            state
                .count
                .unwrap_or(0)
                .saturating_mul(10)
                .saturating_add(d)
                .min(MAX_COUNT),
        );
    });
}

/// 위치를 가진 오류를 사람이 읽는 한 줄로.
#[must_use]
pub fn describe(error: &KeymapError) -> String {
    let layer = match error.layer {
        Layer::Standard => "표준 키맵",
        Layer::Preset => "기본 프리셋",
        Layer::PresetOverlay => "vim 프리셋",
        Layer::User => "keymap.json",
    };
    let Pos { line, column } = error.pos;
    format!("{layer} {line}:{column}: {}", error.message)
}
