//! S3-T3b 노트 편집(REQ 5.8, 5.8.1, 5.7, 5.6). 헤드리스 창에서 키를 눌러 문서·선택·커서·상태줄을 본다. 규칙은
//! `docs/reports/s3-t3b/plan.md` 「끝의 정의」.

use std::collections::BTreeSet;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, KeyContext, Keystroke, TestAppContext};
use keyscore_app::actions::args::{KeyLayout, LengthCode};
use keyscore_app::app_state::AppSettings;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::View;
use keyscore_app::pianoroll::{Cursor, Selection};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::transform::Grid;
use keyscore_core::{
    Channel, Clip, ClipId, Note, Pitch, Project, Tick, Track, TrackColor, TrackId, Velocity,
};

const BAR: u64 = 3840;

type Triple = (u64, u32, u8);

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-noteops-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        keyscore_app::clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-noteops-{name}", std::process::id()),
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

const C4: Triple = (0, 960, 60);
const D4: Triple = (960, 960, 62);
const E4: Triple = (1920, 960, 64);
const G4: Triple = (3840, 960, 67);

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

fn fixture() -> Project {
    project_with(&[C4, D4, E4, G4])
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

fn clip_notes(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, index: usize) -> Vec<Triple> {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        track
            .clips()
            .nth(index)
            .unwrap()
            .1
            .notes()
            .iter()
            .map(triple)
            .collect()
    })
}

fn clip_a(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<Triple> {
    clip_notes(cx, view, 0)
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
    preset: Preset,
    project: Project,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    if preset == Preset::Vim {
        cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    }
    let (handle, view) = open(cx, project);
    enter_piano_roll(cx, handle, &view);
    (handle, view)
}

fn setup(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    setup_with(cx, name, Preset::Default, fixture())
}

// ---------- 10. 분할 ----------

#[gpui_kit::test]
fn split_slash_and_vim_s_split_at_cursor(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "split");
    // 커서 노트: (480, C4)에서 `/`. 선택은 생기지 않고 커서는 그대로
    set_cursor(cx, &view, 480, 60);
    press(cx, handle, "/");
    assert_eq!(
        clip_a(cx, &view),
        vec![(0, 480, 60), (480, 480, 60), D4, E4, G4]
    );
    assert!(selection_of(cx, &view).is_empty());
    assert_eq!(cursor(cx, &view), (480, 60));
    assert_eq!(undo_len(cx, &view), 1);
    press(cx, handle, "cmd-z");
    assert_eq!(clip_a(cx, &view), vec![C4, D4, E4, G4]);
    // 선택: 커서 틱 1440을 가로지르는 D4만(E4는 1920부터). 뒤 선택은 두 조각 + 그대로인 E4
    select(cx, &view, 0, &[D4, E4]);
    set_cursor(cx, &view, 1440, 30);
    press(cx, handle, "/");
    assert_eq!(
        clip_a(cx, &view),
        vec![C4, (960, 480, 62), (1440, 480, 62), E4, G4]
    );
    assert_eq!(
        selection_of(cx, &view),
        vec![(960, 480, 62), (1440, 480, 62), E4]
    );
    // 커서가 노트 시작이면 나눌 것이 없다
    press(cx, handle, "escape");
    set_cursor(cx, &view, 0, 60);
    let undo = undo_len(cx, &view);
    press(cx, handle, "/");
    assert_eq!(undo_len(cx, &view), undo);
    assert!(last_message(cx, &view).contains("가로지르는 노트가 없다"));
}

#[gpui_kit::test]
fn vim_s_splits_cursor_note(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "split-vim", Preset::Vim, fixture());
    set_cursor(cx, &view, 2400, 64);
    press(cx, handle, "s");
    assert_eq!(
        clip_a(cx, &view),
        vec![C4, D4, (1920, 480, 64), (2400, 480, 64), G4]
    );
}

// ---------- 11. 합치기 ----------

#[gpui_kit::test]
fn join_cmd_j_and_vim_shift_j(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(
        cx,
        "join",
        Preset::Default,
        project_with(&[(0, 480, 60), (480, 240, 60), D4, G4]),
    );
    select(cx, &view, 0, &[(0, 480, 60), (480, 240, 60), D4]);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-j");
    assert_eq!(clip_a(cx, &view), vec![(0, 720, 60), D4, G4]);
    assert_eq!(selection_of(cx, &view), vec![(0, 720, 60), D4]);
    assert_eq!(undo_len(cx, &view), 1);
    // 선택 없이 커서 노트 하나: 합칠 것이 없다
    press(cx, handle, "escape");
    press(cx, handle, "cmd-j");
    assert_eq!(undo_len(cx, &view), 1);
    assert!(last_message(cx, &view).contains("둘 이상"));
}

