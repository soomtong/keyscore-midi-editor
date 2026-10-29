//! S3-T4a Step 입력(REQ 5.1, 5.6, 5.10, 5.11). 헤드리스 창에서 키를 눌러 문서·커서·상태줄을 본다. 규칙은
//! `docs/decisions/2026-10-29-s3-t4-step-input-details.md`, 시험 목록은 `docs/reports/s3-t4a/plan.md`.

use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::{KeyLayout, LengthCode};
use keyscore_app::app_state::AppSettings;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, ModeEvent, View};
use keyscore_app::pianoroll::{Cursor, Selection};
use keyscore_app::step::{StepLength, StepStatus};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;
const WAIT: Duration = Duration::from_millis(300);

/// (절대 시작, 길이, 음높이)
type Abs = (u64, u32, u8);

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-step-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
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

/// 트랙 하나, 클립들(시작, 길이, 노트(클립 기준)).
fn project(clips: &[(u64, u64, &[Abs])]) -> Project {
    let mut project = Project::new();
    let mut track = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    for &(s, l, notes) in clips {
        let mut clip = Clip::new(Tick::new(s), l).unwrap();
        for &(ns, nl, np) in notes {
            clip.insert_note(note(ns, nl, np)).unwrap();
        }
        track.insert_clip(clip).unwrap();
    }
    project.push_track(track).unwrap();
    project
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    for key in keys.split_whitespace() {
        cx.simulate_keystrokes(handle, key);
        cx.run_until_parked();
        render(cx, handle);
    }
}

fn wait(cx: &mut TestAppContext, handle: AnyWindowHandle, d: Duration) {
    cx.executor().advance_clock(d);
    cx.run_until_parked();
    render(cx, handle);
}

fn event(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    e: ModeEvent,
) {
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |this, cx| this.apply_mode_event(e, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn set_layout(cx: &mut TestAppContext, handle: AnyWindowHandle, layout: KeyLayout) {
    cx.update(|cx| AppSettings::update(cx, |s| s.input.key_layout = layout));
    cx.run_until_parked();
    render(cx, handle);
}

fn set_cursor(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, tick: u64) {
    cx.update(|cx| {
        view.read(cx).piano_roll().set_cursor(Cursor {
            tick: Tick::new(tick),
            pitch: Pitch::new(60).unwrap(),
        });
    });
}

/// 피아노롤 뷰의 Step 모드 창. 커서는 틱 0.
fn setup(
    cx: &mut TestAppContext,
    name: &str,
    layout: KeyLayout,
    p: Project,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    set_layout(cx, handle, layout);
    event(cx, handle, &view, ModeEvent::ToggleView);
    set_cursor(cx, &view, 0);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    (handle, view)
}

/// 트랙 0의 모든 노트(절대 시작), 정렬.
fn notes(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<Abs> {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        let mut v: Vec<Abs> = track
            .clips()
            .flat_map(|(_, c)| {
                c.notes().iter().map(move |n| {
                    (
                        c.start().get() + n.start().get(),
                        n.length(),
                        n.pitch().get(),
                    )
                })
            })
            .collect();
        v.sort_unstable();
        v
    })
}

/// 트랙 0의 클립(시작, 길이).
fn clips(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<(u64, u64)> {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        track
            .clips()
            .map(|(_, c)| (c.start().get(), c.length()))
            .collect()
    })
}

