//! S3-T3a 적대적 리뷰 2. 리뷰 1 수정 뒤의 깨뜨리기 시도를 남긴다. 실패하는 시험이 지적의 근거다
//! (`docs/reports/s3-t3a/review-2.md`). 시스템 클립보드는 시험마다 이름 붙은 pasteboard를 쓴다.

use std::collections::BTreeSet;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::clipboard;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::View;
use keyscore_app::pianoroll::{Cursor, Selection};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Clip, ClipId, Note, Pitch, Project, Tick, Track, TrackColor, TrackId, Velocity,
};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

type Triple = (u64, u32, u8);

// ---------- 픽스처(review_s3_t3a.rs와 같다) ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review2-t3a-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        clipboard::use_named_pasteboard(
            cx,
            &format!("ks-review2-t3a-{}-{name}", std::process::id()),
        );
    });
}

fn open(cx: &mut TestAppContext, project: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(project, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    (handle, view)
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

fn note(start: u64, len: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        len,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

fn triple(n: &Note) -> Triple {
    (n.start().get(), n.length(), n.pitch().get())
}

fn clip(start: u64, length: u64, notes: &[Note]) -> Clip {
    let mut clip = Clip::new(Tick::new(start), length).unwrap();
    for n in notes {
        clip.insert_note(n.clone()).unwrap();
    }
    clip
}

fn project_named(name: &str, clips: Vec<Clip>) -> Project {
    let mut project = Project::new();
    let mut track = Track::new(name, Channel::new(2).unwrap(), TrackColor::new(0).unwrap());
    for c in clips {
        track.insert_clip(c).unwrap();
    }
    project.push_track(track).unwrap();
    project
}

fn project_with(clips: Vec<Clip>) -> Project {
    project_named("Lead", clips)
}

const C4: Triple = (0, 960, 60);
const D4: Triple = (960, 960, 62);
const E4: Triple = (1920, 960, 64);
const G4: Triple = (3840, 960, 67);

/// 클립 A [0, 2마디) C4 D4 E4 G4, 클립 B [4마디, 5마디) A4, 클립 C [5마디, 6마디) 빈 클립.
fn fixture() -> Project {
    let a: Vec<Note> = [C4, D4, E4, G4]
        .iter()
        .map(|&(s, l, p)| note(s, l, p))
        .collect();
    project_with(vec![
        clip(0, 2 * BAR, &a),
        clip(4 * BAR, BAR, &[note(0, 960, 69)]),
        clip(5 * BAR, BAR, &[]),
    ])
}

fn set_cursor(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, tick: u64, pitch: u8) {
    cx.update(|cx| {
        view.read(cx).piano_roll().set_cursor(Cursor {
            tick: Tick::new(tick),
            pitch: Pitch::new(pitch).unwrap(),
        });
    });
}

fn clips(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<(u64, u64, Vec<Triple>)> {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        track
            .clips()
            .map(|(_, c)| {
                (
                    c.start().get(),
                    c.length(),
                    c.notes().iter().map(triple).collect(),
                )
            })
            .collect()
    })
}

fn clip_a(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<Triple> {
    clips(cx, view)[0].2.clone()
}

fn ids(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, index: usize) -> (TrackId, ClipId) {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        (track.id(), track.clips().nth(index).unwrap().0)
    })
}

fn select(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, index: usize, notes: &[Triple]) {
    let (track, clip) = ids(cx, view, index);
    let set: BTreeSet<Note> = notes.iter().map(|&(s, l, p)| note(s, l, p)).collect();
    view.update(cx, |this, cx| {
        let mut sel = Selection::default();
        sel.set(track, clip, set);
        this.set_selection(sel, cx);
    });
}

fn selection_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<Triple> {
    cx.read(|cx| {
        let mut v: Vec<_> = view.read(cx).selection().notes().map(triple).collect();
        v.sort_unstable();
        v
    })
}

fn selection_clip(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<ClipId> {
    cx.read(|cx| view.read(cx).selection().clip())
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

fn enter_piano_roll(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
) {
    press(cx, handle, "enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
}

fn setup_with(
    cx: &mut TestAppContext,
    name: &str,
    project: Project,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    let (handle, view) = open(cx, project);
    enter_piano_roll(cx, handle, &view);
    (handle, view)
}

/// 클립 A [0, 1마디) C4(0, 1박)·E4(2880, 1박), 클립 B는 마디 중간(5000)에서 시작한다(SMF 가져오기·줄이기가 만드는
/// 모양). A 끝과 B 시작 사이 [3840, 5000)이 비어 있다.
fn mid_bar_next() -> Project {
    project_with(vec![
        clip(0, BAR, &[note(0, 960, 60), note(2880, 960, 64)]),
        clip(5000, BAR, &[]),
    ])
}

// ---------- 지적 1: 있던 클립을 늘리는 붙여넣기·복제가 마디 중간의 다음 클립 앞 빈 자리에 들어가지 않는다 ----------

/// A 안 3000에 C4(1박, 끝 3960)를 붙인다. 빈 자리 [3840, 5000)에 들어가는데 A를 마디 끝(7680)까지 늘리려다 B와
/// 겹쳐 거부된다. REQ 7.5 「노트는 클립 경계를 넘으면 클립을 늘리고」.
#[gpui_kit::test]
fn paste_near_clip_end_fits_before_mid_bar_next_clip(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "paste-mid-next", mid_bar_next());
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-c");
    set_cursor(cx, &view, 3000, 60);
    press(cx, handle, "cmd-v");
    let a = clip_a(cx, &view);
    assert!(
        a.contains(&(3000, 960, 60)),
        "붙지 않았다: {a:?}, 알림 {:?}",
        last_message(cx, &view)
    );
}

/// 같은 모양에서 E4(2880)에 `cmd-d`: 사본은 3840(끝 4800)이라 빈 자리에 들어가는데 거부된다. REQ 7.5 `Cmd+D`
/// 「바로 뒤에 붙여넣기」.
#[gpui_kit::test]
fn duplicate_near_clip_end_fits_before_mid_bar_next_clip(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "dup-mid-next", mid_bar_next());
    set_cursor(cx, &view, 2880, 64);
    press(cx, handle, "cmd-d");
    let a = clip_a(cx, &view);
    assert!(
        a.contains(&(3840, 960, 64)),
        "복제되지 않았다: {a:?}, 알림 {:?}",
        last_message(cx, &view)
    );
}

/// 대조(통과): 같은 C4를 한 틱 뒤인 A 끝(3840, 틈)에 붙이면 [3840, 5000)의 새 클립이 생긴다. A 안(3000)은 거부되고
/// A 끝은 받아들여져 같은 빈 자리에 대한 판단이 자리 틱에 따라 갈린다.
#[gpui_kit::test]
fn paste_at_clip_end_before_mid_bar_next_clip_makes_new_clip(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "paste-at-end-mid-next", mid_bar_next());
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-c");
    set_cursor(cx, &view, BAR, 60);
    press(cx, handle, "cmd-v");
    let all = clips(cx, &view);
    assert_eq!(all.len(), 3, "{all:?}");
    assert_eq!((all[1].0, all[1].1), (BAR, 5000 - BAR));
}

// ---------- 리뷰 1 지적 2 수정의 앱 경로 ----------

/// 앞 클립 [0, 1.5마디), 다음 클립 [2마디 + 3박, …). 1마디 + 3박에 붙이면 새 클립 [1.5마디, 2마디)이 생기고 선택이
/// 그 노트다. undo는 비우고 redo는 다시 그 노트를 고른다. 이어서 `r`은 새 클립 안(끝 2마디)에 들어간다.
#[gpui_kit::test]
fn gap_paste_between_mid_bar_clips_selects_and_repeats(cx: &mut TestAppContext) {
    let project = project_with(vec![
        clip(0, BAR + 2 * QUARTER, &[note(0, 480, 60)]),
        clip(2 * BAR + 3 * QUARTER, BAR, &[]),
    ]);
    let (handle, view) = setup_with(cx, "gap-both-sides", project);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-c");
    set_cursor(cx, &view, BAR + 3 * QUARTER, 60);
    press(cx, handle, "cmd-v");
    let all = clips(cx, &view);
    assert_eq!(all.len(), 3, "{all:?} {:?}", last_message(cx, &view));
    assert_eq!((all[1].0, all[1].1), (BAR + 2 * QUARTER, 2 * QUARTER));
    assert_eq!(selection_of(cx, &view), vec![(QUARTER, 480, 60)]);
    assert_eq!(selection_clip(cx, &view), Some(ids(cx, &view, 1).1));
    press(cx, handle, "cmd-z");
    assert!(selection_of(cx, &view).is_empty());
    press(cx, handle, "cmd-shift-z");
    assert_eq!(selection_of(cx, &view), vec![(QUARTER, 480, 60)]);
    press(cx, handle, "r");
    let all = clips(cx, &view);
    assert_eq!(
        all[1].2,
        vec![(QUARTER, 480, 60), (QUARTER + 480, 480, 60)],
        "{:?}",
        last_message(cx, &view)
    );
}

/// 자리 틱이 앞 클립의 마디 중간 끝과 같은 틱이면 새 클립은 그 틱에서 시작하고 앞 클립과 맞닿는다.
#[gpui_kit::test]
fn paste_at_mid_bar_clip_end_tick_starts_new_clip_there(cx: &mut TestAppContext) {
    let project = project_with(vec![
        clip(0, BAR + 2 * QUARTER, &[note(0, 960, 60)]),
        clip(4 * BAR, BAR, &[]),
    ]);
    let (handle, view) = setup_with(cx, "gap-at-end-tick", project);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-c");
    set_cursor(cx, &view, BAR + 2 * QUARTER, 60);
    press(cx, handle, "cmd-v");
    let all = clips(cx, &view);
    assert_eq!(all.len(), 3, "{all:?}");
    assert_eq!(
        (all[1].0, all[1].1, all[1].2.clone()),
        (BAR + 2 * QUARTER, 2 * QUARTER, vec![(0, 960, 60)])
    );
}

// ---------- 제안 근거(동작 기록, 통과) ----------

/// vim `p`(커서 + 한 스텝)를 클립 A의 마지막 칸(6720)에서 누르면 자리 틱이 A 끝(7680)이라 A를 늘리지 않고 A 뒤에
/// 맞닿은 새 클립을 만든다. 커서는 A 안이다.
#[gpui_kit::test]
fn vim_p_on_last_step_of_clip_makes_separate_clip(cx: &mut TestAppContext) {
    start(cx, "vim-p-last-step");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-c");
    set_cursor(cx, &view, 2 * BAR - QUARTER, 60);
    press(cx, handle, "p");
    let all = clips(cx, &view);
    assert_eq!(all.len(), 4, "{all:?}");
    assert_eq!(all[0].1, 2 * BAR, "A가 늘지 않았다");
    assert_eq!((all[1].0, all[1].2.clone()), (2 * BAR, vec![(0, 960, 60)]));
    // 사본 선택은 새 클립에 있고 커서(6720)는 A에 있다. 바로 이은 `r`은 거부된다.
    press(cx, handle, "r");
    assert!(
        last_message(cx, &view).contains("선택이 현재 클립에 없다"),
        "{:?}",
        last_message(cx, &view)
    );
}

/// 클립 A에서 C4를 지우고 커서를 클립 B로 옮긴 뒤 `cmd-z`하면 선택은 A의 C4가 된다(계획 6). 그 상태에서 `x`는
/// 「선택이 현재 클립에 없다」로 거부된다.
#[gpui_kit::test]
fn undo_selection_away_from_cursor_blocks_next_delete(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "undo-away", fixture());
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "x");
    set_cursor(cx, &view, 4 * BAR, 69);
    press(cx, handle, "cmd-z");
    assert_eq!(selection_of(cx, &view), vec![C4]);
    assert_eq!(selection_clip(cx, &view), Some(ids(cx, &view, 0).1));
    press(cx, handle, "x");
    assert!(
        last_message(cx, &view).contains("선택이 현재 클립에 없다"),
        "{:?}",
        last_message(cx, &view)
    );
    assert_eq!(clips(cx, &view)[1].2, vec![(0, 960, 69)]);
    assert_eq!(undo_len(cx, &view), 0);
}

