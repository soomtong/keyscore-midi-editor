//! S3-T3b 적대적 리뷰 1회차의 깨뜨리기 시험. 도우미는 `tests/note_ops.rs`에서 복사했다.

use std::collections::BTreeSet;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::View;
use keyscore_app::pianoroll::edit::{plan_join, plan_split};
use keyscore_app::pianoroll::{Cursor, Selection};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Clip, ClipId, Document, Note, Pitch, Project, Tick, Track, TrackColor, TrackId,
    Velocity,
};

const BAR: u64 = 3840;
const BAR32: u32 = 3840;

type Triple = (u64, u32, u8);

// ---------- 픽스처(tests/note_ops.rs와 같다) ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-rev-t3b-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        keyscore_app::clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-rev-t3b-{name}", std::process::id()),
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

fn pending(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).pitch_pending())
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

// ---------- 시도 1: 숨김 꼬리 노트의 레가토 토글 되돌리기 ----------

/// 계획 7 「되돌리기는 원래 값이라 꼬리도 원래대로다」. 꼬리 노트에 `~`를 두 번 누르면 꼬리 노트가 그대로
/// 돌아와야 하고 클립 길이는 바뀌지 않아야 한다.
#[gpui_kit::test]
fn legato_toggle_back_restores_hidden_tail_without_touching_clips(cx: &mut TestAppContext) {
    // 클립 [0, 3마디)의 X(960, 2마디)와 Y(1920, 240)를 2마디에서 자른다: 왼쪽 [0, 7680)의 X는 끝 8640인 꼬리.
    let tail: Triple = (960, 2 * BAR32, 60);
    let y: Triple = (1920, 240, 62);
    let (handle, view) = setup_with(
        cx,
        "legato-tail",
        Preset::Default,
        project_with_tail(&[tail, y], 2 * BAR),
    );
    let before = clips(cx, &view);
    assert_eq!(before[0], (0, 2 * BAR, vec![tail, y]));
    select(cx, &view, 0, &[tail]);
    set_cursor(cx, &view, 960, 60);
    // 리뷰 1 지적 1 대응(S3-T3b): 꼬리는 공개 편집으로 되살릴 수 없어 레가토가 꼬리를 건드리지 않는다. 처음 기대
    // (붙인 뒤 되돌리기)를 「꼬리·클립이 그대로」로 바꿨다. 두 번 눌러도 같다.
    press(cx, handle, "~");
    assert_eq!(clips(cx, &view), before, "꼬리는 그대로");
    assert!(last_message(cx, &view).contains("바뀌는 노트가 없다"));
    press(cx, handle, "~");
    assert_eq!(
        clips(cx, &view),
        before,
        "꼬리·클립 그대로. 상태줄: {}",
        last_message(cx, &view)
    );
}

// ---------- 시도 2: `.`이 되풀이한 복제와 다음 `cmd-d`의 되돌리기 묶음 ----------

/// 계획 8 「반복된 동작은 undo 한 단위다(`.` 키는 묶음을 끊는다)」, REQ 5.7 「같은 조작을 연속으로 누른 것」만
/// 묶는다. `cmd-d` `.` `cmd-d`는 세 단위여야 한다.
#[gpui_kit::test]
fn dot_replayed_duplicate_is_not_coalesced_with_next_cmd_d(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(
        cx,
        "dot-coalesce",
        Preset::Default,
        project_with(&[(0, 240, 60)]),
    );
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-d");
    assert_eq!(undo_len(cx, &view), 1);
    press(cx, handle, ".");
    assert_eq!(clip_a(cx, &view).len(), 3);
    assert_eq!(undo_len(cx, &view), 2, "`.`은 새 단위");
    press(cx, handle, "cmd-d");
    assert_eq!(clip_a(cx, &view).len(), 4);
    assert_eq!(
        undo_len(cx, &view),
        3,
        "`.`이 되풀이한 복제와 그 뒤 cmd-d가 한 단위로 묶였다"
    );
    // 사용자 눈으로: cmd-z 한 번은 마지막 cmd-d 하나만 되돌려야 한다.
    press(cx, handle, "cmd-z");
    assert_eq!(clip_a(cx, &view).len(), 3);
}

/// 대조군: 같은 경로에서 `Alt` 조작(`LastEdit::Op`, 바로 적용)은 묶이지 않는다.
#[gpui_kit::test]
fn dot_replayed_alt_op_is_not_coalesced_with_next_alt_op(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(
        cx,
        "dot-coalesce-op",
        Preset::Default,
        project_with(&[(0, 240, 60)]),
    );
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "alt-k");
    press(cx, handle, ".");
    press(cx, handle, "alt-k");
    assert_eq!(clip_a(cx, &view), vec![(0, 240, 63)]);
    assert_eq!(undo_len(cx, &view), 3);
}

// ---------- 시도 3: vim 프리셋의 맨 `0` ----------

/// A9: vim에서 숫자는 카운트이고 길이 기본값은 `g 0`이다. 맨 `0`(예: `1 0 l`을 치다가)이 커서 노트 길이를
/// 바꾸면 안 된다.
#[gpui_kit::test]
fn vim_bare_zero_does_not_reset_cursor_note_length(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "vim-zero", Preset::Vim, project_with(&[(0, 240, 60)]));
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "1 0");
    assert_eq!(
        clip_a(cx, &view),
        vec![(0, 240, 60)],
        "vim의 0이 note::ResetLength로 남아 있다"
    );
    assert_eq!(undo_len(cx, &view), 0);
    // 대조군: g 0은 길이 기본값이다.
    press(cx, handle, "escape g 0");
    assert_eq!(clip_a(cx, &view), vec![(0, 960, 60)]);
}