fn cursor(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> u64 {
    cx.read(|cx| view.read(cx).piano_roll().cursor().tick.get())
}

fn status(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> StepStatus {
    cx.read(|cx| view.read(cx).step_status())
}

fn label(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> String {
    cx.read(|cx| view.read(cx).mode_label())
}

fn path(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<&'static str> {
    cx.read(|cx| view.read(cx).context_path().to_vec())
}

fn undo_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).document().read(cx).undo_len())
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

fn mode(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Mode {
    cx.read(|cx| view.read(cx).mode_state().mode())
}

fn len(code: LengthCode, dotted: bool) -> StepLength {
    StepLength { code, dotted }
}

// ---------- 1·2. REQ 5.6 입력 예 표 ----------

/// 표의 한 행: 키 묶음(묶음 사이에 대기 시간을 흘린다), 새로 생기는 노트, 뒤의 길이·옥타브.
struct Row {
    input: &'static str,
    keys: &'static [&'static str],
    new: &'static [Abs],
    length: StepLength,
    octave: u8,
}

fn rows(piano: bool) -> Vec<Row> {
    use LengthCode::{Eighth, Half, Quarter, Sixteenth};
    let k = |solfege: &'static [&'static str], keyboard: &'static [&'static str]| {
        if piano { keyboard } else { solfege }
    };
    vec![
        Row {
            input: "c",
            keys: k(&["c"], &["a"]),
            new: &[(0, 960, 60)],
            length: len(Quarter, false),
            octave: 4,
        },
        Row {
            input: "5c",
            keys: k(&["5 c"], &["5 a"]),
            new: &[(960, 240, 60)],
            length: len(Sixteenth, false),
            octave: 4,
        },
        Row {
            input: "d e f",
            keys: k(&["d", "e", "f"], &["s", "d", "f"]),
            new: &[(1200, 240, 62), (1440, 240, 64), (1680, 240, 65)],
            length: len(Sixteenth, false),
            octave: 4,
        },
        Row {
            input: "Shift+g",
            keys: k(&["shift-g"], &["shift-g"]),
            new: &[(1920, 120, 67)],
            length: len(Sixteenth, false),
            octave: 4,
        },
        Row {
            input: "Shift+3 a",
            keys: k(&["# a"], &["# h"]),
            new: &[(2040, 1440, 69)],
            length: len(Quarter, true),
            octave: 4,
        },
        Row {
            input: "c5",
            keys: k(&["c 5"], &["a 5"]),
            new: &[(3480, 1440, 72)],
            length: len(Quarter, true),
            octave: 5,
        },
        Row {
            input: "4e4",
            keys: k(&["4 e 4"], &["4 d 4"]),
            new: &[(4920, 480, 64)],
            length: len(Eighth, false),
            octave: 4,
        },
        Row {
            input: "5f#3",
            keys: k(&["5 f # 3"], &["5 t 3"]),
            new: &[(5400, 240, 54)],
            length: len(Sixteenth, false),
            octave: 3,
        },
        Row {
            input: "4na 4nb 4nc",
            keys: k(&["4 n a", "4 n b", "4 n c"], &["4 n h", "4 n j", "4 n a"]),
            new: &[(5640, 320, 57), (5960, 320, 59), (6280, 320, 48)],
            length: len(Eighth, false),
            octave: 3,
        },
        Row {
            input: "2 Space",
            keys: k(&["2 space"], &["2 space"]),
            new: &[],
            length: len(Half, false),
            octave: 3,
        },
        Row {
            input: "5 Enter",
            keys: k(&["5 enter"], &["5 enter"]),
            new: &[],
            length: len(Sixteenth, false),
            octave: 3,
        },
        Row {
            input: "0",
            keys: k(&["0"], &["0"]),
            new: &[],
            length: len(Quarter, false),
            octave: 3,
        },
    ]
}

fn run_table(cx: &mut TestAppContext, layout: KeyLayout, name: &str) {
    let (handle, view) = setup(cx, name, layout, project(&[]));
    let mut expected: Vec<Abs> = Vec::new();
    for row in rows(layout == KeyLayout::Piano) {
        for chunk in row.keys {
            press(cx, handle, chunk);
            wait(cx, handle, WAIT);
        }
        expected.extend_from_slice(row.new);
        expected.sort_unstable();
        assert_eq!(notes(cx, &view), expected, "{layout:?} 행 {}", row.input);
        let s = status(cx, &view);
        assert_eq!(
            (s.length, s.octave),
            (row.length, row.octave),
            "{layout:?} 행 {}",
            row.input
        );
        assert_eq!(s.composing, None, "{layout:?} 행 {}", row.input);
    }
    // 쉼표 2분(1920) 뒤 커서
    assert_eq!(cursor(cx, &view), 6600 + 1920);
}

#[gpui_kit::test]
fn req_5_6_rows_solfege_match_table(cx: &mut TestAppContext) {
    run_table(cx, KeyLayout::Solfege, "table-solfege");
}

#[gpui_kit::test]
fn req_5_6_rows_piano_match_table(cx: &mut TestAppContext) {
    run_table(cx, KeyLayout::Piano, "table-piano");
}

// ---------- 3–5. 조합과 확정 ----------