#[gpui_kit::test]
fn vim_shift_j_joins(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(
        cx,
        "join-vim",
        Preset::Vim,
        project_with(&[(0, 480, 60), (960, 480, 60)]),
    );
    select(cx, &view, 0, &[(0, 480, 60), (960, 480, 60)]);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "shift-j");
    assert_eq!(clip_a(cx, &view), vec![(0, 1440, 60)]);
}

// ---------- 12·13. 길이 코드 ----------

#[gpui_kit::test]
fn length_codes_set_selection_length(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "length");
    select(cx, &view, 0, &[C4, D4]);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "5");
    assert_eq!(
        clip_a(cx, &view),
        vec![(0, 240, 60), (960, 240, 62), E4, G4]
    );
    assert_eq!(selection_of(cx, &view), vec![(0, 240, 60), (960, 240, 62)]);
    // 점4분음표는 Shift+3이 들어오는 글자 `#`
    press(cx, handle, "#");
    assert_eq!(
        clip_a(cx, &view),
        vec![(0, 1440, 60), (960, 1440, 62), E4, G4]
    );
    press(cx, handle, "0");
    assert_eq!(clip_a(cx, &view), vec![C4, D4, E4, G4]);
    assert_eq!(undo_len(cx, &view), 3);
    // 같은 길이면 바뀌지 않는다
    press(cx, handle, "3");
    assert_eq!(undo_len(cx, &view), 3);
    assert!(last_message(cx, &view).contains("바뀌는 노트가 없다"));
    // 선택 없이 커서 노트(G4): 온음표. 끝이 클립 끝(2마디)과 같아 늘지 않는다
    press(cx, handle, "escape");
    set_cursor(cx, &view, 3840, 67);
    press(cx, handle, "1");
    assert_eq!(clip_a(cx, &view)[3], (3840, 3840, 67));
    assert!(selection_of(cx, &view).is_empty());
    // 2분음표로 줄이면 커서는 노트 안에 남는다
    set_cursor(cx, &view, 3840 + 2880, 67);
    press(cx, handle, "2");
    assert_eq!(clip_a(cx, &view)[3], (3840, 1920, 67));
    let (tick, pitch) = cursor(cx, &view);
    assert!((3840..3840 + 1920).contains(&tick) && pitch == 67, "{tick}");
}

#[gpui_kit::test]
fn shift_digit_keys_are_not_bound(cx: &mut TestAppContext) {
    for file in ["default.json", "vim.json", "standard.json"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/keymaps")
            .join(file);
        let text = std::fs::read_to_string(&path).unwrap();
        for n in 0..=9 {
            assert!(
                !text.contains(&format!("\"shift-{n}\"")),
                "{file}: shift-{n} (B25 a′)"
            );
        }
    }
    let (handle, view) = setup(cx, "shift-digit");
    select(cx, &view, 0, &[C4]);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "%");
    assert_eq!(clip_a(cx, &view)[0], (0, 360, 60), "점16분음표");
}

#[gpui_kit::test]
fn vim_g_digits_set_length(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "length-vim", Preset::Vim, fixture());
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "g 5");
    assert_eq!(clip_a(cx, &view)[0], (0, 240, 60));
    press(cx, handle, "g 0");
    assert_eq!(clip_a(cx, &view)[0], C4);
    // `5` 단독은 카운트다(5.7): 길이를 바꾸지 않는다
    press(cx, handle, "5");
    press(cx, handle, "escape");
    assert_eq!(clip_a(cx, &view)[0], C4);
}

// ---------- 14. 음높이 바꾸기 ----------

