//! S3-T7 적대적 리뷰 2회차의 깨뜨리기 시험(`docs/reports/s3-t7/review-2.md`). 픽스처는 `tests/search.rs`를
//! 베꼈다. 실패하는 시험은 구현자가 고칠 때까지 실패한 채로 둔다.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::mode;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, View};
use keyscore_app::pianoroll::Cursor;
use keyscore_app::search::{Direction, Position, Query, find_hits, step};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};
use proptest::prelude::*;

const BAR: u64 = 3840;

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review-s3t7r2-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        keyscore_app::clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-review-s3t7r2-{name}", std::process::id()),
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

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
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

fn setup_with(
    cx: &mut TestAppContext,
    name: &str,
    preset: Preset,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    if preset == Preset::Vim {
        cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    }
    let (handle, view) = cx.update(|cx| open_project_window(project(), cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
    set_cursor(cx, &view, 0, 72);
    (handle, view)
}

fn setup(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    setup_with(cx, name, Preset::Default)
}

fn set_cursor(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, tick: u64, pitch: u8) {
    cx.update(|cx| {
        view.read(cx).piano_roll().set_cursor(Cursor {
            tick: Tick::new(tick),
            pitch: Pitch::new(pitch).unwrap(),
        });
    });
}

fn cursor(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> (u64, u8) {
    cx.read(|cx| {
        let c = view.read(cx).piano_roll().cursor();
        (c.tick.get(), c.pitch.get())
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

fn bar_open(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).search_bar_open())
}

fn status(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<String> {
    cx.read(|cx| view.read(cx).search_status())
}

fn current_mode(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Mode {
    cx.read(|cx| view.read(cx).mode_state().mode())
}

fn focus_in_bar(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    cx.update_window(handle, |_, window, _| {
        let stack = window.context_stack();
        stack.iter().any(|c| c.contains("SearchBar")) && stack.iter().any(|c| c.contains("Input"))
    })
    .unwrap()
}

fn focus_in_info(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    cx.update_window(handle, |_, window, _| {
        let stack = window.context_stack();
        stack.iter().any(|c| c.contains("Dialog")) && stack.iter().any(|c| c.contains("NoteInfo"))
    })
    .unwrap()
}

fn lead_notes(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| {
        view.read(cx).document().read(cx).project().tracks()[0]
            .clips()
            .next()
            .unwrap()
            .1
            .notes()
            .len()
    })
}

fn undo_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).undo_len())
}

// ---------- 1. 팔레트에서 여는 길 ----------

/// 결정 1절 2: 팔레트로도 막대를 연다. 팔레트는 닫힐 때 문서로 포커스를 돌린 뒤 Action을 낸다. 그 순서에서도
/// 막대가 열린 채 입력란에 포커스가 있어야 한다.
#[gpui_kit::test]
fn palette_search_command_opens_bar_with_focus(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "palette-search");
    press(cx, handle, "cmd-shift-p");
    assert!(cx.read(|cx| view.read(cx).palette_open()));
    type_text(cx, handle, "검색");
    press(cx, handle, "enter");
    assert!(!cx.read(|cx| view.read(cx).palette_open()));
    assert!(
        bar_open(cx, &view),
        "팔레트의 「검색」이 막대를 연다(마지막 글: {})",
        last_message(cx, &view)
    );
    assert!(focus_in_bar(cx, handle), "포커스는 막대 입력란");
    type_text(cx, handle, "c#4");
    assert_eq!(status(cx, &view).as_deref(), Some("결과 2개"));
}

/// 결정 6절 5: 팔레트 경로는 다른 모드에서도 정보 팝업을 연다. 팔레트가 닫히며 문서로 돌린 포커스가 팝업을
/// 닫지 않아야 한다.
#[gpui_kit::test]
fn palette_note_info_in_step_mode_keeps_popup_focused(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "palette-info");
    run(cx, handle, mode::Step);
    assert_eq!(current_mode(cx, &view), Mode::Step);
    set_cursor(cx, &view, 0, 61);
    press(cx, handle, "cmd-shift-p");
    type_text(cx, handle, "노트 정보");
    press(cx, handle, "enter");
    assert!(
        cx.read(|cx| view.read(cx).note_info().is_some()),
        "팔레트의 「노트 정보」가 팝업을 연다(마지막 글: {})",
        last_message(cx, &view)
    );
    assert!(focus_in_info(cx, handle), "포커스는 팝업");
    press(cx, handle, "escape");
    assert!(cx.read(|cx| view.read(cx).note_info().is_none()));
    assert_eq!(current_mode(cx, &view), Mode::Step);
}