#[gpui_kit::test]
fn octave_wait_commits_after_timeout_with_fake_clock(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "wait", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "c");
    assert!(notes(cx, &view).is_empty());
    assert!(
        path(cx, &view).contains(&"Pending"),
        "{:?}",
        path(cx, &view)
    );
    wait(cx, handle, Duration::from_millis(299));
    assert!(notes(cx, &view).is_empty(), "299 ms에는 아직 조합 중");
    assert_eq!(status(cx, &view).composing.as_deref(), Some("c_"));
    wait(cx, handle, Duration::from_millis(1));
    assert_eq!(notes(cx, &view), vec![(0, 960, 60)]);
    assert!(!path(cx, &view).contains(&"Pending"));
    assert_eq!(cursor(cx, &view), 960);
}

#[gpui_kit::test]
fn octave_wait_setting_changes_timeout(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "wait-setting", KeyLayout::Solfege, project(&[]));
    cx.update(|cx| AppSettings::update(cx, |s| s.input.octave_wait_ms = 1000));
    press(cx, handle, "c");
    wait(cx, handle, Duration::from_millis(999));
    assert!(notes(cx, &view).is_empty());
    wait(cx, handle, Duration::from_millis(1));
    assert_eq!(notes(cx, &view), vec![(0, 960, 60)]);
}

#[gpui_kit::test]
fn enter_and_next_note_commit_immediately(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "commit", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "c enter");
    assert_eq!(notes(cx, &view), vec![(0, 960, 60)]);
    press(cx, handle, "d e");
    assert_eq!(notes(cx, &view), vec![(0, 960, 60), (960, 960, 62)]);
    assert_eq!(status(cx, &view).composing.as_deref(), Some("e_"));
    // 늦은 타이머(앞 음 d의 것)가 지금 조합을 건드리지 않는다
    wait(cx, handle, Duration::from_millis(200));
    assert_eq!(notes(cx, &view).len(), 2);
    wait(cx, handle, Duration::from_millis(100));
    assert_eq!(notes(cx, &view).len(), 3);
}

// ---------- 6–8. 길이 ----------

#[gpui_kit::test]
fn shift_note_is_half_once_not_sticky(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "half", KeyLayout::Piano, project(&[]));
    press(cx, handle, "4 shift-a enter s enter");
    assert_eq!(notes(cx, &view), vec![(0, 240, 60), (240, 480, 62)]);
    assert_eq!(status(cx, &view).length, len(LengthCode::Eighth, false));
}

#[gpui_kit::test]
fn length_codes_and_dotted_are_sticky_zero_resets_to_setting(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "lengths", KeyLayout::Solfege, project(&[]));
    let cases = [
        ("1", len(LengthCode::Whole, false)),
        ("!", len(LengthCode::Whole, true)),
        ("2", len(LengthCode::Half, false)),
        ("@", len(LengthCode::Half, true)),
        ("3", len(LengthCode::Quarter, false)),
        ("#", len(LengthCode::Quarter, true)),
        ("4", len(LengthCode::Eighth, false)),
        ("$", len(LengthCode::Eighth, true)),
        ("5", len(LengthCode::Sixteenth, false)),
        ("%", len(LengthCode::Sixteenth, true)),
        ("6", len(LengthCode::ThirtySecond, false)),
        ("^", len(LengthCode::ThirtySecond, true)),
        ("7", len(LengthCode::SixtyFourth, false)),
        ("&", len(LengthCode::SixtyFourth, true)),
    ];
    for (key, want) in cases {
        press(cx, handle, key);
        assert_eq!(status(cx, &view).length, want, "{key}");
    }
    // 점64분 두 음: 스티키
    press(cx, handle, "c enter d enter");
    assert_eq!(notes(cx, &view), vec![(0, 90, 60), (90, 90, 62)]);
    press(cx, handle, "0");
    assert_eq!(status(cx, &view).length, len(LengthCode::Quarter, false));
    // 기본 길이 설정을 8분으로
    cx.update(|cx| AppSettings::update(cx, |s| s.input.step_default_length = LengthCode::Eighth));
    press(cx, handle, "2 0");
    assert_eq!(status(cx, &view).length, len(LengthCode::Eighth, false));
}

#[gpui_kit::test]
fn triplet_prefix_applies_to_next_note_or_rest_only(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "triplet", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "n c enter d enter");
    assert_eq!(notes(cx, &view), vec![(0, 640, 60), (640, 960, 62)]);
    press(cx, handle, "n space");
    assert_eq!(cursor(cx, &view), 1600 + 640);
    press(cx, handle, "e enter");
    assert_eq!(notes(cx, &view).last(), Some(&(2240, 960, 64)));
}

