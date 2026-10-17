//! 키맵 엔진(REQ 7.3, KM-1·2·5·6·7·11, 7.5).
//!
//! 레이어 셋을 차례로 GPUI에 건다: 표준(`Cmd` 단축키, 프리셋 불변) → 프리셋(`default`·`vim`) →
//! 사용자(`keymap.json`). GPUI는 나중에 건 바인딩을 앞 바인딩보다 먼저 보므로 뒤 레이어가 이긴다.
//! 컨텍스트 우선순위는 GPUI 규칙을 그대로 쓴다: 키 컨텍스트 트리에서 더 깊은 노드에 맞는 바인딩이
//! 이기므로 `Normal > Arrangement`가 `Normal`보다 우선한다. 깊이가 같으면 뒤 레이어·뒤 항목이 이긴다.
//! 예외 둘(GPUI 규칙에서 나온다, 리뷰 1 제안 10):
//! - 컨텍스트 없는(전역) 바인딩은 어느 깊이와도 맞으므로, 뒤에 건 전역 바인딩은 가장 깊은 컨텍스트의
//!   앞 바인딩과 동률이 되어 이긴다.
//! - 사용자 레이어의 얕은 컨텍스트(`Normal`) 바인딩은 프리셋의 깊은 컨텍스트(`Normal > PianoRoll`)
//!   바인딩을 덮지 못한다. 덮으려면 같은 컨텍스트로 적는다.
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