#[gpui_kit::test]
fn e_then_note_key_sets_cursor_note_pitch(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "pitch");
    // 건반형(기본): e h = A, 커서 옥타브(C4=60)의 A = 69
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "e");
    assert!(pending(cx, &view));
    assert!(last_message(cx, &view).contains("음 키"));
    press(cx, handle, "h");
    assert!(!pending(cx, &view));
    assert_eq!(clip_a(cx, &view)[0], (0, 960, 69));
    assert_eq!(cursor(cx, &view), (0, 69));
    assert_eq!(undo_len(cx, &view), 1);
    // e k: 한 옥타브 위 C(60 + 12)
    press(cx, handle, "e k");
    assert_eq!(clip_a(cx, &view)[0], (0, 960, 72));
    // Esc는 대기만 푼다
    press(cx, handle, "e");
    press(cx, handle, "escape");
    assert!(!pending(cx, &view));
    assert!(last_message(cx, &view).contains("취소"));
    assert_eq!(clip_a(cx, &view)[0], (0, 960, 72));
    // 음 키가 아닌 키(x)는 대기를 풀고 먹는다: 노트를 지우지 않는다
    let undo = undo_len(cx, &view);
    press(cx, handle, "e x");
    assert!(!pending(cx, &view));
    assert_eq!(undo_len(cx, &view), undo);
    assert_eq!(clip_a(cx, &view)[0], (0, 960, 72));
    // 계이름형: e g = G(72의 옥타브 C5=72 → 79)
    cx.update(|cx| AppSettings::update(cx, |s| s.input.key_layout = KeyLayout::Solfege));
    press(cx, handle, "e g");
    assert_eq!(clip_a(cx, &view)[0], (0, 960, 79));
    // 커서 위에 노트가 없으면 대기에 들어가지 않는다
    set_cursor(cx, &view, 2880, 30);
    press(cx, handle, "e");
    assert!(!pending(cx, &view));
    assert!(last_message(cx, &view).contains("노트가 없다"));
}

#[gpui_kit::test]
fn vim_r_sets_cursor_note_pitch(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "pitch-vim", Preset::Vim, fixture());
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "r");
    assert!(pending(cx, &view));
    press(cx, handle, "a");
    assert_eq!(clip_a(cx, &view)[1], (960, 960, 60));
}

// ---------- 15. 퀀타이즈 ----------

#[gpui_kit::test]
fn quantize_q_uses_settings(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(
        cx,
        "quantize",
        Preset::Default,
        project_with(&[(250, 100, 60), (1000, 100, 62)]),
    );
    select(cx, &view, 0, &[(250, 100, 60), (1000, 100, 62)]);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "q");
    assert_eq!(clip_a(cx, &view), vec![(240, 100, 60), (960, 100, 62)]);
    assert_eq!(
        selection_of(cx, &view),
        vec![(240, 100, 60), (960, 100, 62)]
    );
    press(cx, handle, "q");
    assert!(last_message(cx, &view).contains("바뀌는 노트가 없다"));
    assert_eq!(undo_len(cx, &view), 1);
    press(cx, handle, "cmd-z");
    // 설정을 1/8로: 250은 480에 가깝다(0까지 250, 480까지 230)
    cx.update(|cx| AppSettings::update(cx, |s| s.quantize.grid = Grid::Eighth));
    press(cx, handle, "escape");
    set_cursor(cx, &view, 250, 60);
    press(cx, handle, "q");
    assert_eq!(clip_a(cx, &view)[0], (480, 100, 60));
    assert_eq!(cursor(cx, &view).0, 480, "커서 노트를 따라간다");
}

// ---------- 16. 레가토 ----------

#[gpui_kit::test]
fn legato_tilde_toggles_back(cx: &mut TestAppContext) {
    let original = [(0, 240, 60), (960, 240, 62), (1920, 240, 64)];
    let (handle, view) = setup_with(cx, "legato", Preset::Default, project_with(&original));
    select(cx, &view, 0, &original);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "~");
    let legato = vec![(0, 960, 60), (960, 960, 62), (1920, 240, 64)];
    assert_eq!(clip_a(cx, &view), legato);
    assert_eq!(selection_of(cx, &view), legato);
    // 다시 누르면 되돌린다(새 편집)
    press(cx, handle, "~");
    assert_eq!(clip_a(cx, &view), original.to_vec());
    assert_eq!(selection_of(cx, &view), original.to_vec());
    assert_eq!(undo_len(cx, &view), 2);
    // 또 누르면 다시 붙인다
    press(cx, handle, "~");
    assert_eq!(clip_a(cx, &view), legato);
    // 사이에 다른 편집이 끼면 되돌리지 않는다(이미 붙어 있어 바뀌는 노트가 없다)
    press(cx, handle, "alt-k");
    let undo = undo_len(cx, &view);
    press(cx, handle, "~");
    assert_eq!(undo_len(cx, &view), undo);
    assert!(last_message(cx, &view).contains("바뀌는 노트가 없다"));
}

// ---------- 17. 마지막 편집 반복 ----------