// ---------- 9–12. 쉼표·지우기·Tab ----------

#[gpui_kit::test]
fn space_rests_and_does_not_create_clip(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "rest", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "space");
    assert_eq!(cursor(cx, &view), 960);
    assert!(clips(cx, &view).is_empty());
    press(cx, handle, "4 space");
    assert_eq!(cursor(cx, &view), 1440);
    assert_eq!(undo_len(cx, &view), 0);
}

#[gpui_kit::test]
fn space_in_clip_pushes_later_notes(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "rest-push",
        KeyLayout::Solfege,
        project(&[(0, BAR, &[(0, 960, 60)])]),
    );
    press(cx, handle, "space");
    assert_eq!(notes(cx, &view), vec![(960, 960, 60)]);
    assert_eq!(undo_len(cx, &view), 1);
}

#[gpui_kit::test]
fn backspace_deletes_step_and_pulls_later_notes(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "back",
        KeyLayout::Solfege,
        project(&[(0, BAR, &[(0, 960, 60), (960, 960, 62), (1920, 960, 64)])]),
    );
    set_cursor(cx, &view, 1920);
    press(cx, handle, "backspace");
    assert_eq!(notes(cx, &view), vec![(0, 960, 60), (960, 960, 64)]);
    assert_eq!(cursor(cx, &view), 960);
    assert_eq!(clips(cx, &view), vec![(0, BAR)]);
    assert_eq!(undo_len(cx, &view), 1);
    // 클립 시작에서는 지울 스텝이 없다: 커서만 0에서 멈춤
    set_cursor(cx, &view, 0);
    press(cx, handle, "backspace");
    assert_eq!(cursor(cx, &view), 0);
    assert_eq!(undo_len(cx, &view), 1);
}

#[gpui_kit::test]
fn backspace_while_composing_discards_composition(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "back-compose", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "c backspace");
    wait(cx, handle, WAIT);
    assert!(notes(cx, &view).is_empty());
    assert_eq!(cursor(cx, &view), 0);
    assert_eq!(status(cx, &view).composing, None);
}

#[gpui_kit::test]
fn tab_confirms_and_advances_shift_tab_steps_back(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "tab", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "c tab");
    assert_eq!(notes(cx, &view), vec![(0, 960, 60)]);
    assert_eq!(cursor(cx, &view), 960);
    press(cx, handle, "tab");
    assert_eq!(cursor(cx, &view), 1920, "조합이 없으면 커서만 한 스텝");
    assert_eq!(undo_len(cx, &view), 1);
    press(cx, handle, "shift-tab shift-tab");
    assert_eq!(cursor(cx, &view), 0);
    assert_eq!(
        notes(cx, &view),
        vec![(0, 960, 60)],
        "Shift+Tab은 지우지 않는다"
    );
    press(cx, handle, "shift-tab");
    assert_eq!(cursor(cx, &view), 0);
}

// ---------- 13·14. 화음·타이 ----------

#[gpui_kit::test]
fn ctrl_note_builds_chord_plain_note_or_tab_advances(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "chord",
        KeyLayout::Solfege,
        project(&[(0, BAR, &[(0, 960, 72)])]),
    );
    press(cx, handle, "ctrl-c ctrl-e g");
    wait(cx, handle, WAIT);
    // 첫 화음 음만 뒤 노트(C5)를 밀었다
    assert_eq!(
        notes(cx, &view),
        vec![(0, 960, 60), (0, 960, 64), (0, 960, 67), (960, 960, 72)]
    );
    assert_eq!(cursor(cx, &view), 960);
    assert_eq!(undo_len(cx, &view), 3, "음 하나가 undo 하나");
    press(cx, handle, "ctrl-d");
    wait(cx, handle, WAIT);
    assert!(status(cx, &view).chord_open);
    assert_eq!(cursor(cx, &view), 960);
    press(cx, handle, "tab");
    assert!(!status(cx, &view).chord_open);
    assert_eq!(cursor(cx, &view), 1920);
}