// ---------- 2. 막대 안의 키 ----------

/// REQ KM-14: 텍스트 입력 중의 `Cmd+Z`는 그 입력란의 undo다. 막대 입력란에서 `cmd-z`가 문서를 되돌리면 안 된다.
#[gpui_kit::test]
fn bar_cmd_z_does_not_undo_document(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "cmdz");
    set_cursor(cx, &view, 0, 61);
    press(cx, handle, "x");
    assert_eq!(lead_notes(cx, &view), 2);
    let undo = undo_len(cx, &view);
    press(cx, handle, "cmd-f");
    type_text(cx, handle, "c#4");
    press(cx, handle, "cmd-z");
    assert_eq!(lead_notes(cx, &view), 2, "문서는 그대로다(KM-14)");
    assert_eq!(undo_len(cx, &view), undo);
    assert!(bar_open(cx, &view));
}

/// 막대의 `Enter`가 막대를 닫은 뒤 같은 키가 문서의 `enter`(기본 피아노롤 `mode::Step`)로 새지 않는다.
#[gpui_kit::test]
fn bar_enter_does_not_leak_to_document_enter(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "enter-leak");
    press(cx, handle, "cmd-f");
    type_text(cx, handle, "c#4");
    press(cx, handle, "enter");
    assert!(!bar_open(cx, &view));
    assert_eq!(current_mode(cx, &view), Mode::Normal);
    // 빈 검색어의 Enter(닫기만)도 같다
    press(cx, handle, "cmd-f");
    press(cx, handle, "backspace");
    press(cx, handle, "enter");
    assert!(!bar_open(cx, &view));
    assert_eq!(current_mode(cx, &view), Mode::Normal);
    // Shift+Enter: 기본 피아노롤 Normal에는 shift-enter 바인딩이 없지만 막대가 삼키는지 함께 본다
    press(cx, handle, "cmd-f");
    press(cx, handle, "shift-enter");
    assert_eq!(current_mode(cx, &view), Mode::Normal);
}

/// 막대 안의 `Tab`: 결정은 막대의 키로 `Enter`·`Shift+Enter`·`Esc`만 적는다. `Tab`이 포커스를 버튼으로 옮겨
/// 포커스 이탈로 막대가 닫히면 친 검색어를 잃는다.
#[gpui_kit::test]
fn bar_tab_does_not_close_bar(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "tab");
    press(cx, handle, "cmd-f");
    type_text(cx, handle, "c#4");
    press(cx, handle, "tab");
    assert!(bar_open(cx, &view), "Tab이 막대를 닫았다");
}

// ---------- 3. vim 카운트 ----------

/// REQ 5.8.1 머리말: vim 프리셋에서 「반복은 숫자 카운트 접두로 한다」. vim `3n`은 세 번째 다음 결과다. 결정
/// 4·5절은 카운트를 적지 않는다.
#[gpui_kit::test]
fn vim_count_n_moves_count_results(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "count-n", Preset::Vim);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "/");
    type_text(cx, handle, "c#");
    // 지난 검색어를 만든다: Enter로 확정하면 (0, 61)
    press(cx, handle, "enter");
    assert_eq!(cursor(cx, &view), (0, 61));
    // c# 결과: (0,L,61) (480,B,37) (BAR,L,61) (BAR+960,B,49). 3n이면 (BAR+960, 49)
    press(cx, handle, "3 n");
    assert_eq!(cursor(cx, &view), (BAR + 960, 49), "3n은 세 번째 다음 결과");
}

