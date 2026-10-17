//! S2-T1 적대적 리뷰 3회차. 리뷰 2 지적 1의 근본 수정(접힌 패널을 그룹의 자식에서 뺀다, 접기·펴기
//! 때 크기 상태를 새로 만든다, 크기를 펴진 패널 순서로 매핑한다)을 무작위 접기·조절·펴기 순서와 두
//! 창으로 깨뜨려 본다. 제안 2(UTF-8이 아닌 설정 파일 백업)도 본다.

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext, px};
use keyscore_app::actions::view::{ToggleBottomPanel, ToggleInspector, ToggleTrackList};
use keyscore_app::app_state::AppSettings;
use keyscore_app::settings::{LayoutSettings, Settings};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::Project;

fn temp_settings(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-s2t1r3-{}-{name}", std::process::id()));
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

/// gpui-base는 컨테이너 크기가 바뀐 프레임 뒤에 한 번 더 맞춘다. 여러 번 그려 안정된 값을 본다.
fn draw(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    for _ in 0..3 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
    }
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
    (a - b).abs() <= 2.0
}

/// 결정적인 의사 난수(xorshift). 실패한 seed를 그대로 다시 돌릴 수 있게.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u16) -> u16 {
        u16::try_from(self.next() % u64::from(n)).unwrap()
    }
}

/// 창 하나의 레이아웃 모델. 그 창에서의 명시적 조절과 토글만 값을 바꾼다.
fn check_window(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    model: LayoutSettings,
    ctx: &str,
) {
    let actual = cx.read(|cx| view.read(cx).layout());
    assert_eq!(actual, model, "{ctx}: 창 레이아웃이 모델과 다르다");
    if model.track_list_open {
        let (w, _) = region_size(cx, handle, "track-list");
        assert!(
            close(w, model.track_list_width),
            "{ctx}: 트랙 목록 그린 너비 {w}, 저장 {}",
            model.track_list_width
        );
    }
    if model.inspector_open {
        let (w, _) = region_size(cx, handle, "inspector");
        assert!(
            close(w, model.inspector_width),
            "{ctx}: 인스펙터 그린 너비 {w}, 저장 {}",
            model.inspector_width
        );
    }
    if model.bottom_open {
        let (_, h) = region_size(cx, handle, "bottom-panel");
        assert!(
            close(h, model.bottom_height),
            "{ctx}: 하단 그린 높이 {h}, 저장 {}",
            model.bottom_height
        );
    }
}

fn run_sequence(cx: &mut TestAppContext, seed: u64, steps: usize) {
    let mut rng = Rng(seed);
    // 시작 레이아웃도 무작위로 접힌 패널을 섞는다.
    let mut initial = Settings::default().layout;
    initial.track_list_open = rng.below(2) == 0;
    initial.inspector_open = rng.below(2) == 0;
    initial.bottom_open = rng.below(2) == 0;
    initial.track_list_width = 130.0 + f32::from(rng.below(250));
    initial.inspector_width = 130.0 + f32::from(rng.below(250));
    initial.bottom_height = 130.0 + f32::from(rng.below(150));
    start_with(cx, &format!("seq-{seed}"), |l| *l = initial);

    let windows = [open(cx), open(cx)];
    let mut models = [initial, initial];
    // 설정의 각 필드는 어느 창에서든 마지막으로 바꾼 값이다.
    let mut saved = initial;
    let mut log = Vec::new();

    for step in 0..steps {
        let w = usize::from(rng.below(2));
        let (handle, ref view) = windows[w];
        let m = &mut models[w];
        match rng.below(6) {
            0 => {
                toggle(cx, handle, Box::new(ToggleTrackList));
                m.track_list_open = !m.track_list_open;
                saved.track_list_open = m.track_list_open;
                log.push(format!("w{w}:toggle-track"));
            }
            1 => {
                toggle(cx, handle, Box::new(ToggleInspector));
                m.inspector_open = !m.inspector_open;
                saved.inspector_open = m.inspector_open;
                log.push(format!("w{w}:toggle-inspector"));
            }
            2 => {
                toggle(cx, handle, Box::new(ToggleBottomPanel));
                m.bottom_open = !m.bottom_open;
                saved.bottom_open = m.bottom_open;
                log.push(format!("w{w}:toggle-bottom"));
            }
            3 if m.track_list_open => {
                let v = 130.0 + f32::from(rng.below(250));
                resize(cx, handle, view, false, 0, v);
                m.track_list_width = v;
                saved.track_list_width = v;
                log.push(format!("w{w}:track={v}"));
            }
            4 if m.inspector_open => {
                let v = 130.0 + f32::from(rng.below(250));
                let last = usize::from(m.track_list_open) + 1;
                resize(cx, handle, view, false, last, v);
                m.inspector_width = v;
                saved.inspector_width = v;
                log.push(format!("w{w}:inspector={v}"));
            }
            5 if m.bottom_open => {
                let v = 130.0 + f32::from(rng.below(150));
                resize(cx, handle, view, true, 1, v);
                m.bottom_height = v;
                saved.bottom_height = v;
                log.push(format!("w{w}:bottom={v}"));
            }
            _ => continue,
        }
        let ctx = format!("seed {seed} step {step} [{}]", log.join(", "));
        for (i, (h, v)) in windows.iter().enumerate() {
            check_window(cx, *h, v, models[i], &format!("{ctx} w{i}"));
        }
        let s = cx.read(|cx| AppSettings::get(cx).layout);
        assert_eq!(s, saved, "{ctx}: 설정이 마지막 명시적 변경과 다르다");
    }
}

