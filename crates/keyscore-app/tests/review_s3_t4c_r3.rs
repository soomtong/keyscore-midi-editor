//! S3-T4c 적대적 리뷰 3회차의 확인 시험(`docs/reports/s3-t4c/review-3.md`). 리뷰 2 지적 1의 수정
//! (`next_step_write`가 다음 음 키의 `step_key` → `step_commit` 차례를 따름)이 세 모습을 서로 겹쳐도 지키는지 본다.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::{KeyLayout, LengthCode};
use keyscore_app::actions::{step, track};
use keyscore_app::app_state::AppSettings;
use keyscore_app::mode::{Mode, ModeEvent, View};
use keyscore_app::pianoroll::Cursor;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review-t4c-r3-{}-{name}", std::process::id()));
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

// ---------- 겹친 모습 ----------

/// 셋잇단 접두 `n`과 MML 길이 대기 `l2_`가 함께 있을 때(이전 길이 온음표). 다음 음은 대기를 확정한 2분음표의
/// 셋잇단(1280)이다. 커서를 마디 3 끝 1300틱 앞에 두어 셋잇단 2분음표만 마디 3 안에 들어가게 한다.
#[gpui_kit::test]
fn triplet_with_mml_length_pending_display_matches_next_note(cx: &mut TestAppContext) {
    let at = 3 * BAR - 1300;
    let (handle, view) = open_arrangement_step(
        cx,
        "triplet-mml",
        arrangement(vec![track_with("Lead", 1, &[])]),
        at,
    );
    run(cx, handle, step::SetLength(LengthCode::Whole));
    run(cx, handle, step::Triplet);
    press(cx, handle, "l 2");
    let shown = clip_text(cx, handle);
    press(cx, handle, "c enter");
    let lead = notes_of(cx, &view, 0);
    assert_eq!(lead, vec![(at, 1280, 60)], "n l2 뒤 c는 셋잇단 2분음표");
    let (first, last) = bars_of_clip_at(cx, &view, 0, at).unwrap();
    assert_eq!(
        shown,
        format!("클립 밖: 입력하면 마디 {first}–{last}에 새 클립")
    );
}

/// 다른 트랙(Lead)에 화음이 열린 채 Bass에서 화음 음(`ctrl-c`)을 조합 중. 확정은 Lead 화음을 닫아 커서가 화음
/// 끝으로 간 뒤 Bass에 새 화음을 연다. Lead 화음은 마디 3 셋째 박의 4분음표라 끝은 넷째 박이고, 거기서 Bass의
/// 2분음표는 마디 4로 넘는다(마디 3–4). 화음을 닫지 않는 계획이면 셋째 박에서 써 마디 3–3이라고 말한다.
#[gpui_kit::test]
fn other_track_chord_then_chord_composing_display_matches(cx: &mut TestAppContext) {
    let at = 2 * BAR + 2 * QUARTER;
    let (handle, view) = open_arrangement_step(
        cx,
        "chord-other-chord",
        arrangement(vec![track_with("Lead", 1, &[]), track_with("Bass", 2, &[])]),
        at,
    );
    run(cx, handle, step::SetLength(LengthCode::Quarter));
    press(cx, handle, "ctrl-c enter");
    run(cx, handle, track::Next);
    run(cx, handle, step::SetLength(LengthCode::Half));
    let idle = clip_text(cx, handle);
    press(cx, handle, "ctrl-e");
    let composing = clip_text(cx, handle);
    press(cx, handle, "enter");
    let bass = notes_of(cx, &view, 1);
    assert_eq!(bass.len(), 1, "Bass에 노트 하나: {bass:?}");
    let (first, last) = bars_of_clip_at(cx, &view, 1, bass[0].0).unwrap();
    assert_eq!((first, last), (3, 4), "Lead 화음 끝에서 쓴다: {bass:?}");
    let actual = format!("클립 밖: 입력하면 마디 {first}–{last}에 새 클립");
    assert_eq!(composing, actual, "조합 중 표시, 실제 노트 {bass:?}");
    assert_eq!(idle, actual, "조합 전 표시, 실제 노트 {bass:?}");
}

/// 같은 트랙의 화음을 클립 밖에서 열면(새 클립이 생김) 조합 없는 표시는 그 새 클립 안이라고 해야 하고, 다음
/// 음은 화음 자리에 그 클립으로 들어간다(클립이 하나 더 생기지 않는다).
#[gpui_kit::test]
fn open_chord_in_new_clip_display_points_at_that_clip(cx: &mut TestAppContext) {
    let at = 2 * BAR + 3 * QUARTER;
    let (handle, view) = open_arrangement_step(
        cx,
        "chord-new-clip",
        arrangement(vec![track_with("Lead", 1, &[])]),
        at,
    );
    run(cx, handle, step::SetLength(LengthCode::Quarter));
    press(cx, handle, "ctrl-c enter");
    let shown = clip_text(cx, handle);
    press(cx, handle, "e enter tab");
    let lead = notes_of(cx, &view, 0);
    assert_eq!(lead.iter().filter(|n| n.0 == at).count(), 2, "{lead:?}");
    let clips = cx.read(|cx| {
        view.read(cx).document().read(cx).project().tracks()[0]
            .clips()
            .count()
    });
    assert_eq!(clips, 1);
    assert!(
        shown.ends_with(" 안") && !shown.contains("새 클립"),
        "표시 「{shown}」"
    );
}