use gpui_kit::{App, Global, KeyBinding, KeyBindingContextPredicate, Keystroke, NoAction, Task};
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
    let standard_keys: std::collections::HashSet<String> =
        standard.iter().map(|b| canonical_keys(&b.keys)).collect();
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
            if standard_keys.contains(&canonical_keys(&b.keys)) {
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
            // 사용자는 표준 단축키도 덮을 수 있지만, 메뉴 단축키가 바뀌므로 알린다(KM-12).
            for b in &bindings {
                if standard_keys.contains(&canonical_keys(&b.keys)) {
                    errors.push(KeymapError {
                        layer: Layer::User,
                        pos: b.pos,
                        message: format!("경고: 표준 단축키 '{}'를 덮는다", b.keys),
                    });
                }
            }
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

/// 키 시퀀스를 GPUI 키 입력 단위로 정규화한다(수식키 별칭 `super`·`win`, 대소문자, 순서를 맞춘다).
/// 읽을 수 없는 토큰은 그대로 둔다(그 오류는 `build`가 위치와 함께 낸다).
#[must_use]
pub fn canonical_keys(keys: &str) -> String {
    keys.split_whitespace()
        .map(|stroke| Keystroke::parse(stroke).map_or_else(|_| stroke.to_string(), |k| k.unparse()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// 수식키 `alt`와 `cmd`를 바꾼다(KM-11). GPUI가 받는 별칭(`super`, `win`, 대소문자)도 같다.
#[must_use]
pub fn swap_alt_cmd(keys: &str) -> String {
    keys.split_whitespace()
        .map(|stroke| match Keystroke::parse(stroke) {
            Ok(mut k) => {
                std::mem::swap(&mut k.modifiers.alt, &mut k.modifiers.platform);
                k.unparse()
            }
            Err(_) => stroke.to_string(),
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
    let standard_keys: std::collections::HashSet<String> = layers
        .standard
        .iter()
        .map(|b| canonical_keys(&b.keys))
        .collect();
    // 같은 레이어 안에서는 짧은 시퀀스를 먼저 건다. GPUI는 나중 바인딩을 우선하므로 긴 시퀀스(`y y`)가
    // 대기를 이기고, 짧은 것(`y`)은 대기 시간 뒤에 실행된다. 파일 안의 순서에 기대지 않는다(리뷰 2
    // 지적 1). 레이어 순서와 같은 길이 안의 순서는 그대로다(안정 정렬).
    let mut ordered: Vec<&Binding> = layers.iter().collect();
    ordered.sort_by_key(|b| (b.layer, b.keys.split_whitespace().count()));
    for b in ordered {
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
        // 수식키 교환으로 사용자 키가 표준 키가 되면(`alt-s` → `cmd-s`) 그것도 알린다(KM-12, 리뷰 2
        // 지적 5). 교환 없이 겹치는 경우는 `load_layers`가 이미 알렸다.
        if options.swap_alt_cmd
            && b.layer == Layer::User
            && !standard_keys.contains(&canonical_keys(&b.keys))
            && standard_keys.contains(&canonical_keys(&keys))
        {
            errors.push(error(format!(
                "경고: 수식키 교환으로 '{}'가 표준 단축키 '{keys}'를 덮는다",
                b.keys
            )));
        }
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
            // 지우기: GPUI의 NoAction은 같은 키의 우선순위가 낮은 바인딩(같거나 얕은 컨텍스트의 것,
            // 앞에 건 것)을 모두 가린다. 그래서 프리셋 파일은 깊은 컨텍스트에서 얕은 컨텍스트의 자기
            // 바인딩을 가리지 않게 `null` 대신 Action을 직접 건다(리뷰 1 지적 1).
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
    /// 카운트 접두(KM-5). 다음 키 Action 처리기가 `take_count`로 꺼낸다.
    count: Option<u32>,
    /// 지금 키 입력으로 시작된 디스패치 중인가. 이때만 `take_count`가 카운트를 준다.
    in_key_dispatch: bool,
    /// 마지막으로 제대로 읽힌 사용자 레이어.
    last_user: Vec<Binding>,
    /// 설치 전에 이미 걸려 있던 바인딩(gpui-component가 `init`에서 건 `PopupMenu`·`Input` 등의 키).
    /// `apply`는 `clear_key_bindings`로 모두 지우므로 매번 가장 먼저(가장 낮은 우선순위로) 다시 건다.
    base: Vec<KeyBinding>,
}

impl Global for KeymapState {}

/// 엔진을 설치하고 키맵을 건다. `after_apply`는 걸 때마다 부른다(메뉴 다시 걸기).
pub fn install(
    cx: &mut App,
    user_path: PathBuf,
    options: KeymapOptions,
    after_apply: fn(&mut App),
) {
    let base = cx.key_bindings().borrow().bindings().cloned().collect();
    cx.set_global(KeymapState {
        options,
        user_path,
        seen: None,
        errors: Vec::new(),
        after_apply: Some(after_apply),
        poll: None,
        count: None,
        in_key_dispatch: false,
        last_user: Vec::new(),
        base,
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
    // 카운트는 키 입력의 흐름에만 붙는다. GPUI에는 모든 Action 디스패치를 보는 훅이 없어(메뉴·팔레트
    // 경로를 잡을 수 없다) 다음처럼 정한다(리뷰 2 지적 2·3).
    // - 키 입력으로 시작된 디스패치 안에서만 `take_count`가 카운트를 준다. 메뉴·팔레트로 실행된
    //   처리기는 카운트를 받지 않는다.
    // - 키 입력이 끝났는데 그것이 카운트 숫자도 아니고 대기 중인 시퀀스의 앞부분도 아니면 남은
    //   카운트를 비운다. 바인딩이 없는 키(`escape`)도 비운다.
    cx.intercept_keystrokes(|_, _, cx| {
        cx.global_mut::<KeymapState>().in_key_dispatch = true;
    })
    .detach();
    cx.observe_keystrokes(|event, window, cx| {
        let is_count = event
            .action
            .as_ref()
            .is_some_and(|a| a.as_any().is::<crate::actions::operator::Count>());
        let state = cx.global_mut::<KeymapState>();
        state.in_key_dispatch = false;
        if !is_count && !window.has_pending_keystrokes() {
            state.count = None;
        }
    })
    .detach();
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
    let mut read_errors = Vec::new();
    let user_text = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        // 있는데 못 읽으면(권한, UTF-8 아님) 알리고 마지막으로 읽힌 사용자 레이어를 쓴다(리뷰 1 제안 11).
        Err(e) => {
            read_errors.push(KeymapError {
                layer: Layer::User,
                pos: Pos { line: 1, column: 1 },
                message: format!("keymap.json을 읽을 수 없다: {e}"),
            });
            None
        }
    };
    let (mut layers, mut errors) = load_layers(options.preset, user_text.as_deref());
    // 사용자 파일이 문법 오류로 통째로 읽히지 않으면 마지막으로 읽힌 사용자 레이어를 유지한다. 편집 중
    // 저장할 때마다 사용자 키가 모두 사라졌다 돌아오지 않게 한다(리뷰 1 제안 13).
    let user_syntax_error = user_text.is_some()
        && layers.user.is_empty()
        && errors.iter().any(|e| e.layer == Layer::User);
    if user_syntax_error || !read_errors.is_empty() {
        layers
            .user
            .clone_from(&cx.global::<KeymapState>().last_user);
    } else {
        cx.global_mut::<KeymapState>()
            .last_user
            .clone_from(&layers.user);
    }
    errors.extend(read_errors);
    let (bindings, build_errors) = build(&layers, options, cx);
    errors.extend(build_errors);
    let base = cx.global::<KeymapState>().base.clone();
    cx.clear_key_bindings();
    cx.bind_keys(base);
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
    crate::app_state::AppSettings::update(cx, |s| s.keymap.preset = preset);
    apply(cx);
}

/// 수식키 교환을 켜거나 끈다(KM-11).
pub fn set_swap_alt_cmd(cx: &mut App, on: bool) {
    cx.global_mut::<KeymapState>().options.swap_alt_cmd = on;
    crate::app_state::AppSettings::update(cx, |s| s.keymap.swap_alt_cmd = on);
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

/// 쌓인 카운트를 꺼낸다(KM-5). 키 입력으로 시작된 디스패치 안에서만 주고, 그 밖(메뉴, 팔레트,
/// 처리기 밖)에서는 `None`이다.
pub fn take_count(cx: &mut App) -> Option<u32> {
    let state = cx.global_mut::<KeymapState>();
    if state.in_key_dispatch {
        state.count.take()
    } else {
        None
    }
}

/// 쌓인 카운트를 버린다(모드 나가기, `Esc` 처리기).
pub fn clear_count(cx: &mut App) {
    cx.global_mut::<KeymapState>().count = None;
}

/// 바탕 레이어: 엔진을 설치하기 전에 걸려 있던 바인딩(gpui-component의 `PopupMenu`·`Input`·`Dialog`·
/// `Root` 키). 표준·프리셋·사용자 레이어보다 우선순위가 낮다. 키맵 편집기(S2-T6)가 보여 줄 때 쓴다.
#[must_use]
pub fn base_bindings(cx: &App) -> &[KeyBinding] {
    &cx.global::<KeymapState>().base
}

/// 쌓여 있는 카운트를 꺼내지 않고 본다(상태줄 표시, 진단).
#[must_use]
pub fn pending_count(cx: &App) -> Option<u32> {
    cx.global::<KeymapState>().count
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