// ---------- 4. 노트 없는 결과 여럿(리뷰 1 지적 1 수정의 경계) ----------

fn empty_pads_project() -> Project {
    let mut project = Project::new();
    let mut lead = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    let mut clip = Clip::new(Tick::ZERO, BAR).unwrap();
    clip.insert_note(note(0, 480, 61)).unwrap();
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
    // 클립도 없는 빈 트랙(자리 틱 0)
    project
        .push_track(Track::new(
            "Pad 3",
            Channel::new(3).unwrap(),
            TrackColor::new(3).unwrap(),
        ))
        .unwrap();
    project
}

/// 같은 틱의 노트 없는 결과 둘(Pad 1·Pad 2, BAR)과 클립 없는 트랙(Pad 3, 0)을 커서 음높이를 둔 채 앞뒤로 돈다.
#[test]
fn step_cycles_through_several_empty_track_hits() {
    let project = empty_pads_project();
    let hits = find_hits(&project, &Query::parse("pad").unwrap());
    assert_eq!(hits.len(), 3);
    for direction in [Direction::Forward, Direction::Backward] {
        let mut at = Position {
            track_index: 0,
            tick: Tick::ZERO,
            pitch: Pitch::new(72).unwrap(),
        };
        let mut seen = Vec::new();
        for _ in 0..hits.len() {
            let f = step(&project, &hits, at, direction).unwrap();
            let h = hits[f.index];
            seen.push(f.index);
            at = Position {
                track_index: project.track_index(h.track).unwrap(),
                tick: h.tick,
                pitch: if h.has_note { h.pitch } else { at.pitch },
            };
        }
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 3, "{direction:?}: 모든 결과를 한 번씩 지난다");
    }
}

proptest! {
    /// 창의 이동을 그대로 흉내 낸다: 노트 없는 결과로 가면 커서 음높이를 둔다(결정 4절 1). 그래도 결과 수만큼
    /// 걸으면 모든 결과를 한 번씩 지나고 돎은 한 번 이하다. (`search_pure.rs`의 성질 시험은 노트 없는 결과를
    /// 만들지 않고, 결과 음높이로 커서를 옮긴다.)
    #[test]
    fn step_with_kept_cursor_pitch_visits_every_hit_once(
        tracks in proptest::collection::vec(
            (proptest::option::of(0u64..3 * BAR), proptest::collection::vec((0u64..2 * BAR, 55u8..70), 0..4)),
            1..5,
        ),
        start_track in 0usize..5,
        start_tick in 0u64..4 * BAR,
        start_pitch in 50u8..80,
    ) {
        let mut project = Project::new();
        for (i, (clip_start, notes)) in tracks.iter().enumerate() {
            let mut t = Track::new(format!("x{i}"), Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
            if let Some(cs) = clip_start {
                let mut clip = Clip::new(Tick::new(*cs), 2 * BAR + 10).unwrap();
                for (tick, pitch) in notes {
                    clip.insert_note(note(*tick, 1, *pitch)).unwrap();
                }
                t.insert_clip(clip).unwrap();
            }
            project.push_track(t).unwrap();
        }
        let hits = find_hits(&project, &Query::parse("x").unwrap());
        prop_assert_eq!(hits.len(), tracks.len());
        for direction in [Direction::Forward, Direction::Backward] {
            let mut at = Position {
                track_index: start_track.min(tracks.len() - 1),
                tick: Tick::new(start_tick),
                pitch: Pitch::new(start_pitch).unwrap(),
            };
            let mut seen = std::collections::BTreeSet::new();
            let mut wraps = 0;
            for _ in 0..hits.len() {
                let f = step(&project, &hits, at, direction).unwrap();
                prop_assert!(seen.insert(f.index), "같은 결과를 두 번 골랐다: {:?}", hits[f.index]);
                wraps += usize::from(f.wrapped);
                let h = hits[f.index];
                at = Position {
                    track_index: project.track_index(h.track).unwrap(),
                    tick: h.tick,
                    pitch: if h.has_note { h.pitch } else { at.pitch },
                };
            }
            prop_assert!(wraps <= 1);
        }
    }
}
