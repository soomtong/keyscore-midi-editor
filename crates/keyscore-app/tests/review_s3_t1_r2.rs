//! S3-T1 적대적 리뷰 2회차(재리뷰) 시험. 1회차 지적 1·2·3과 제안 1·6의 수정이 맞게 됐는지, 그 수정이 새 결함을
//! 만들지 않았는지를 깨뜨리려 한다. 구현 파일은 고치지 않는다.
//!
//! UI 시험은 `tests/pianoroll.rs`·`tests/review_s3_t1.rs`의 도우미 패턴을 따른다.
#![allow(
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::mode::View;
use keyscore_app::pianoroll::{Cursor, DEFAULT_PX_PER_TICK, Viewport};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

// ---------------------------------------------------------------- 도우미

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rv31r2-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn open(cx: &mut TestAppContext, project: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(project, cx)).unwrap();
    render(cx, handle);
    (handle, view)
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    render(cx, handle);
}

fn note(start: u64, len: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        len,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

fn clip(start: u64, length: u64, notes: &[Note]) -> Clip {
    let mut clip = Clip::new(Tick::new(start), length).unwrap();
    for n in notes {
        clip.insert_note(n.clone()).unwrap();
    }
    clip
}

fn project_with_clip(c: Clip) -> Project {
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    track.insert_clip(c).unwrap();
    let mut project = Project::new();
    project.push_track(track).unwrap();
    project
}

fn cursor_visible(vp: &Viewport, cursor: Cursor, step: u64) -> bool {
    let tick = cursor.tick.get() as f64;
    let row = Viewport::row_of(cursor.pitch);
    tick >= vp.first_tick
        && (tick + step as f64 <= vp.last_tick() + 0.001 || tick == vp.first_tick)
        && row >= vp.first_row
        && row + 1.0 <= vp.first_row + vp.height / vp.row_height + 0.001
}

fn cursor_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Cursor {
    cx.read(|cx| view.read(cx).piano_roll().cursor())
}

fn vp(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Viewport {
    cx.read(|cx| view.read(cx).piano_roll_viewport())
}

fn current_view(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> View {
    cx.read(|cx| view.read(cx).mode_state().view())
}

fn bounds_of(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    id: &'static str,
) -> gpui_kit::Bounds<gpui_kit::Pixels> {
    cx.update_window(handle, |_, window, _| window.find(id).bounds())
        .unwrap()
}

fn wheel(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    at: gpui_kit::Point<gpui_kit::Pixels>,
    dy: f32,
) {
    let mut vcx = gpui_kit::VisualTestContext::from_window(handle, cx);
    vcx.simulate_event(gpui_kit::ScrollWheelEvent {
        position: at,
        delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(gpui_kit::px(0.0), gpui_kit::px(dy))),
        modifiers: gpui_kit::Modifiers::none(),
        touch_phase: gpui_kit::TouchPhase::Moved,
    });
    cx.run_until_parked();
    render(cx, handle);
}

fn resize(cx: &mut TestAppContext, handle: AnyWindowHandle, w: f32, h: f32) {
    cx.simulate_window_resize(handle, gpui_kit::size(gpui_kit::px(w), gpui_kit::px(h)));
    cx.run_until_parked();
    render(cx, handle);
}

// ---------------------------------------------------------------- 지적 2 수정의 회귀: 다른 요소 위의 휠

/// `should_handle_scroll`은 히트 테스트만 보므로 포인터가 인스펙터·트랙 목록 위에 있는 휠은 피아노롤로
/// 가면 안 된다(키 입력 뒤라도). 대조로 주 뷰 가운데의 휠은 스크롤한다.
#[gpui_kit::test]
fn wheel_over_side_panels_after_keyboard_does_not_scroll_piano_roll(cx: &mut TestAppContext) {
    start(cx, "wheel-side");
    let (handle, view) = open(cx, project_with_clip(clip(0, 4 * BAR, &[note(0, 960, 60)])));
    press(cx, handle, "enter");
    let before = vp(cx, &view);
    assert!(before.width > 0.0, "{before:?}");
    for id in ["inspector", "track-list", "status-bar"] {
        let at = bounds_of(cx, handle, id).center();
        wheel(cx, handle, at, -120.0);
        assert_eq!(
            vp(cx, &view),
            before,
            "{id} 위의 휠이 피아노롤을 스크롤한다"
        );
    }
    let at = bounds_of(cx, handle, "main-view").center();
    wheel(cx, handle, at, -120.0);
    let after = vp(cx, &view);
    assert!(
        (after.first_row - (before.first_row + 10.0)).abs() < 0.01,
        "대조: 주 뷰 위의 휠은 스크롤한다 {before:?} → {after:?}"
    );
}

/// 휠 → 키 → 휠(포인터 이동 없음) 순서에서도 두 번째 휠이 처리된다(키보드 모달리티가 다시 켜진 뒤).
#[gpui_kit::test]
fn wheel_key_wheel_without_pointer_move_scrolls_twice(cx: &mut TestAppContext) {
    start(cx, "wheel-key-wheel");
    let (handle, view) = open(cx, project_with_clip(clip(0, 4 * BAR, &[note(0, 960, 60)])));
    press(cx, handle, "enter");
    let at = bounds_of(cx, handle, "main-view").center();
    let v0 = vp(cx, &view);
    wheel(cx, handle, at, -120.0);
    let v1 = vp(cx, &view);
    assert!(
        (v1.first_row - (v0.first_row + 10.0)).abs() < 0.01,
        "{v0:?} → {v1:?}"
    );
    // 커서는 아직 보이므로 `l`은 뷰포트를 옮기지 않는다(가로만 봄)
    press(cx, handle, "l");
    wheel(cx, handle, at, -120.0);
    let v2 = vp(cx, &view);
    assert!(
        (v2.first_row - (v1.first_row + 10.0)).abs() < 0.01,
        "키 뒤의 둘째 휠이 무시된다 {v1:?} → {v2:?}"
    );
}

// ---------------------------------------------------------------- 지적 1 수정의 회귀: pending_follow

/// 처음 크기를 알 때 적용된 따라가기는 한 번뿐이어야 한다. 그 뒤 휠로 멀리 가면 다음 프레임이 커서로
/// 되돌리지 않는다(플래그가 소비됐는가).
#[gpui_kit::test]
fn first_layout_follow_is_consumed_and_does_not_snap_back_later(cx: &mut TestAppContext) {
    start(cx, "pending-once");
    let far = 50 * BAR;
    let (handle, view) = open(cx, project_with_clip(clip(far, BAR, &[note(0, 960, 60)])));
    press(cx, handle, "enter");
    let c = cursor_of(cx, &view);
    let v = vp(cx, &view);
    assert!(cursor_visible(&v, c, QUARTER), "{v:?} {c:?}");
    cx.read(|cx| view.read(cx).piano_roll().scroll_by(-50_000.0, 300.0));
    let away = vp(cx, &view);
    assert!(!cursor_visible(&away, c, QUARTER), "{away:?}");
    render(cx, handle);
    render(cx, handle);
    assert_eq!(vp(cx, &view), away, "그리기만으로 커서로 되돌아간다");
}

/// 배치 뷰에 있는 동안(피아노롤이 아직 한 번도 그려지지 않았을 때) `cmd-=`로 줌한 뒤 먼 클립으로 들어가도
/// 커서가 보인다(제안 6의 「크기를 모르면 배율만」과 `pending_follow`의 맞물림).
#[gpui_kit::test]
fn zoom_before_first_layout_then_enter_far_clip_keeps_cursor_visible(cx: &mut TestAppContext) {
    start(cx, "zoom-then-enter");
    let far = 50 * BAR;
    let (handle, view) = open(cx, project_with_clip(clip(far, BAR, &[note(0, 960, 60)])));
    assert_eq!(current_view(cx, &view), View::Arrangement);
    press(cx, handle, "cmd-= cmd-= cmd-= cmd-= cmd-=");
    let zoomed = vp(cx, &view);
    assert!(zoomed.px_per_tick > DEFAULT_PX_PER_TICK, "{zoomed:?}");
    assert_eq!(zoomed.width, 0.0, "아직 그리기 전");
    press(cx, handle, "enter");
    let c = cursor_of(cx, &view);
    let v = vp(cx, &view);
    assert_eq!(c.tick.get(), far);
    assert!(cursor_visible(&v, c, QUARTER), "{v:?} {c:?}");
    press(cx, handle, "cmd-0");
    let v = vp(cx, &view);
    assert!(cursor_visible(&v, c, QUARTER), "{v:?} {c:?}");
}

/// 배치 뷰에서 `ctrl-l`(Normal 공통 바인딩)로 커서를 클립 안 먼 자리로 옮긴 뒤 들어가면 커서가 보인다.
/// `place_cursor_in_clip`이 옮기지 않는 경로에서도 따라가야 한다.
#[gpui_kit::test]
fn cursor_moved_in_arrangement_before_first_layout_is_visible_on_entry(cx: &mut TestAppContext) {
    start(cx, "arr-move-enter");
    let (handle, view) = open(
        cx,
        project_with_clip(clip(0, 20 * BAR, &[note(0, 960, 60)])),
    );
    for _ in 0..15 {
        press(cx, handle, "ctrl-l");
    }
    assert_eq!(cursor_of(cx, &view).tick.get(), 15 * BAR);
    press(cx, handle, "enter");
    assert_eq!(current_view(cx, &view), View::PianoRoll);
    let c = cursor_of(cx, &view);
    assert_eq!(c.tick.get(), 15 * BAR, "클립 안이라 옮기지 않는다");
    let v = vp(cx, &view);
    assert!(cursor_visible(&v, c, QUARTER), "{v:?} {c:?}");
}

/// 피아노롤을 그린 뒤 `v`로 배치 뷰에 갔다가 다시 `v`로 돌아올 때, 커서가 보이는 상태였으면 사용자가 스크롤한
/// 자리를 그대로 둔다(`ensure_cursor_visible`은 보이면 무작위 이동을 하지 않는다).
#[gpui_kit::test]
fn returning_from_arrangement_keeps_scroll_when_cursor_still_visible(cx: &mut TestAppContext) {
    start(cx, "return-keep");
    let (handle, view) = open(cx, project_with_clip(clip(0, 8 * BAR, &[note(0, 960, 60)])));
    press(cx, handle, "enter l l l l");
    // 커서 3840틱 = x 384 px. 100 px 오른쪽·3행 아래로 스크롤해도 커서는 보인다
    cx.read(|cx| view.read(cx).piano_roll().scroll_by(100.0, 36.0));
    render(cx, handle);
    let scrolled = vp(cx, &view);
    let c = cursor_of(cx, &view);
    assert!(cursor_visible(&scrolled, c, QUARTER), "{scrolled:?} {c:?}");
    press(cx, handle, "v");
    assert_eq!(current_view(cx, &view), View::Arrangement);
    press(cx, handle, "v");
    assert_eq!(current_view(cx, &view), View::PianoRoll);
    assert_eq!(vp(cx, &view), scrolled, "돌아올 때 스크롤 자리가 바뀐다");
}

/// 같은 왕복이지만 커서가 화면 밖으로 스크롤된 상태였다면 돌아올 때 커서로 되돌아간다(현재 구현의 선택.
/// 계획 2의 「커서 칸이 보이게」로 읽는다. 어긋나면 제안으로 적는다).
#[gpui_kit::test]
fn returning_from_arrangement_after_scrolling_away_snaps_to_cursor(cx: &mut TestAppContext) {
    start(cx, "return-snap");
    let (handle, view) = open(cx, project_with_clip(clip(0, 8 * BAR, &[note(0, 960, 60)])));
    press(cx, handle, "enter");
    cx.read(|cx| view.read(cx).piano_roll().scroll_by(50_000.0, 600.0));
    render(cx, handle);
    let c = cursor_of(cx, &view);
    assert!(!cursor_visible(&vp(cx, &view), c, QUARTER));
    press(cx, handle, "v v");
    assert_eq!(current_view(cx, &view), View::PianoRoll);
    let v = vp(cx, &view);
    assert!(cursor_visible(&v, c, QUARTER), "{v:?} {c:?}");
}

/// 창을 아주 작게 줄였다가(주 뷰가 0에 가깝거나 0) 되돌려도 패닉 없이 커서가 보이고, 줄어든 동안 누른 커서
/// 키의 따라가기가 되돌아올 때 적용된다.
#[gpui_kit::test]
fn window_shrink_then_regrow_keeps_cursor_visible(cx: &mut TestAppContext) {
    start(cx, "shrink-regrow");
    let (handle, view) = open(
        cx,
        project_with_clip(clip(0, 20 * BAR, &[note(0, 960, 60)])),
    );
    press(cx, handle, "enter");
    let full = vp(cx, &view);
    assert!(full.width > 0.0 && full.height > 0.0, "{full:?}");
    resize(cx, handle, 1.0, 1.0);
    let tiny = vp(cx, &view);
    println!("tiny viewport: {tiny:?}");
    // 줄어든 동안 커서를 멀리 옮긴다(크기 0이면 pending_follow, 아니면 바로 적용)
    for _ in 0..15 {
        press(cx, handle, "ctrl-l");
    }
    let c = cursor_of(cx, &view);
    assert_eq!(c.tick.get(), 15 * BAR);
    resize(cx, handle, 1280.0, 800.0);
    let back = vp(cx, &view);
    println!("regrown viewport: {back:?}");
    assert!(back.width > 0.0 && back.height > 0.0, "{back:?}");
    assert!(cursor_visible(&back, c, QUARTER), "{back:?} {c:?}");
}

/// 창 크기가 0을 거치지 않는 보통의 크기 변경은 세로 스크롤 자리를 지킨다(`set_size`의 「처음」 분기가
/// 다시 타지 않는가).
#[gpui_kit::test]
fn ordinary_resize_keeps_vertical_scroll(cx: &mut TestAppContext) {
    start(cx, "resize-keep");
    let (handle, view) = open(cx, project_with_clip(clip(0, 8 * BAR, &[note(0, 960, 60)])));
    press(cx, handle, "enter");
    let at = bounds_of(cx, handle, "main-view").center();
    wheel(cx, handle, at, -60.0);
    let scrolled = vp(cx, &view);
    resize(cx, handle, 1000.0, 700.0);
    let v = vp(cx, &view);
    assert_ne!(v.width, scrolled.width, "크기가 바뀌어야 뜻이 있다 {v:?}");
    assert_eq!(
        v.first_row, scrolled.first_row,
        "크기 변경이 세로 스크롤을 되돌린다"
    );
    assert_eq!(v.first_tick, scrolled.first_tick);
}

// ---------------------------------------------------------------- 새 영역(결함만 「수정」): 들어갈 때 보이는 범위

/// 먼 클립으로 들어갈 때 커서 칸이 뷰포트의 오른쪽 끝에 붙는다. 클립 안쪽은 한 스텝(960틱)만 보인다.
/// 깨뜨리지 못함(계획 2 「커서 칸이 보이게」는 만족). 수치를 기록해 제안의 근거로 둔다.
#[gpui_kit::test]
fn entering_far_clip_shows_only_one_step_of_the_clip(cx: &mut TestAppContext) {
    start(cx, "far-edge");
    let far = 50 * BAR;
    let (handle, view) = open(
        cx,
        project_with_clip(clip(far, 4 * BAR, &[note(0, 960, 60)])),
    );
    press(cx, handle, "enter");
    let v = vp(cx, &view);
    let c = cursor_of(cx, &view);
    let visible_after_cursor = v.last_tick() - c.tick.get() as f64;
    println!("entering far clip: visible ticks after cursor = {visible_after_cursor}");
    assert!(cursor_visible(&v, c, QUARTER), "{v:?} {c:?}");
    assert!(
        visible_after_cursor >= QUARTER as f64 - 0.001,
        "커서 칸은 보인다 {v:?} {c:?}"
    );
}
