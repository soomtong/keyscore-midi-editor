//! S3-T3b 적대적 리뷰 2회차의 깨뜨리기 시험. 도우미는 `tests/review_s3_t3b.rs`에서 복사했다.

use std::collections::BTreeSet;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::View;
use keyscore_app::pianoroll::{Cursor, Selection};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Clip, ClipId, Note, Pitch, Project, Tick, Track, TrackColor, TrackId, Velocity,
};

const BAR: u64 = 3840;
const BAR32: u32 = 3840;

type Triple = (u64, u32, u8);

// ---------- 픽스처(tests/note_ops.rs와 같다) ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-rev2-t3b-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        keyscore_app::clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-rev2-t3b-{name}", std::process::id()),
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

/// 클립 A [0, 2마디): `a`. 클립 B [4마디, 5마디): A4.
fn project_with(a: &[Triple]) -> Project {
    let mut project = Project::new();
    let mut track = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    let mut clip_a = Clip::new(Tick::ZERO, 2 * BAR).unwrap();
    for &(s, l, p) in a {
        clip_a.insert_note(note(s, l, p)).unwrap();
    }
    track.insert_clip(clip_a).unwrap();
    let mut clip_b = Clip::new(Tick::new(4 * BAR), BAR).unwrap();
    clip_b.insert_note(note(0, 960, 69)).unwrap();
    track.insert_clip(clip_b).unwrap();
    project.push_track(track).unwrap();
    project
}