#[gpui_kit::test]
fn tie_extends_previous_step_and_advances(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "tie",
        KeyLayout::Solfege,
        project(&[(0, BAR, &[(0, 480, 72)])]),
    );
    press(cx, handle, "c ~");
    assert_eq!(notes(cx, &view), vec![(0, 1920, 60), (1920, 480, 72)]);
    assert_eq!(cursor(cx, &view), 1920);
    assert_eq!(undo_len(cx, &view), 2);
    // 화음도 모두 늘린다
    press(cx, handle, "4 ctrl-d f enter ~");
    assert_eq!(
        notes(cx, &view),
        vec![
            (0, 1920, 60),
            (1920, 960, 62),
            (1920, 960, 65),
            (2880, 480, 72)
        ]
    );
    // 쉼표 뒤에는 직전 스텝이 없다: 거부
    press(cx, handle, "space ~");
    assert!(
        last_message(cx, &view).contains("타이"),
        "{}",
        last_message(cx, &view)
    );
    assert_eq!(notes(cx, &view).len(), 4);
}

// ---------- 15–17. 옥타브·변화표 ----------

#[gpui_kit::test]
fn octave_keys_change_sticky_octave_and_stop_at_bounds(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "octave", KeyLayout::Piano, project(&[]));
    press(cx, handle, "] x");
    assert_eq!(status(cx, &view).octave, 6);
    press(cx, handle, "[ z z");
    assert_eq!(status(cx, &view).octave, 3);
    press(cx, handle, "z z z z z");
    assert_eq!(status(cx, &view).octave, 0);
    press(cx, handle, "] ] ] ] ] ] ] ] ] ]");
    assert_eq!(status(cx, &view).octave, 9);
    // oct9 j = B9(131): 거부, 노트 없음, 커서 그대로
    press(cx, handle, "j enter");
    assert!(notes(cx, &view).is_empty());
    assert_eq!(cursor(cx, &view), 0);
    assert!(last_message(cx, &view).contains("0–127"));
    press(cx, handle, "g enter");
    assert_eq!(notes(cx, &view), vec![(0, 960, 127)]);
}

#[gpui_kit::test]
fn piano_octave_digit_sets_row_base(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "piano-row", KeyLayout::Piano, project(&[]));
    press(cx, handle, "k 3");
    assert_eq!(notes(cx, &view), vec![(0, 960, 60)], "oct4 k 3 = C4");
    assert_eq!(status(cx, &view).octave, 3);
    press(cx, handle, "a enter");
    assert_eq!(notes(cx, &view)[1], (960, 960, 48), "이후 a는 C3");
}

#[gpui_kit::test]
fn solfege_accidentals_only_while_composing(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "accidental", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "c # enter c + enter c - enter c # # enter");
    assert_eq!(
        notes(cx, &view).iter().map(|n| n.2).collect::<Vec<_>>(),
        vec![61, 61, 59, 62]
    );
    // 조합이 없을 때 #은 점4분음표
    press(cx, handle, "#");
    assert_eq!(status(cx, &view).length, len(LengthCode::Quarter, true));
    assert_eq!(notes(cx, &view).len(), 4);
}

// ---------- 18–20. 클립·삽입·undo ----------

#[gpui_kit::test]
fn outside_clip_creates_bar_clip_and_extends_at_end(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "clip", KeyLayout::Solfege, project(&[]));
    set_cursor(cx, &view, BAR + 960);
    press(cx, handle, "c enter");
    assert_eq!(clips(cx, &view), vec![(BAR, BAR)]);
    assert_eq!(notes(cx, &view), vec![(BAR + 960, 960, 60)]);
    // 클립 끝 = 커서: 그 클립을 마디 단위로 늘린다(새 클립이 아니다)
    set_cursor(cx, &view, 2 * BAR);
    press(cx, handle, "d enter");
    assert_eq!(clips(cx, &view), vec![(BAR, 2 * BAR)]);
    assert_eq!(undo_len(cx, &view), 2);
}

#[gpui_kit::test]
fn insert_pushes_later_notes_and_rejects_next_clip_overlap(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "insert",
        KeyLayout::Solfege,
        project(&[
            (0, BAR, &[(0, 960, 60), (2880, 960, 64)]),
            (BAR + 1920, BAR, &[]),
        ]),
    );
    set_cursor(cx, &view, 960);
    press(cx, handle, "d enter");
    assert_eq!(
        notes(cx, &view),
        vec![(0, 960, 60), (960, 960, 62), (3840, 960, 64)]
    );
    assert_eq!(
        clips(cx, &view)[0],
        (0, BAR + 1920),
        "다음 클립 시작까지만 늘림"
    );
    // 2분음표로 한 번 더 밀면 E4가 다음 클립에 닿는다: 거부, 문서·커서 그대로
    let before = notes(cx, &view);
    press(cx, handle, "2 e enter");
    assert_eq!(notes(cx, &view), before);
    assert_eq!(cursor(cx, &view), 1920);
    assert_eq!(undo_len(cx, &view), 1);
}

