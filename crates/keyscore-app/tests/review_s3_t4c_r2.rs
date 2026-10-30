//! S3-T4c 적대적 리뷰 2회차의 깨뜨리기 시험(`docs/reports/s3-t4c/review-2.md`). 리뷰 1 뒤 배치 뷰 Step 표시가
//! `next_step_write`·`plan_step_write`로 실제 확정과 같은 말을 하는지, 리팩터링이 쓰기를 바꾸지 않았는지 본다.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::{AccidentalKind, Int, KeyLayout, LengthCode};
use keyscore_app::actions::{mode as mode_action, step, track};
use keyscore_app::app_state::AppSettings;
use keyscore_app::mode::{Mode, ModeEvent, View};
use keyscore_app::pianoroll::Cursor;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review-t4c-r2-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
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

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
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

fn set_cursor(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
    tick: u64,
) {
    cx.update(|cx| {
        view.read(cx).piano_roll().set_cursor(Cursor {
            tick: Tick::new(tick),
            pitch: Pitch::new(60).unwrap(),
        });
    });
    cx.update(|cx| view.update(cx, |_, cx| cx.notify()));
    cx.run_until_parked();
    render(cx, handle);
}

fn label(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.label().map(str::to_owned))
    })
    .unwrap()
}

fn clip_text(cx: &mut TestAppContext, handle: AnyWindowHandle) -> String {
    label(cx, handle, "arrangement-step-clip").unwrap_or_default()
}