// ---------- 시도 4: 같은 값 노트 둘 합치기 뒤 선택 ----------

/// 계획 1 「편집 뒤 선택은 대상 가운데 그대로 남은 노트 + 새로 나타난 노트」. 완전히 같은 노트 둘을 합치면 하나가
/// 남는다. 남은 노트는 대상 값 그대로이므로 선택에 남아야 한다.
#[gpui_kit::test]
fn join_of_identical_duplicates_keeps_remaining_note_selected(cx: &mut TestAppContext) {
    let a: Triple = (0, 480, 60);
    let (handle, view) = setup_with(cx, "join-dup", Preset::Default, project_with(&[a, a]));
    assert_eq!(clip_a(cx, &view), vec![a, a]);
    select(cx, &view, 0, &[a]);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-j");
    assert_eq!(clip_a(cx, &view), vec![a]);
    assert_eq!(
        selection_of(cx, &view),
        vec![a],
        "남은 노트가 선택에서 빠졌다"
    );
}

// ---------- 시도 5: 숨김 꼬리·같은 값 노트 분할·합치기와 undo(순수) ----------

#[test]
fn split_and_join_on_duplicate_tails_apply_and_undo_exactly() {
    // 같은 꼬리 노트 둘(완전히 같은 값). 분할하면 넷, 다시 합치면 꼬리 하나(둘째 꼬리는 지운다).
    let tail: Triple = (960, 2 * BAR32, 60);
    let project = project_with_tail(&[tail, tail], 2 * BAR);
    let track = project.tracks()[0].id();
    let clip = project.tracks()[0].clips().next().unwrap().0;
    let targets = vec![note(960, 2 * BAR32, 60), note(960, 2 * BAR32, 60)];
    let split = plan_split(&project, track, clip, &targets, Tick::new(1920))
        .unwrap()
        .unwrap();
    let mut doc = Document::new(project.clone(), usize::MAX).unwrap();
    doc.apply(split.edits, None).unwrap();
    let after: Vec<Triple> = doc.project().tracks()[0]
        .clip(clip)
        .unwrap()
        .notes()
        .iter()
        .map(triple)
        .collect();
    let back: Triple = (1920, 2 * BAR32 - 960, 60);
    assert_eq!(after, vec![(960, 960, 60), (960, 960, 60), back, back]);
    let all: Vec<Note> = doc.project().tracks()[0]
        .clip(clip)
        .unwrap()
        .notes()
        .to_vec();
    let join = plan_join(doc.project(), track, clip, &all)
        .unwrap()
        .unwrap();
    doc.apply(join.edits, None).unwrap();
    let joined: Vec<Triple> = doc.project().tracks()[0]
        .clip(clip)
        .unwrap()
        .notes()
        .iter()
        .map(triple)
        .collect();
    assert_eq!(joined, vec![tail]);
    assert_eq!(
        doc.project().tracks()[0].clip(clip).unwrap().length(),
        2 * BAR
    );
    assert!(doc.undo().unwrap());
    assert!(doc.undo().unwrap());
    assert_eq!(doc.project(), &project);
}

// ---------- 시도 6: `e` 대기와 수식키·카운트 ----------

/// 계획 5 「그 밖의 키(수식키 조합)는 대기만 풀고 … 먹는다」. `cmd-z`가 대기 중에 들어오면 되돌리기가 되면 안 된다.
#[gpui_kit::test]
fn pitch_pending_eats_cmd_z(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(
        cx,
        "pitch-cmdz",
        Preset::Default,
        project_with(&[(0, 960, 60)]),
    );
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "5");
    assert_eq!(clip_a(cx, &view), vec![(0, 240, 60)]);
    press(cx, handle, "e");
    assert!(pending(cx, &view));
    press(cx, handle, "cmd-z");
    assert!(!pending(cx, &view));
    assert_eq!(
        clip_a(cx, &view),
        vec![(0, 240, 60)],
        "cmd-z가 먹히지 않았다"
    );
    assert_eq!(undo_len(cx, &view), 1);
}

/// vim `3 r a` 뒤 카운트가 남지 않는다(다음 `l`은 한 스텝).
#[gpui_kit::test]
fn vim_count_before_r_does_not_leak_to_next_motion(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(
        cx,
        "vim-count-r",
        Preset::Vim,
        project_with(&[(0, 960, 62)]),
    );
    set_cursor(cx, &view, 0, 62);
    press(cx, handle, "l");
    let step = cursor(cx, &view).0;
    set_cursor(cx, &view, 0, 62);
    press(cx, handle, "3 r a");
    assert_eq!(clip_a(cx, &view), vec![(0, 960, 60)]);
    assert!(!pending(cx, &view));
    press(cx, handle, "l");
    assert_eq!(cursor(cx, &view).0, step);
}

// ---------- 시도 7: 레가토 기억과 undo ----------

/// 계획 7 「undo·다른 편집으로 판이 바뀌면 기억은 쓰지 않는다」. `~` `cmd-z` `~`는 다시 붙이기다.
#[gpui_kit::test]
fn legato_after_undo_applies_again(cx: &mut TestAppContext) {
    let original = [(0, 240, 60), (960, 240, 62)];
    let (handle, view) = setup_with(cx, "legato-undo", Preset::Default, project_with(&original));
    select(cx, &view, 0, &original);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "~");
    press(cx, handle, "cmd-z");
    assert_eq!(clip_a(cx, &view), original.to_vec());
    select(cx, &view, 0, &original);
    press(cx, handle, "~");
    assert_eq!(clip_a(cx, &view), vec![(0, 960, 60), (960, 240, 62)]);
}