/// 잘라내기의 상태줄은 「복사했다」로 끝난다(노트는 지워졌다).
#[gpui_kit::test]
fn cut_status_line_says_copied(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "cut-status", fixture());
    select(cx, &view, 0, &[C4, D4]);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-x");
    assert_eq!(clip_a(cx, &view), vec![E4, G4]);
    assert_eq!(last_message(cx, &view), "노트 2개를 복사했다");
}

/// 리뷰 1 제안 4의 대응(「다른 앱용 텍스트·SMF는 만들지 못했다」)을 일으켜 보려는 시도. 텍스트 왕복이 어려워 보이는
/// 노트 모양을 복사한다. 빠진 형식이 있으면 알림이 그것을 말해야 한다. 지금은 모든 모양이 텍스트를 만든다(못 깨뜨림).
#[gpui_kit::test]
fn copy_awkward_notes_keeps_text_or_reports_missing(cx: &mut TestAppContext) {
    let cases: Vec<(&str, Vec<Note>)> = vec![
        (
            "partial-overlap",
            vec![note(0, 960, 60), note(480, 960, 64)],
        ),
        (
            "same-pitch-overlap",
            vec![note(0, 960, 60), note(480, 960, 60)],
        ),
        ("twins-diff-len", vec![note(0, 960, 60), note(0, 480, 60)]),
        ("lyric-quote", vec![note(0, 960, 60).with_lyric("a\"b")]),
        (
            "lyric-chord",
            vec![
                note(0, 960, 60).with_lyric("x"),
                note(0, 960, 64).with_lyric("y"),
            ],
        ),
        ("tiny", vec![note(0, 1, 60), note(1, 7, 62)]),
    ];
    for (name, notes) in cases {
        start(cx, &format!("copy-awkward-{name}"));
        let (handle, view) = open(cx, project_with(vec![clip(0, BAR, &notes)]));
        enter_piano_roll(cx, handle, &view);
        let (track, clip_id) = ids(cx, &view, 0);
        let set: BTreeSet<Note> = notes.iter().cloned().collect();
        view.update(cx, |this, cx| {
            let mut sel = Selection::default();
            sel.set(track, clip_id, set);
            this.set_selection(sel, cx);
        });
        set_cursor(cx, &view, 0, 60);
        press(cx, handle, "cmd-c");
        let message = last_message(cx, &view);
        let system = cx.read(clipboard::read_system);
        assert!(system.midi.is_some(), "{name}: SMF 없음");
        if system.text.is_none() {
            assert!(message.contains("만들지 못했다"), "{name}: {message}");
        }
    }
}