fn note(start: u64, length: u32) -> Note {
    Note::new(
        Tick::new(start),
        length,
        Pitch::new(72).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

/// 클립 하나: (시작, 길이, 이름, 클립 기준 노트(시작, 길이)).
type ClipSpec<'a> = (u64, u64, Option<&'a str>, &'a [(u64, u32)]);

fn track_with(name: &str, channel: u8, clips: &[ClipSpec<'_>]) -> Track {
    let mut track = Track::new(
        name,
        Channel::new(channel).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    for &(s, l, name, notes) in clips {
        let mut clip = Clip::new(Tick::new(s), l).unwrap();
        for &(ns, nl) in notes {
            clip.insert_note(note(ns, nl)).unwrap();
        }
        if let Some(name) = name {
            clip.set_name(name);
        }
        track.insert_clip(clip).unwrap();
    }
    track
}

fn arrangement(tracks: Vec<Track>) -> Project {
    let mut project = Project::new();
    for t in tracks {
        project.push_track(t).unwrap();
    }
    project
}

/// 배치 뷰 Step 창(계이름 레이아웃).
fn open_arrangement_step(
    cx: &mut TestAppContext,
    name: &str,
    p: Project,
    cursor: u64,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| AppSettings::update(cx, |s| s.input.key_layout = KeyLayout::Solfege));
    cx.run_until_parked();
    render(cx, handle);
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    set_cursor(cx, handle, &view, cursor);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    (handle, view)
}

/// 트랙 `index`의 노트(절대 시작, 길이, 음높이).
fn notes_of(
    cx: &mut TestAppContext,
    view: &Entity<ProjectWindow>,
    index: usize,
) -> Vec<(u64, u32, u8)> {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[index];
        let mut v: Vec<_> = track
            .clips()
            .flat_map(|(_, c)| {
                c.notes()
                    .iter()
                    .map(|n| {
                        (
                            c.start().get() + n.start().get(),
                            n.length(),
                            n.pitch().get(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        v.sort_unstable();
        v
    })
}

/// 트랙 `index`에서 `at`을 품는 클립의 마디 범위(1부터).
fn bars_of_clip_at(
    cx: &mut TestAppContext,
    view: &Entity<ProjectWindow>,
    index: usize,
    at: u64,
) -> Option<(u64, u64)> {
    cx.read(|cx| {
        let project = view.read(cx).document().read(cx).project();
        let meter = project.meter_map();
        let (_, c) = project.tracks()[index]
            .clips()
            .find(|(_, c)| c.start().get() <= at && at < c.end())?;
        let first = meter.position(c.start()).bar() + 1;
        let last = meter.position(Tick::new(c.end() - 1)).bar() + 1;
        Some((first, last))
    })
}

// ---------- 열린 화음 ----------

/// 화음 첫 음(`ctrl-c enter`)을 확정해 화음이 열렸고 조합은 없다. 다음 음은 화음 자리에 밀지 않고 쓴다(`push` = 0).
/// 표시가 「쓸 수 없다」고 하면 실제 다음 음도 쓰이지 않아야 한다. 클립 A 끝에 바로 붙은 B가 있어, 밀기를 가정하면
/// 화음 첫 음이 A 끝을 넘어 B와 겹친다.
#[gpui_kit::test]
fn open_chord_without_composing_display_agrees_with_next_chord_note(cx: &mut TestAppContext) {
    let at = 3 * QUARTER;
    let (handle, view) = open_arrangement_step(
        cx,
        "chord-open",
        arrangement(vec![track_with(
            "Lead",
            1,
            &[(0, BAR, Some("A"), &[]), (BAR, BAR, Some("B"), &[])],
        )]),
        at,
    );
    run(cx, handle, step::SetLength(LengthCode::Quarter));
    press(cx, handle, "ctrl-c enter");
    assert!(
        cx.read(|cx| view.read(cx).step_status().chord_open),
        "화음이 열려 있다"
    );
    let shown = clip_text(cx, handle);
    press(cx, handle, "e enter tab");
    let written = notes_of(cx, &view, 0);
    let both = written.iter().filter(|n| n.0 == at).count() == 2;
    assert!(both, "화음 두 음이 같은 자리에: {written:?}");
    assert!(
        !shown.contains("쓸 수 없다"),
        "표시 「{shown}」인데 다음 화음 음은 쓰였다: {written:?}"
    );
}

/// 다른 트랙에 화음이 열려 있으면 확정은 그 화음을 먼저 닫고(커서가 화음 끝으로 간다) 거기에 쓴다. 표시가 말하는
/// 새 클립 마디가 실제와 같아야 한다.
#[gpui_kit::test]
fn other_track_chord_display_matches_where_the_note_lands(cx: &mut TestAppContext) {
    let at = 2 * BAR + 3 * QUARTER;
    let (handle, view) = open_arrangement_step(
        cx,
        "chord-other-track",
        arrangement(vec![track_with("Lead", 1, &[]), track_with("Bass", 2, &[])]),
        at,
    );
    run(cx, handle, step::SetLength(LengthCode::Quarter));
    press(cx, handle, "ctrl-c enter");
    run(cx, handle, track::Next);
    assert_eq!(cx.read(|cx| view.read(cx).mode_state().mode()), Mode::Step);
    // 조합 없이 본 표시와 음 키를 친(조합 중) 표시
    let idle = clip_text(cx, handle);
    press(cx, handle, "e");
    let composing = clip_text(cx, handle);
    press(cx, handle, "enter");
    let bass = notes_of(cx, &view, 1);
    assert_eq!(bass.len(), 1, "Bass에 노트 하나: {bass:?}");
    let (first, last) = bars_of_clip_at(cx, &view, 1, bass[0].0).unwrap();
    let actual = format!("클립 밖: 입력하면 마디 {first}–{last}에 새 클립");
    assert_eq!(composing, actual, "조합 중 표시, 실제 노트 {bass:?}");
    assert_eq!(idle, actual, "조합 전 표시, 실제 노트 {bass:?}");
}

// ---------- MML 대기·음높이 ----------

/// MML 길이 대기(`l8_`) 중의 표시. 다음 음 키는 대기를 먼저 확정해 8분음표로 쓴다.
#[gpui_kit::test]
fn mml_length_pending_display_matches_next_note(cx: &mut TestAppContext) {
    let at = 2 * BAR + QUARTER;
    let (handle, view) = open_arrangement_step(
        cx,
        "mml-pending",
        arrangement(vec![track_with("Lead", 1, &[])]),
        at,
    );
    run(cx, handle, step::SetLength(LengthCode::Whole));
    press(cx, handle, "l 8");
    let shown = clip_text(cx, handle);
    press(cx, handle, "c enter");
    let lead = notes_of(cx, &view, 0);
    assert_eq!(lead, vec![(at, 480, 60)], "l8 뒤 c는 8분음표");
    let (first, last) = bars_of_clip_at(cx, &view, 0, at).unwrap();
    assert_eq!(
        shown,
        format!("클립 밖: 입력하면 마디 {first}–{last}에 새 클립")
    );
}

/// 조합 음높이가 0–127을 벗어나면(옥타브 9의 `g#`) 표시는 그렇게 말하고 확정은 쓰지 않는다.
#[gpui_kit::test]
fn out_of_range_composition_display_and_commit_agree(cx: &mut TestAppContext) {
    let (handle, view) = open_arrangement_step(
        cx,
        "out-of-range",
        arrangement(vec![track_with("Lead", 1, &[])]),
        0,
    );
    run(cx, handle, step::SetOctave(Int(9)));
    press(cx, handle, "g");
    run(cx, handle, step::Accidental(AccidentalKind::Sharp));
    let shown = clip_text(cx, handle);
    press(cx, handle, "enter");
    assert!(notes_of(cx, &view, 0).is_empty());
    assert_eq!(shown, "클립 밖: 음높이가 0–127을 벗어나 쓸 수 없다");
}

/// 조합 중 `OctaveOnce`(옥타브 9에서 +1은 거부)로도 표시가 확정과 같다: 거부된 뒤 g(127)는 쓰인다.
#[gpui_kit::test]
fn octave_once_rejected_keeps_display_consistent(cx: &mut TestAppContext) {
    let (handle, view) = open_arrangement_step(
        cx,
        "octave-once",
        arrangement(vec![track_with("Lead", 1, &[])]),
        0,
    );
    run(cx, handle, step::SetOctave(Int(9)));
    press(cx, handle, "g");
    run(cx, handle, step::OctaveOnce(Int(1)));
    let shown = clip_text(cx, handle);
    press(cx, handle, "enter");
    assert_eq!(notes_of(cx, &view, 0), vec![(0, 960, 127)]);
    assert_eq!(shown, "클립 밖: 입력하면 마디 1–1에 새 클립");
}

// ---------- 리팩터링 회귀(쓰기) ----------

/// 덮어쓰기(`R`)는 클립 안에서 스텝 범위에서 시작하는 노트를 지우고 밀지 않는다. 화음 다음 음은 지우지 않는다.
/// 표시는 클립 안이라고만 한다.
#[gpui_kit::test]
fn overwrite_chord_in_clip_keeps_chord_notes_and_display(cx: &mut TestAppContext) {
    let (handle, view) = open_arrangement_step(
        cx,
        "overwrite-chord",
        arrangement(vec![track_with(
            "Lead",
            1,
            &[(0, BAR, Some("A"), &[(QUARTER, 240), (2 * QUARTER, 240)])],
        )]),
        QUARTER,
    );
    run(cx, handle, mode_action::StepOverwrite);
    run(cx, handle, step::SetLength(LengthCode::Quarter));
    assert_eq!(clip_text(cx, handle), "클립 「A」 안");
    press(cx, handle, "ctrl-c enter");
    let chord_open = clip_text(cx, handle);
    press(cx, handle, "e enter tab");
    assert_eq!(
        notes_of(cx, &view, 0),
        vec![
            (QUARTER, 960, 60),
            (QUARTER, 960, 64),
            (2 * QUARTER, 240, 72)
        ],
        "덮어쓰기: 960의 기존 노트만 지우고 화음 두 음, 1920의 노트는 그대로"
    );
    assert_eq!(chord_open, "클립 「A」 안");
}

/// 클립 끝 이어 쓰기(다음 클립 없음): 표시 「끝에 이어 쓴다」, 실제로 클립이 늘어 노트가 들어간다.
#[gpui_kit::test]
fn extend_at_clip_end_still_writes(cx: &mut TestAppContext) {
    let (handle, view) = open_arrangement_step(
        cx,
        "extend",
        arrangement(vec![track_with(
            "Lead",
            1,
            &[(0, BAR, Some("A"), &[(0, 240)])],
        )]),
        BAR,
    );
    run(cx, handle, step::SetLength(LengthCode::Quarter));
    assert_eq!(clip_text(cx, handle), "클립 「A」 끝에 이어 쓴다");
    press(cx, handle, "c enter");
    assert_eq!(notes_of(cx, &view, 0), vec![(0, 240, 72), (BAR, 960, 60)]);
    assert_eq!(bars_of_clip_at(cx, &view, 0, BAR), Some((1, 2)));
}