#[gpui_kit::test]
fn each_commit_is_one_undo_unit(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "undo", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "c enter d enter e enter");
    assert_eq!(undo_len(cx, &view), 3);
    press(cx, handle, "cmd-z");
    assert_eq!(notes(cx, &view), vec![(0, 960, 60), (960, 960, 62)]);
    // 조합 중 cmd-z: 확정한 뒤 되돌린다(그 음이 없어진다)
    press(cx, handle, "f cmd-z");
    wait(cx, handle, WAIT);
    assert_eq!(notes(cx, &view), vec![(0, 960, 60), (960, 960, 62)]);
    press(cx, handle, "cmd-z cmd-z");
    assert!(notes(cx, &view).is_empty());
    assert!(
        clips(cx, &view).is_empty(),
        "첫 음을 되돌리면 만든 클립도 사라진다"
    );
}

// ---------- 21·22. 나가기·상태줄 ----------

#[gpui_kit::test]
fn leaving_step_commits_composition(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "leave", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "c escape");
    assert_eq!(mode(cx, &view), Mode::Normal);
    assert_eq!(notes(cx, &view), vec![(0, 960, 60)]);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    press(cx, handle, "d ctrl-tab");
    assert_eq!(mode(cx, &view), Mode::Sequence);
    assert_eq!(notes(cx, &view), vec![(0, 960, 60), (960, 960, 62)]);
    wait(cx, handle, WAIT);
    assert_eq!(
        notes(cx, &view).len(),
        2,
        "나간 뒤 타이머는 아무것도 하지 않는다"
    );
}

#[gpui_kit::test]
fn status_shows_length_octave_and_composition(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "status", KeyLayout::Solfege, project(&[]));
    assert_eq!(label(cx, &view), "STEP ♩ oct4");
    press(cx, handle, "5 c");
    assert_eq!(label(cx, &view), "STEP 1/16 oct4 5c_");
    press(cx, handle, "#");
    assert_eq!(label(cx, &view), "STEP 1/16 oct4 5c#_");
    press(cx, handle, "enter #");
    assert_eq!(label(cx, &view), "STEP ♩. oct4");
}

// ---------- 23·24. 키맵 ----------

/// Step 경로에서 키 하나가 내는 Action 이름.
fn action_for(cx: &mut TestAppContext, handle: AnyWindowHandle, key: &str) -> Option<String> {
    cx.update_window(handle, |_, window, cx| {
        let stroke = gpui_kit::Keystroke::parse(key).unwrap();
        let stack = window.context_stack();
        let (found, _) = cx
            .key_bindings()
            .borrow()
            .bindings_for_input(&[stroke], &stack);
        found.first().map(|b| b.action().name().to_string())
    })
    .unwrap()
}