/// 세 패널의 접기·조절·펴기를 두 창에 무작위로 섞는다. 매 단계 뒤 펴진 패널은 그 창의 저장 크기로
/// 그려지고, 저장 크기는 그 패널의 명시적 조절로만 바뀐다.
#[gpui_kit::test]
fn random_collapse_resize_reopen_sequences_keep_drawn_and_stored_sizes(cx: &mut TestAppContext) {
    for seed in 1..=12_u64 {
        run_sequence(cx, seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1, 40);
    }
}

/// 세 패널을 모두 접은 채 열고, 조절 없이 하나씩 펴고 다시 거꾸로 접었다 편다. 모두 저장 크기다.
#[gpui_kit::test]
fn all_collapsed_at_open_reopen_in_any_order_at_saved_sizes(cx: &mut TestAppContext) {
    start_with(cx, "all-collapsed", |l| {
        l.track_list_open = false;
        l.inspector_open = false;
        l.bottom_open = false;
        l.track_list_width = 333.0;
        l.inspector_width = 177.0;
        l.bottom_height = 244.0;
    });
    let (handle, view) = open(cx);
    for action in [
        Box::new(ToggleInspector) as Box<dyn gpui_kit::Action>,
        Box::new(ToggleBottomPanel),
        Box::new(ToggleTrackList),
        Box::new(ToggleInspector),
        Box::new(ToggleTrackList),
        Box::new(ToggleInspector),
        Box::new(ToggleTrackList),
    ] {
        toggle(cx, handle, action);
        let layout = cx.read(|cx| view.read(cx).layout());
        check_window(cx, handle, &view, layout, "토글 뒤");
        assert_eq!(
            (
                layout.track_list_width,
                layout.inspector_width,
                layout.bottom_height
            ),
            (333.0, 177.0, 244.0)
        );
    }
}

/// 접기 전에 쥐고 있던 옛 크기 상태로 조절 이벤트를 내도 레이아웃·설정에 쓰이지 않는다(구독이 새
/// 상태로 옮겨졌다). 옛 상태의 인덱스는 접힌 패널을 포함하므로 잘못 매핑되면 값이 틀어진다.
#[gpui_kit::test]
fn resize_on_stale_state_after_collapse_is_ignored(cx: &mut TestAppContext) {
    start_with(cx, "stale-state", |_| {});
    let (handle, view) = open(cx);
    let stale = cx.read(|cx| view.read(cx).horizontal_state().clone());
    toggle(cx, handle, Box::new(ToggleTrackList));
    let before = cx.read(|cx| view.read(cx).layout());
    cx.update_window(handle, |_, window, cx| {
        stale.update(cx, |s, cx| s.resize_panel(0, px(150.0), window, cx));
        stale.update(cx, |s, cx| s.resize_panel(2, px(150.0), window, cx));
    })
    .unwrap();
    draw(cx, handle);
    let after = cx.read(|cx| view.read(cx).layout());
    assert_eq!(before, after);
    let saved = cx.read(|cx| AppSettings::get(cx).layout);
    assert!(close(saved.inspector_width, before.inspector_width));
    assert!(close(saved.track_list_width, before.track_list_width));
}

/// 제안 2: UTF-8이 아닌 바이트도 `.bak`으로 원본 그대로 남고, 기본값과 경고가 나온다.
#[test]
fn non_utf8_settings_file_is_backed_up_byte_for_byte() {
    let path = temp_settings("non-utf8");
    let bytes: &[u8] = &[0x7b, 0x22, 0xff, 0xfe, 0x22, 0x3a, 0x31, 0x7d, 0x80];
    std::fs::write(&path, bytes).unwrap();
    let (settings, warnings) = Settings::load(&path);
    assert_eq!(settings, Settings::default());
    assert!(!warnings.is_empty(), "경고가 없다");
    let bak = path.with_file_name("settings.json.bak");
    assert_eq!(std::fs::read(&bak).unwrap(), bytes);
    // 원본도 읽기에서 지워지지 않는다.
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