#[gpui_kit::test]
fn dot_repeats_last_edit(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "dot");
    press(cx, handle, ".");
    assert!(last_message(cx, &view).contains("반복할 편집이 없다"));
    // 길이 코드 → 다른 노트에서 반복
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "5");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, ".");
    assert_eq!(clip_a(cx, &view)[1], (960, 240, 62));
    assert_eq!(undo_len(cx, &view), 2);
    // Alt 조작 → 반복(커서가 노트를 따라가 같은 노트에 한 번 더)
    set_cursor(cx, &view, 1920, 64);
    press(cx, handle, "alt-k");
    press(cx, handle, ".");
    assert_eq!(clip_a(cx, &view)[2], (1920, 960, 66));
    assert_eq!(undo_len(cx, &view), 4, "`.`은 묶음을 끊는다");
    // 음높이 바꾸기 → 반복은 대기 없이 같은 음 키(h = A)를 그 노트의 옥타브에
    set_cursor(cx, &view, 3840, 67);
    press(cx, handle, "e h");
    assert_eq!(clip_a(cx, &view)[3], (3840, 960, 69));
    // 복사는 기억하지 않는다
    press(cx, handle, "cmd-c");
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, ".");
    assert!(!pending(cx, &view));
    assert_eq!(clip_a(cx, &view)[0], (0, 240, 69));
    // 삭제 반복
    press(cx, handle, "x");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, ".");
    assert_eq!(clip_a(cx, &view), vec![(1920, 960, 66), (3840, 960, 69)]);
}

// ---------- 18. vim 키맵(A1·A9) ----------

#[gpui_kit::test]
fn vim_shift_k_is_show_info_and_g_digits_are_lengths(cx: &mut TestAppContext) {
    let (_handle, view) = setup_with(cx, "vim-keys", Preset::Vim, fixture());
    let path: Vec<&'static str> = cx.read(|cx| view.read(cx).mode_state().context_path().to_vec());
    let stack: Vec<KeyContext> = path.iter().map(|c| KeyContext::parse(c).unwrap()).collect();
    let winner = |keys: &str, cx: &mut TestAppContext| -> Option<String> {
        let strokes: Vec<Keystroke> = keys
            .split(' ')
            .map(|k| Keystroke::parse(k).unwrap())
            .collect();
        cx.read(|cx| {
            let keymap = cx.key_bindings();
            let keymap = keymap.borrow();
            let (found, pending) = keymap.bindings_for_input(&strokes, &stack);
            assert!(!pending, "{keys}");
            found.first().map(|b| b.action().name().to_string())
        })
    };
    assert_eq!(winner("shift-k", cx).as_deref(), Some("note::ShowInfo"));
    assert_eq!(winner("shift-j", cx).as_deref(), Some("edit::Join"));
    assert_eq!(winner("g 0", cx).as_deref(), Some("note::ResetLength"));
    let g5 = cx.read(|cx| {
        let strokes = [
            Keystroke::parse("g").unwrap(),
            Keystroke::parse("5").unwrap(),
        ];
        let keymap = cx.key_bindings();
        let keymap = keymap.borrow();
        let (found, _) = keymap.bindings_for_input(&strokes, &stack);
        found.first().map(|b| {
            b.action()
                .partial_eq(&keyscore_app::actions::note::SetLength(
                    LengthCode::Sixteenth,
                ))
        })
    });
    assert_eq!(g5, Some(true));
}

// ---------- 19·20. 거부 ----------

#[gpui_kit::test]
fn outside_clip_selection_is_refused_for_new_edits(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "outside");
    for key in ["/", "cmd-j", "5", "q", "~"] {
        select(cx, &view, 1, &[(0, 960, 69)]);
        set_cursor(cx, &view, 480, 60);
        let before = (clip_a(cx, &view), clip_notes(cx, &view, 1));
        press(cx, handle, key);
        assert_eq!(
            (clip_a(cx, &view), clip_notes(cx, &view, 1)),
            before,
            "{key}"
        );
        assert!(
            last_message(cx, &view).contains("선택이 현재 클립에 없다"),
            "{key}: {}",
            last_message(cx, &view)
        );
    }
    assert_eq!(undo_len(cx, &view), 0);
}

#[gpui_kit::test]
fn arrangement_view_note_edit_keys_do_nothing(cx: &mut TestAppContext) {
    start(cx, "arrangement");
    let (handle, view) = open(cx, fixture());
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    for key in ["/", "5", "q", "~", "."] {
        press(cx, handle, key);
        assert_eq!(clip_a(cx, &view), vec![C4, D4, E4, G4], "{key}");
    }
    assert_eq!(undo_len(cx, &view), 0);
}