/// 클립 [0, 3마디)에 `notes`를 두고 `split_at`에서 클립을 자른 프로젝트(왼쪽 클립에 숨김 꼬리가 남는다).
fn project_with_tail(notes: &[Triple], split_at: u64) -> Project {
    let mut project = Project::new();
    let mut track = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    let mut clip = Clip::new(Tick::ZERO, 3 * BAR).unwrap();
    for &(s, l, p) in notes {
        clip.insert_note(note(s, l, p)).unwrap();
    }
    let id = track.insert_clip(clip).unwrap();
    track.split_clip(id, Tick::new(split_at)).unwrap();
    project.push_track(track).unwrap();
    project
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

fn clips(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<(u64, u64, Vec<Triple>)> {
    cx.read(|cx| {
        view.read(cx).document().read(cx).project().tracks()[0]
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

fn setup_with(
    cx: &mut TestAppContext,
    name: &str,
    preset: Preset,
    project: Project,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    if preset == Preset::Vim {
        cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    }
    let (handle, view) = open(cx, project);
    press(cx, handle, "enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
    (handle, view)
}

fn pending_count(cx: &mut TestAppContext) -> Option<u32> {
    cx.read(keymap::pending_count)
}

// ---------- 지적 2: `.`의 지연 디스패치 뒤 묶음 끊기 ----------

/// 기본 프리셋 `r`(`edit::Repeat`)도 묶음 키로 적용된다. `r` `.` `r`은 세 단위여야 한다.
#[gpui_kit::test]
fn dot_replayed_repeat_is_not_coalesced_with_next_r(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(
        cx,
        "dot-repeat",
        Preset::Default,
        project_with(&[(0, 240, 60)]),
    );
    select(cx, &view, 0, &[(0, 240, 60)]);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "r");
    let after_r = clip_a(cx, &view).len();
    assert!(
        after_r > 1,
        "r가 적용되지 않았다: {}",
        last_message(cx, &view)
    );
    assert_eq!(undo_len(cx, &view), 1);
    press(cx, handle, ".");
    assert_eq!(undo_len(cx, &view), 2);
    press(cx, handle, "r");
    assert_eq!(undo_len(cx, &view), 3, "`.` 뒤 r가 되풀이 단위에 묶였다");
}

/// `cmd-d` `.` `.`: 되풀이 둘도 따로 단위이고 `cmd-z`는 하나씩 되돌린다.
#[gpui_kit::test]
fn dot_twice_after_cmd_d_are_separate_units(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(
        cx,
        "dot-twice",
        Preset::Default,
        project_with(&[(0, 240, 60)]),
    );
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-d . .");
    assert_eq!(clip_a(cx, &view).len(), 4);
    assert_eq!(undo_len(cx, &view), 3);
    press(cx, handle, "cmd-z");
    assert_eq!(clip_a(cx, &view).len(), 3);
    press(cx, handle, "cmd-z");
    assert_eq!(clip_a(cx, &view).len(), 2);
}

/// 수정이 묶음을 너무 끊지 않는가: `.` 뒤 `cmd-d` `cmd-d`는 둘이 한 단위로 묶여야 한다(EDT-1 연속 같은 키).
#[gpui_kit::test]
fn cmd_d_pair_after_dot_still_coalesces(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(
        cx,
        "dot-then-pair",
        Preset::Default,
        project_with(&[(0, 240, 60)]),
    );
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-d");
    press(cx, handle, ".");
    assert_eq!(undo_len(cx, &view), 2);
    press(cx, handle, "cmd-d");
    press(cx, handle, "cmd-d");
    assert_eq!(clip_a(cx, &view).len(), 5);
    assert_eq!(undo_len(cx, &view), 3, "연속 cmd-d 둘은 한 단위");
    press(cx, handle, "cmd-z");
    assert_eq!(clip_a(cx, &view).len(), 3);
}

// ---------- 지적 3: vim `0`은 카운트 자리 ----------

/// `1 0`은 카운트 10, `0 3`은 3(맨 앞 `0`은 카운트를 시작하지 않는다), 맨 `0`은 문서·커서를 바꾸지 않는다.
/// 이동 처리기가 아직 카운트를 꺼내지 않으므로(`take_count` 사용처 없음) 쌓인 카운트 값으로 본다.
#[gpui_kit::test]
fn vim_zero_extends_count_and_is_inert_alone(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(
        cx,
        "vim-zero-count",
        Preset::Vim,
        project_with(&[(0, 240, 60)]),
    );
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "0");
    assert_eq!(cursor(cx, &view), (0, 60));
    assert_eq!(pending_count(cx), None);
    assert_eq!(clip_a(cx, &view), vec![(0, 240, 60)]);
    press(cx, handle, "1 0");
    assert_eq!(pending_count(cx), Some(10));
    press(cx, handle, "0");
    assert_eq!(pending_count(cx), Some(100));
    press(cx, handle, "escape");
    assert_eq!(pending_count(cx), None);
    press(cx, handle, "0 3");
    assert_eq!(pending_count(cx), Some(3));
    press(cx, handle, "l");
    assert_eq!(pending_count(cx), None, "l 뒤 카운트가 남았다");
    assert_eq!(undo_len(cx, &view), 0);
}

/// 카운트 뒤 `g 0`(길이 기본값)은 여전히 시퀀스로 잡힌다(`0`이 카운트 숫자로 먹히지 않는다).
#[gpui_kit::test]
fn vim_g_zero_after_count_digits_still_resets_length(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(
        cx,
        "vim-g0-count",
        Preset::Vim,
        project_with(&[(0, 240, 60)]),
    );
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "2 g 0");
    assert_eq!(clip_a(cx, &view), vec![(0, 960, 60)]);
    assert_eq!(pending_count(cx), None, "g 0 뒤 카운트가 남았다");
    // 배치 뷰에서도 맨 0은 아무 일도 하지 않는다.
    press(cx, handle, "g a");
    let undo = undo_len(cx, &view);
    let before = clips(cx, &view);
    press(cx, handle, "0");
    assert_eq!(clips(cx, &view), before);
    assert_eq!(undo_len(cx, &view), undo);
}

/// 기본 프리셋의 `0`은 그대로 길이 기본값이다(vim 덮기가 새지 않는다).
#[gpui_kit::test]
fn default_zero_still_resets_length(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(
        cx,
        "default-zero",
        Preset::Default,
        project_with(&[(0, 240, 60)]),
    );
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "0");
    assert_eq!(clip_a(cx, &view), vec![(0, 960, 60)]);
}

// ---------- 지적 4: 넓힌 `selection_after` ----------

/// 완전히 같은 노트 셋을 합치면 하나가 남고 선택에 남는다.
#[gpui_kit::test]
fn join_of_three_identical_keeps_one_selected(cx: &mut TestAppContext) {
    let a: Triple = (0, 480, 60);
    let (handle, view) = setup_with(cx, "join-three", Preset::Default, project_with(&[a, a, a]));
    select(cx, &view, 0, &[a]);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-j");
    assert_eq!(clip_a(cx, &view), vec![a]);
    assert_eq!(selection_of(cx, &view), vec![a]);
}

/// 같은 값 둘을 분할하면 원래 값은 남지 않으므로 선택은 두 조각뿐이다(넓힌 규칙이 원래 값을 남기지 않는다).
#[gpui_kit::test]
fn split_of_identical_pair_selects_only_halves(cx: &mut TestAppContext) {
    let a: Triple = (0, 960, 60);
    let other: Triple = (0, 960, 64);
    let (handle, view) = setup_with(
        cx,
        "split-pair",
        Preset::Default,
        project_with(&[a, a, other]),
    );
    select(cx, &view, 0, &[a]);
    set_cursor(cx, &view, 480, 60);
    press(cx, handle, "/");
    assert_eq!(
        clip_a(cx, &view),
        vec![
            (0, 480, 60),
            (0, 480, 60),
            (0, 960, 64),
            (480, 480, 60),
            (480, 480, 60)
        ]
    );
    assert_eq!(selection_of(cx, &view), vec![(0, 480, 60), (480, 480, 60)]);
}

/// 퀀타이즈로 대상 하나가 다른 대상과 같은 값이 되면 선택은 그 값 하나다. 선택 밖 노트는 들지 않는다.
#[gpui_kit::test]
fn quantize_onto_identical_target_keeps_selection_to_targets(cx: &mut TestAppContext) {
    let on: Triple = (0, 240, 60);
    let off: Triple = (10, 240, 60);
    let outside: Triple = (970, 240, 60);
    let (handle, view) = setup_with(
        cx,
        "quantize-dup",
        Preset::Default,
        project_with(&[on, off, outside]),
    );
    select(cx, &view, 0, &[on, off]);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "q");
    assert_eq!(clip_a(cx, &view), vec![on, on, outside]);
    assert_eq!(selection_of(cx, &view), vec![on]);
}

// ---------- 지적 1: 레가토가 꼬리를 빼는 규칙 ----------

/// 꼬리와 일반 노트를 함께 고르면 일반 노트만 붙고, 다시 `~`는 일반 노트만 되돌린다. 꼬리·클립은 그대로.
#[gpui_kit::test]
fn legato_mixed_tail_and_normal_toggles_normal_only(cx: &mut TestAppContext) {
    let tail: Triple = (960, 2 * BAR32, 60);
    let y: Triple = (1920, 240, 62);
    let z: Triple = (4800, 240, 64);
    let (handle, view) = setup_with(
        cx,
        "legato-mixed",
        Preset::Default,
        project_with_tail(&[tail, y, z], 2 * BAR),
    );
    let before = clips(cx, &view);
    assert_eq!(before[0], (0, 2 * BAR, vec![tail, y, z]));
    select(cx, &view, 0, &[tail, y]);
    set_cursor(cx, &view, 1920, 62);
    press(cx, handle, "~");
    let after = clips(cx, &view);
    assert_eq!(after[0], (0, 2 * BAR, vec![tail, (1920, 2880, 62), z]));
    assert_eq!(after[1], before[1]);
    assert_eq!(selection_of(cx, &view), vec![tail, (1920, 2880, 62)]);
    assert_eq!(undo_len(cx, &view), 1);
    press(cx, handle, "~");
    assert_eq!(
        clips(cx, &view),
        before,
        "되돌리기: {}",
        last_message(cx, &view)
    );
    assert_eq!(undo_len(cx, &view), 2);
}

/// 꼬리만 골랐을 때 알림이 꼬리를 뺐다는 사실을 알리는가(「바뀌는 노트가 없다」만 나오면 사용자는 이유를 모른다).
#[gpui_kit::test]
fn legato_on_tail_only_explains_why(cx: &mut TestAppContext) {
    let tail: Triple = (960, 2 * BAR32, 60);
    let y: Triple = (1920, 240, 62);
    let (handle, view) = setup_with(
        cx,
        "legato-tail-msg",
        Preset::Default,
        project_with_tail(&[tail, y], 2 * BAR),
    );
    select(cx, &view, 0, &[tail]);
    set_cursor(cx, &view, 960, 60);
    press(cx, handle, "~");
    let msg = last_message(cx, &view);
    assert!(msg.contains("꼬리"), "알림: {msg}");
}
