//! S3-T3b 적대적 리뷰 3회차의 깨뜨리기 시험(`docs/reports/s3-t3b/review-3.md`). 도우미는 `tests/review_s3_t3b_r2.rs`에서 복사했다.

use std::collections::BTreeSet;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, Keystroke, TestAppContext};
use keyscore_app::keymap::{self, Preset, canonical_keys, load_layers};
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
    let dir = std::env::temp_dir().join(format!("ks-rev3-t3b-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        keyscore_app::clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-rev3-t3b-{name}", std::process::id()),
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

const C4: Triple = (0, 960, 60);
const E4: Triple = (1920, 960, 64);

// ---------- 834ea4a: review_s3_t3a_r2의 `r` → `y y p` ----------

/// vim `y y p`는 `y y`(복사) + `p`(붙여넣기)로 갈라지지 않고 반복 하나로 간다. 834ea4a가 바꾼 시험은 알림만 보므로,
/// 붙여넣기가 일어나지 않았는지(클립·undo 그대로)도 본다.
#[gpui_kit::test]
fn vim_yyp_after_p_is_rejected_repeat_not_copy_and_paste(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "vim-yyp-after-p", Preset::Vim, project_with(&[C4, E4]));
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-c");
    set_cursor(cx, &view, 2 * BAR - 960, 60);
    press(cx, handle, "p");
    let after_p = clips(cx, &view);
    assert_eq!(after_p.len(), 3, "{after_p:?}");
    let undo = undo_len(cx, &view);
    press(cx, handle, "y y p");
    assert!(
        last_message(cx, &view).contains("선택이 현재 클립에 없다"),
        "{:?}",
        last_message(cx, &view)
    );
    assert_eq!(clips(cx, &view), after_p);
    assert_eq!(undo_len(cx, &view), undo);
}

/// 허용되는 자리에서 vim `y y p`는 선택을 한 번만 뒤에 복제한다(반복). `y y` 뒤 `p`였다면 커서 다음 칸에 붙는다.
#[gpui_kit::test]
fn vim_yyp_duplicates_selection_once(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "vim-yyp-dup", Preset::Vim, project_with(&[C4, E4]));
    select(cx, &view, 0, &[C4]);
    set_cursor(cx, &view, 0, 60);
    let undo = undo_len(cx, &view);
    press(cx, handle, "y y p");
    assert_eq!(clips(cx, &view)[0].2, vec![C4, (960, 960, 60), E4]);
    assert_eq!(undo_len(cx, &view), undo + 1);
    assert_eq!(selection_of(cx, &view), vec![(960, 960, 60)]);
}

/// 834ea4a의 전제: vim `r`은 이제 반복으로 넘어가지 않는다. 처음 시험의 `r`이 지금 무엇을 하는지 기록한다.
#[gpui_kit::test]
fn vim_r_after_p_no_longer_reaches_repeat(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "vim-r-after-p", Preset::Vim, project_with(&[C4, E4]));
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-c");
    set_cursor(cx, &view, 2 * BAR - 960, 60);
    press(cx, handle, "p");
    let after_p = clips(cx, &view);
    press(cx, handle, "r");
    let msg = last_message(cx, &view);
    assert!(!msg.contains("선택이 현재 클립에 없다"), "{msg:?}");
    press(cx, handle, "escape");
    assert_eq!(clips(cx, &view), after_p);
}

// ---------- 834ea4a: review_s2_t2_r2의 `checked > 0` 제거 ----------

const US_SHIFTED_BASES: &[&str] = &[
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "=", "[", "]", "\\", ";", "'", ",", ".",
    "/", "`",
];

/// 단언을 뺀 대신 B25 a′(「`shift-N` 바인딩은 지운다」)가 지켜지는지 직접 본다. 어느 프리셋·레이어에도
/// `shift-<숫자·기호>` 한 타 바인딩이 없어야 한다(시퀀스 안의 타도 본다).
#[gpui_kit::test]
fn no_preset_binds_shift_digit_or_symbol(_cx: &mut TestAppContext) {
    let mut found = Vec::new();
    for preset in [Preset::Default, Preset::Vim] {
        let (layers, errors) = load_layers(preset, None);
        assert!(errors.is_empty(), "{errors:?}");
        for b in layers.iter() {
            let keys = canonical_keys(&b.keys);
            for stroke in keys.split(' ') {
                let Ok(k) = Keystroke::parse(stroke) else {
                    continue;
                };
                if k.modifiers.shift && US_SHIFTED_BASES.contains(&k.key.as_str()) {
                    found.push(format!("{preset:?} {:?} '{}'", b.context, b.keys));
                }
            }
        }
    }
    assert!(found.is_empty(), "{found:#?}");
}

// ---------- 리뷰 2 제안 2: 숨김 꼬리 알림 ----------

/// 꼬리가 없고 바뀌는 노트도 없으면 알림은 예전 그대로다(꼬리 문구가 새지 않는다).
#[gpui_kit::test]
fn legato_nothing_without_tail_keeps_plain_message(cx: &mut TestAppContext) {
    let a: Triple = (0, 960, 60);
    let b: Triple = (960, 960, 62);
    let (handle, view) = setup_with(cx, "legato-plain", Preset::Default, project_with(&[a, b]));
    select(cx, &view, 0, &[a]);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "~");
    let msg = last_message(cx, &view);
    assert!(msg.contains("바뀌는 노트가 없다"), "{msg:?}");
    assert!(!msg.contains("꼬리"), "{msg:?}");
    assert_eq!(undo_len(cx, &view), 0);
}

/// 끝이 클립 끝과 딱 맞는 노트는 꼬리가 아니다(`>`이지 `>=`가 아니다).
#[gpui_kit::test]
fn legato_note_ending_exactly_at_clip_end_is_not_tail(cx: &mut TestAppContext) {
    let last: Triple = (2 * BAR - 960, 960, 60);
    let (handle, view) = setup_with(cx, "legato-edge", Preset::Default, project_with(&[last]));
    select(cx, &view, 0, &[last]);
    set_cursor(cx, &view, 2 * BAR - 960, 60);
    press(cx, handle, "~");
    let msg = last_message(cx, &view);
    assert!(!msg.contains("꼬리"), "{msg:?}");
}

/// 꼬리와 이미 붙은 일반 노트를 함께 고르면 바뀌는 노트가 없고, 알림은 꼬리를 뺐다고 말한다. 아무것도 바꾸지 않는다.
#[gpui_kit::test]
fn legato_tail_plus_already_legato_note_explains_and_changes_nothing(cx: &mut TestAppContext) {
    let y: Triple = (0, 960, 62);
    let tail: Triple = (960, 2 * BAR32, 60);
    let (handle, view) = setup_with(
        cx,
        "legato-tail-plus",
        Preset::Default,
        project_with_tail(&[y, tail], 2 * BAR),
    );
    let before = clips(cx, &view);
    select(cx, &view, 0, &[y, tail]);
    set_cursor(cx, &view, 0, 62);
    press(cx, handle, "~");
    let msg = last_message(cx, &view);
    assert!(msg.contains("바뀌는 노트가 없다"), "{msg:?}");
    assert!(msg.contains("꼬리"), "{msg:?}");
    assert_eq!(clips(cx, &view), before);
    assert_eq!(undo_len(cx, &view), 0);
}
