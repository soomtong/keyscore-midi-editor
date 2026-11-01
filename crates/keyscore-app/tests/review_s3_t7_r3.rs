//! S3-T7 적대적 리뷰 3회차의 깨뜨리기 시험(`docs/reports/s3-t7/review-3.md`). 픽스처는 `tests/search.rs`를
//! 베꼈다. 실패하는 시험은 구현자가 고칠 때까지 실패한 채로 둔다.

use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::View;
use keyscore_app::note_info::info_lines;
use keyscore_app::pianoroll::Cursor;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Clip, Note, Pitch, PitchOffset, Project, Tick, Track, TrackColor, TrackId, Velocity,
};

const BAR: u64 = 3840;

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review-s3t7r3-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        keyscore_app::clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-review-s3t7r3-{name}", std::process::id()),
        );
    });
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    cx.run_until_parked();
    render(cx, handle);
}

fn type_text(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    cx.simulate_input(handle, text);
    cx.run_until_parked();
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

/// `tests/search.rs`와 같은 트랙 둘(Lead, Bass Line).
fn project() -> Project {
    let mut project = Project::new();
    let mut lead = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    let mut clip = Clip::new(Tick::ZERO, 2 * BAR).unwrap();
    clip.insert_note(note(0, 480, 61).with_lyric("반")).unwrap();
    clip.insert_note(note(960, 480, 60).with_lyric("짝"))
        .unwrap();
    clip.insert_note(note(BAR, 480, 61)).unwrap();
    lead.insert_clip(clip).unwrap();
    project.push_track(lead).unwrap();
    let mut bass = Track::new(
        "Bass Line",
        Channel::new(2).unwrap(),
        TrackColor::new(1).unwrap(),
    );
    let mut clip = Clip::new(Tick::ZERO, 2 * BAR).unwrap();
    clip.insert_note(note(480, 480, 37)).unwrap();
    clip.insert_note(note(BAR + 960, 480, 49).with_lyric("별"))
        .unwrap();
    bass.insert_clip(clip).unwrap();
    project.push_track(bass).unwrap();
    project
}

/// 노트 결과와 노트 없는 트랙 결과가 섞인다: 「pad」는 Lead의 가사 「pad」(0, BAR+480), 노트 없는 Pad 1·Pad 2
/// (BAR), 클립 없는 Pad 3(0)을 찾는다.
fn mixed_project() -> Project {
    let mut project = Project::new();
    let mut lead = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    let mut clip = Clip::new(Tick::ZERO, 2 * BAR).unwrap();
    clip.insert_note(note(0, 480, 61).with_lyric("pad"))
        .unwrap();
    clip.insert_note(note(BAR + 480, 480, 64).with_lyric("pad"))
        .unwrap();
    lead.insert_clip(clip).unwrap();
    project.push_track(lead).unwrap();
    for (name, color) in [("Pad 1", 1), ("Pad 2", 2)] {
        let mut pad = Track::new(
            name,
            Channel::new(2).unwrap(),
            TrackColor::new(color).unwrap(),
        );
        pad.insert_clip(Clip::new(Tick::new(BAR), BAR).unwrap())
            .unwrap();
        project.push_track(pad).unwrap();
    }
    project
        .push_track(Track::new(
            "Pad 3",
            Channel::new(3).unwrap(),
            TrackColor::new(3).unwrap(),
        ))
        .unwrap();
    project
}

/// 한 트랙, c#4 노트 `count`개(48틱 간격).
fn many_notes_project(count: u64) -> Project {
    let mut project = Project::new();
    let mut lead = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    let mut clip = Clip::new(Tick::ZERO, (count + 1) * 48).unwrap();
    for i in 0..count {
        clip.insert_note(note(i * 48, 24, 61)).unwrap();
    }
    lead.insert_clip(clip).unwrap();
    project.push_track(lead).unwrap();
    project
}

fn setup_project(
    cx: &mut TestAppContext,
    name: &str,
    project: Project,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = cx.update(|cx| open_project_window(project, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
    (handle, view)
}

fn set_cursor(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, tick: u64, pitch: u8) {
    cx.update(|cx| {
        view.read(cx).piano_roll().set_cursor(Cursor {
            tick: Tick::new(tick),
            pitch: Pitch::new(pitch).unwrap(),
        });
    });
}

/// (현재 트랙, 커서 틱, 커서 음높이).
fn place(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> (Option<TrackId>, u64, u8) {
    cx.read(|cx| {
        let v = view.read(cx);
        let c = v.piano_roll().cursor();
        (v.current_track(), c.tick.get(), c.pitch.get())
    })
}

fn last_message(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> String {
    cx.read(|cx| {
        view.read(cx)
            .messages()
            .last()
            .map(ToString::to_string)
            .unwrap_or_default()
    })
}

fn pending_count(cx: &mut TestAppContext) -> Option<u32> {
    cx.read(keymap::pending_count)
}

/// 현재 트랙을 Lead로 되돌리고 커서를 `from`에 둔 뒤, 막대로 `query`를 확정한다(한 칸). 현재 트랙을 바꾸는
/// 공개 API가 없어 Lead 트랙 이름 검색으로 되돌린다.
fn reset_and_search(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    from: (u64, u8),
    query: &str,
) {
    press(cx, handle, "/");
    type_text(cx, handle, "lead");
    press(cx, handle, "enter");
    set_cursor(cx, view, from.0, from.1);
    press(cx, handle, "/");
    type_text(cx, handle, query);
    press(cx, handle, "enter");
}

/// 같은 창에서 (시작 → `k key` 한 번)과 (시작 → `key`를 k번)이 같은 자리에 닿는지 본다. 시작은 Lead 트랙의
/// `from`에서 막대로 `query`를 한 칸 간 자리다.
fn assert_count_equals_repeat(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    from: (u64, u8),
    query: &str,
    key: &str,
    max: u32,
) {
    for k in 1..=max {
        reset_and_search(cx, handle, view, from, query);
        let origin = place(cx, view);
        press(cx, handle, &format!("{k} {key}"));
        let counted = place(cx, view);
        reset_and_search(cx, handle, view, from, query);
        assert_eq!(place(cx, view), origin);
        for _ in 0..k {
            press(cx, handle, key);
        }
        let repeated = place(cx, view);
        assert_eq!(
            counted, repeated,
            "{origin:?}에서 `{k} {key}`({counted:?})와 `{key}` {k}번({repeated:?})이 다르다"
        );
    }
}

// ---------- 1. 카운트 걷기 = 키를 그만큼 누른 것 ----------

/// 리뷰 2 지적 2 수정: `3 n`은 `n`을 세 번 누른 것과 같은 자리다(vim과 같다). 결과 사이·결과 위에서 시작해
/// 결과 수(4)를 넘는 카운트까지 앞뒤로 본다.
#[gpui_kit::test]
fn count_walk_equals_repeated_keys_on_note_hits(cx: &mut TestAppContext) {
    let (handle, view) = setup_project(cx, "count-eq-notes", project());
    for from in [(0, 60), (0, 61), (700, 72), (9 * BAR, 72)] {
        assert_count_equals_repeat(cx, handle, &view, from, "c#", "n", 5);
        assert_count_equals_repeat(cx, handle, &view, from, "c#", "shift-n", 5);
    }
}

/// 노트 없는 트랙 결과가 섞여도(커서 음높이를 두는 결과, 결정 3절 5·4절 1) 카운트 걷기는 키를 반복한 것과 같다.
#[gpui_kit::test]
fn count_walk_equals_repeated_keys_with_empty_track_hits(cx: &mut TestAppContext) {
    let (handle, view) = setup_project(cx, "count-eq-mixed", mixed_project());
    set_cursor(cx, &view, 0, 72);
    press(cx, handle, "/");
    type_text(cx, handle, "pad");
    press(cx, handle, "enter");
    assert!(
        last_message(cx, &view).contains("/5"),
        "결과 다섯: {}",
        last_message(cx, &view)
    );
    for from in [(0, 72), (BAR, 72), (0, 61), (9 * BAR, 50)] {
        assert_count_equals_repeat(cx, handle, &view, from, "pad", "n", 6);
        assert_count_equals_repeat(cx, handle, &view, from, "pad", "shift-n", 6);
    }
}

/// 결정 5절 4: 카운트 걷기 중에 돌았으면(count ≤ 결과 수여도) 돎 문구를 남긴다.
#[gpui_kit::test]
fn count_walk_within_hits_reports_wrap(cx: &mut TestAppContext) {
    let (handle, view) = setup_project(cx, "count-wrap-msg", project());
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "/");
    type_text(cx, handle, "c#");
    press(cx, handle, "enter");
    // 결과 넷 중 마지막(BAR+960, Bass)에서 2n이면 처음(0, Lead)을 지나 (480, Bass)
    press(cx, handle, "3 n");
    assert_eq!(place(cx, &view).1, BAR + 960);
    press(cx, handle, "2 n");
    assert_eq!(place(cx, &view).1, 480);
    assert!(
        last_message(cx, &view).ends_with("끝에서 처음으로 돌아왔다"),
        "{}",
        last_message(cx, &view)
    );
}

// ---------- 2. 카운트가 새지 않는다 ----------

/// 지난 검색어가 없을 때 `3 n`의 카운트는 버려진다(다음 모션으로 새지 않는다).
#[gpui_kit::test]
fn count_with_no_last_search_does_not_leak(cx: &mut TestAppContext) {
    let (handle, view) = setup_project(cx, "count-no-last", project());
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "3 n");
    assert_eq!(last_message(cx, &view), "검색어가 없다");
    assert_eq!(pending_count(cx), None);
    let before = place(cx, &view);
    press(cx, handle, "k");
    let one = place(cx, &view);
    assert_eq!(one.2, before.2 + 1, "k는 반음 하나(카운트가 새지 않음)");
}

/// `3 /`로 연 막대에서 `Enter`한 뒤 카운트가 남아 다음 모션에 붙지 않는다.
#[gpui_kit::test]
fn count_before_slash_does_not_leak_past_bar(cx: &mut TestAppContext) {
    let (handle, view) = setup_project(cx, "count-slash", project());
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "3 /");
    type_text(cx, handle, "c#4");
    press(cx, handle, "enter");
    assert_eq!(place(cx, &view).1, 0);
    assert_eq!(pending_count(cx), None);
    let before = place(cx, &view);
    press(cx, handle, "k");
    assert_eq!(place(cx, &view).2, before.2 + 1);
}

// ---------- 3. 큰 카운트의 비용 ----------

/// `step`은 부를 때마다 결과 전체를 훑고 `Vec`를 새로 만든다. 카운트 걷기는 그것을 min(count, n)번 부르므로
/// O(min(count, n) · n)이다. 노트 10,000개(NF-8의 규모)에서 `9999 n` 한 번이 화면을 멈추면 안 된다. 기준은
/// 같은 창의 `n` 한 번(결과를 찾는 비용 포함)의 20배.
#[gpui_kit::test]
fn large_count_walk_is_not_quadratic(cx: &mut TestAppContext) {
    let (handle, view) = setup_project(cx, "count-cost", many_notes_project(10_000));
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "/");
    type_text(cx, handle, "c#4");
    press(cx, handle, "enter");
    assert_eq!(place(cx, &view).1, 0);
    let t = Instant::now();
    press(cx, handle, "n");
    let one = t.elapsed();
    let t = Instant::now();
    press(cx, handle, "9 9 9 9 n");
    let many = t.elapsed();
    eprintln!("n 한 번: {one:?}, 9999 n: {many:?}");
    // 1 + 9999 = 10000걸음은 결과 10,000개를 한 바퀴 돌아 첫 결과(틱 0)로 온다(구현자가 clippy erasing_op 때문에 상수로 고침)
    assert_eq!(place(cx, &view).1, 0);
    assert!(
        many < one * 20 + Duration::from_millis(50),
        "9999 n이 {many:?}(n 한 번 {one:?})"
    );
}

// ---------- 4. cent 0.1 표시 ----------

fn pitch_line(offset_units: i32) -> String {
    let n = note(0, 480, 61)
        .with_offset(PitchOffset::new(offset_units).unwrap())
        .unwrap();
    let mut t = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let mut clip = Clip::new(Tick::ZERO, BAR).unwrap();
    clip.insert_note(n.clone()).unwrap();
    let clip_id = t.insert_clip(clip).unwrap();
    let track_id = t.id();
    let mut project = Project::new();
    project.push_track(t).unwrap();
    info_lines(&project, track_id, clip_id, &n)
        .into_iter()
        .find(|l| l.label == "음높이")
        .unwrap()
        .value
}

/// 아래로 굽은 노트는 아무리 작아도 「+」로 보이면 안 된다(반올림한 값이 0이어도 부호는 오프셋의 것).
#[test]
fn tiny_negative_offset_is_not_shown_as_plus() {
    let text = pitch_line(-1);
    assert!(!text.contains('+'), "-1단위 오프셋이 {text}");
}

/// 0이 아닌 오프셋은 0으로 읽히지 않는다(리뷰 2 제안 4의 뜻). 0.05 cent 미만도 「+0 cent」가 아니어야 한다.
#[test]
fn tiny_positive_offset_is_not_shown_as_zero() {
    let text = pitch_line(PitchOffset::SEMITONE / 4000);
    assert_ne!(text, "C#4 (61) +0 cent");
}

/// 반올림 경계와 끝: 99.95 cent 이상은 「+100 cent」, ±1200 cent, 소수 한 자리.
#[test]
fn offset_rounding_edges() {
    let semitone = i64::from(PitchOffset::SEMITONE);
    let units = |cents_x1000: i64| i32::try_from(semitone * cents_x1000 / 100_000).unwrap();
    assert_eq!(pitch_line(units(99_960)), "C#4 (61) +100 cent");
    assert_eq!(pitch_line(units(12_340)), "C#4 (61) +12.3 cent");
    assert_eq!(pitch_line(-units(12_360)), "C#4 (61) -12.4 cent");
    assert_eq!(pitch_line(PitchOffset::MAX), "C#4 (61) +1200 cent");
    assert_eq!(pitch_line(-PitchOffset::MAX), "C#4 (61) -1200 cent");
}