#[gpui_kit::test]
fn step_contexts_have_no_note_key_conflicts(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "conflict", KeyLayout::Piano, project(&[]));
    for preset in [Preset::Default, Preset::Vim] {
        for layout in [KeyLayout::Piano, KeyLayout::Solfege] {
            set_layout(cx, handle, layout);
            cx.update(|cx| keymap::set_preset(cx, preset));
            cx.run_until_parked();
            render(cx, handle);
            let want = if layout == KeyLayout::Piano {
                "Piano"
            } else {
                "Solfege"
            };
            assert_eq!(path(cx, &view), vec!["Step", want]);
            for key in [
                "n",
                "0",
                "1",
                "2",
                "3",
                "4",
                "5",
                "6",
                "7",
                "[",
                "]",
                "space",
                "tab",
                "enter",
                "backspace",
                "~",
            ] {
                let got = action_for(cx, handle, key);
                assert!(
                    got.as_deref()
                        .is_some_and(|a| a.starts_with("step::") && a != "step::Note"),
                    "{preset:?} {layout:?} {key}: {got:?}"
                );
            }
            // 8·9는 조합 밖에서 바인딩이 없다(길이 코드는 0–7). 음 키가 아닌 것만 본다.
            for key in ["8", "9"] {
                assert_eq!(
                    action_for(cx, handle, key),
                    None,
                    "{preset:?} {layout:?} {key}"
                );
            }
            if layout == KeyLayout::Piano {
                assert_eq!(
                    action_for(cx, handle, "z").as_deref(),
                    Some("step::OctaveDown")
                );
                assert_eq!(
                    action_for(cx, handle, "x").as_deref(),
                    Some("step::OctaveUp")
                );
                for key in [
                    "a", "w", "s", "e", "d", "f", "t", "g", "y", "h", "u", "j", "k", "o", "l", "p",
                ] {
                    assert_eq!(
                        action_for(cx, handle, key).as_deref(),
                        Some("step::Note"),
                        "{key}"
                    );
                }
            } else {
                for key in ["c", "d", "e", "f", "g", "a", "b"] {
                    assert_eq!(
                        action_for(cx, handle, key).as_deref(),
                        Some("step::Note"),
                        "{key}"
                    );
                }
                assert_eq!(
                    action_for(cx, handle, "#").as_deref(),
                    Some("step::SetDottedLength"),
                    "조합이 없을 때 #은 점4분"
                );
                // 조합 중에는 숫자가 옥타브, #·+·-는 변화표
                press(cx, handle, "c");
                assert_eq!(path(cx, &view), vec!["Step", want, "Pending"]);
                assert_eq!(
                    action_for(cx, handle, "4").as_deref(),
                    Some("step::SetOctave")
                );
                for key in ["#", "+", "-"] {
                    assert_eq!(
                        action_for(cx, handle, key).as_deref(),
                        Some("step::Accidental"),
                        "{key}"
                    );
                }
                press(cx, handle, "enter");
            }
        }
    }
}

#[gpui_kit::test]
fn vim_preset_uses_same_step_grammar(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "vim", KeyLayout::Solfege, project(&[]));
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "5 c 4");
    press(cx, handle, "d enter space shift-e enter");
    assert_eq!(
        notes(cx, &view),
        vec![(0, 240, 60), (240, 240, 62), (720, 120, 64)]
    );
}

// ---------- 25·26. 배치 뷰·선택 ----------

#[gpui_kit::test]
fn step_in_arrangement_view_writes_at_cursor(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "arrangement", KeyLayout::Solfege, project(&[]));
    event(cx, handle, &view, ModeEvent::ToNormal);
    event(cx, handle, &view, ModeEvent::ToggleView);
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    set_cursor(cx, &view, 2 * BAR);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    press(cx, handle, "c enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    assert_eq!(clips(cx, &view), vec![(2 * BAR, BAR)]);
    assert_eq!(notes(cx, &view), vec![(2 * BAR, 960, 60)]);
}

#[gpui_kit::test]
fn first_step_edit_clears_note_selection(cx: &mut TestAppContext) {
    let (handle, view) = setup(
        cx,
        "selection",
        KeyLayout::Solfege,
        project(&[(0, BAR, &[(0, 960, 72)])]),
    );
    let (track, clip) = cx.read(|cx| {
        let t = &view.read(cx).document().read(cx).project().tracks()[0];
        (t.id(), t.clips().next().unwrap().0)
    });
    view.update(cx, |this, cx| {
        let mut sel = Selection::default();
        sel.set(track, clip, [note(0, 960, 72)].into_iter().collect());
        this.set_selection(sel, cx);
    });
    assert!(!cx.read(|cx| view.read(cx).selection().is_empty()));
    press(cx, handle, "c enter");
    assert!(cx.read(|cx| view.read(cx).selection().is_empty()));
    assert!(!cx.read(|cx| view.read(cx).mode_state().has_selection()));
}

/// 리뷰 1 제안 6: 조합 표시는 친 그대로(내림표 `-`, `n` 뒤 길이 코드).
#[gpui_kit::test]
fn status_composition_shows_keys_as_typed(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "status-typed", KeyLayout::Solfege, project(&[]));
    press(cx, handle, "c -");
    assert_eq!(label(cx, &view), "STEP ♩ oct4 c-_");
    press(cx, handle, "enter n 5 c");
    assert_eq!(label(cx, &view), "STEP 1/16 oct4 n5c_");
    press(cx, handle, "enter");
    assert_eq!(notes(cx, &view), vec![(0, 960, 59), (960, 160, 60)]);
}
