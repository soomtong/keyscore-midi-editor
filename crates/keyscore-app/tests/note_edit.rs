//! S3-T3a 삭제·클립보드·반복(REQ 5.8, 5.8.1, 7.5, KM-15, 5.10). 헤드리스 창에서 키를 눌러 문서·선택·클립보드를
//! 본다. 시스템 클립보드는 시험마다 이름 붙은 pasteboard를 써서 사람의 클립보드를 덮지 않는다.

use std::collections::BTreeSet;
use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::clipboard::{self, ClipNotes, MIDI_TYPE, TEXT_TYPE};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::View;
use keyscore_app::pianoroll::{Cursor, Selection};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Clip, ClipId, Note, Pitch, PitchOffset, Project, Tick, Track, TrackColor, TrackId,
    Velocity,
};
use keyscore_midi::{SmfFile, smf_import};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

type Triple = (u64, u32, u8);

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-edit-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        clipboard::use_named_pasteboard(cx, &format!("ks-test-{}-{name}", std::process::id()));
    });
    dir
}

fn open(cx: &mut TestAppContext, project: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(project, cx)).unwrap();
    activate(cx, handle);
    (handle, view)
}

fn activate(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
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

const C4: Triple = (0, 960, 60);
const D4: Triple = (960, 960, 62);
const E4: Triple = (1920, 960, 64);
const G4: Triple = (3840, 960, 67);

/// 클립 A [0, 2마디): C4 D4 E4 G4. 틈 [2마디, 4마디). 클립 B [4마디, 5마디): A4. 클립 C [5마디, 6마디): 비어 있음.
fn fixture() -> Project {
    let mut project = Project::new();
    let mut track = Track::new(
        "Lead",
        Channel::new(2).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    let a: Vec<Note> = [C4, D4, E4, G4]
        .iter()
        .map(|&(s, l, p)| note(s, l, p))
        .collect();
    track.insert_clip(clip(0, 2 * BAR, &a)).unwrap();
    track
        .insert_clip(clip(4 * BAR, BAR, &[note(0, 960, 69)]))
        .unwrap();
    track.insert_clip(clip(5 * BAR, BAR, &[])).unwrap();
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

/// 트랙 0의 클립들: (절대 시작, 길이, 노트 (클립 기준 시작, 길이, 음높이)).
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

fn has_selection(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).mode_state().has_selection())
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

fn internal(cx: &mut TestAppContext) -> Option<Vec<Triple>> {
    cx.read(|cx| clipboard::internal(cx).map(|c| c.notes().iter().map(triple).collect()))
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

fn setup(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    (handle, view)
}

// ---------- 10. 삭제 ----------

#[gpui_kit::test]
fn delete_x_delete_backspace_remove_targets(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "delete");
    // 선택 없이 커서 노트
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "x");
    assert_eq!(clip_a(cx, &view), vec![C4, E4, G4]);
    assert_eq!(undo_len(cx, &view), 1);
    // 선택 삭제(Delete), 뒤에 선택·컨텍스트 비움
    select(cx, &view, 0, &[C4, E4]);
    assert!(has_selection(cx, &view));
    press(cx, handle, "delete");
    assert_eq!(clip_a(cx, &view), vec![G4]);
    assert!(selection_of(cx, &view).is_empty());
    assert!(!has_selection(cx, &view));
    // 대상 없음: 알림, 문서 그대로
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "backspace");
    assert_eq!(clip_a(cx, &view), vec![G4]);
    assert_eq!(undo_len(cx, &view), 2);
    assert!(last_message(cx, &view).contains("노트가 없다"));
    // 선택이 다른 클립이면 거부(5.3)
    select(cx, &view, 0, &[G4]);
    set_cursor(cx, &view, 4 * BAR, 69);
    press(cx, handle, "backspace");
    assert_eq!(clip_a(cx, &view), vec![G4]);
    assert_eq!(clips(cx, &view)[1].2, vec![(0, 960, 69)]);
    assert!(last_message(cx, &view).contains("현재 클립"));
    assert_eq!(selection_of(cx, &view), vec![G4]);
}

// ---------- 11. 잘라내기·복사·붙여넣기 ----------

#[gpui_kit::test]
fn cut_copy_paste_in_piano_roll(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "paste");
    // 복사는 문서를 바꾸지 않는다
    select(cx, &view, 0, &[C4, D4]);
    press(cx, handle, "cmd-c");
    assert_eq!(undo_len(cx, &view), 0);
    assert_eq!(internal(cx), Some(vec![(0, 960, 60), (960, 960, 62)]));
    assert_eq!(
        selection_of(cx, &view),
        vec![C4, D4],
        "복사는 선택을 그대로"
    );
    // 커서 틱에 붙는다(음높이는 그대로), 선택은 붙인 노트
    set_cursor(cx, &view, 1920, 40);
    press(cx, handle, "cmd-v");
    assert_eq!(
        clip_a(cx, &view),
        vec![C4, D4, (1920, 960, 60), E4, (2880, 960, 62), G4]
    );
    assert_eq!(
        selection_of(cx, &view),
        vec![(1920, 960, 60), (2880, 960, 62)]
    );
    // 클립 끝을 넘으면 마디 단위로 늘린다
    set_cursor(cx, &view, 6720, 60);
    press(cx, handle, "cmd-v");
    assert_eq!(clips(cx, &view)[0].1, 3 * BAR);
    // 틈이면 자리 틱이 든 마디부터 새 클립
    set_cursor(cx, &view, 12_000, 60);
    press(cx, handle, "cmd-v");
    let all = clips(cx, &view);
    assert_eq!(all.len(), 4);
    assert_eq!(
        (all[1].0, all[1].1, all[1].2.clone()),
        (
            3 * BAR,
            BAR,
            vec![(12_000 - 3 * BAR, 960, 60), (12_960 - 3 * BAR, 960, 62)]
        )
    );
    assert_eq!(
        selection_of(cx, &view),
        vec![(12_000 - 3 * BAR, 960, 60), (12_960 - 3 * BAR, 960, 62)]
    );
    // 늘린 클립이 다음 클립과 겹치면 거부(클립 B 끝 근처 → 클립 C)
    let before = clips(cx, &view);
    let undo = undo_len(cx, &view);
    set_cursor(cx, &view, 5 * BAR - 480, 60);
    press(cx, handle, "cmd-v");
    assert_eq!(clips(cx, &view), before);
    assert_eq!(undo_len(cx, &view), undo);
    // 잘라내기는 지우고 다시 붙일 수 있다
    select(cx, &view, 0, &[G4]);
    set_cursor(cx, &view, 3840, 67);
    press(cx, handle, "cmd-x");
    assert!(!clip_a(cx, &view).contains(&G4));
    assert!(!has_selection(cx, &view));
    assert_eq!(internal(cx), Some(vec![(0, 960, 67)]));
    press(cx, handle, "cmd-v");
    assert!(clip_a(cx, &view).contains(&G4));
    // shift-x는 cmd-x와 같다(커서 노트)
    press(cx, handle, "escape");
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "shift-x");
    assert!(!clip_a(cx, &view).contains(&C4));
    assert_eq!(internal(cx), Some(vec![(0, 960, 60)]));
}

// ---------- 12. 시스템 클립보드(게이트) ----------

#[gpui_kit::test]
fn copy_puts_smf_and_text_on_system_clipboard(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "system");
    select(cx, &view, 0, &[C4, D4, E4]);
    press(cx, handle, "cmd-c");
    let system = cx.read(clipboard::read_system);
    let midi = system.midi.expect(MIDI_TYPE);
    let file = SmfFile::read(&midi).unwrap();
    let (project, _) = smf_import(&file).unwrap();
    let count: usize = project
        .tracks()
        .iter()
        .flat_map(keyscore_core::Track::clips)
        .map(|(_, c)| c.notes().len())
        .sum();
    assert_eq!(count, 3);
    let text = system.text.expect(TEXT_TYPE);
    assert!(
        keyscore_text::parse(&text, keyscore_text::Mode::Strict).is_ok(),
        "{text}"
    );
    assert!(text.contains("@track Lead ch2"), "{text}");
    // 붙여넣기는 내부 구조를 쓴다(changeCount 그대로)
    assert_eq!(
        cx.read(|cx| clipboard::paste_contents(cx).unwrap()),
        ClipNotes::new(
            [C4, D4, E4]
                .iter()
                .map(|&(s, l, p)| note(s, l, p))
                .collect()
        )
        .unwrap()
    );
}

// ---------- 13. 다른 창에서 붙여넣기(게이트) ----------

#[gpui_kit::test]
fn paste_in_other_window_uses_internal_structure(cx: &mut TestAppContext) {
    start(cx, "windows");
    let mut project = fixture();
    // 텍스트·SMF가 나르지 못하는 가사·피치 오프셋
    let rich = note(480, 240, 72)
        .with_offset(PitchOffset::new(5).unwrap())
        .unwrap()
        .with_lyric("별");
    let first = project.tracks()[0].clips().next().unwrap().0;
    project
        .track_mut(0)
        .unwrap()
        .insert_note(first, rich.clone())
        .unwrap();
    let (a, view_a) = open(cx, project);
    enter_piano_roll(cx, a, &view_a);
    set_cursor(cx, &view_a, 480, 72);
    press(cx, a, "cmd-c");

    let (b, view_b) = open(cx, fixture());
    enter_piano_roll(cx, b, &view_b);
    set_cursor(cx, &view_b, 2 * QUARTER, 30);
    press(cx, b, "cmd-v");
    let pasted: Vec<Note> = cx.read(|cx| {
        let track = &view_b.read(cx).document().read(cx).project().tracks()[0];
        track.clips().next().unwrap().1.notes().to_vec()
    });
    let got = pasted.iter().find(|n| n.pitch().get() == 72).unwrap();
    assert_eq!(got.start().get(), 2 * QUARTER);
    assert_eq!(got.lyric(), Some("별"));
    assert_eq!(got.offset(), rich.offset());
    assert_eq!(got.length(), 240);
}

// ---------- 14. 다른 앱이 올린 SMF·텍스트 ----------

#[gpui_kit::test]
fn paste_reads_external_smf_then_text(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "external");
    // 다른 앱이 올린 SMF만
    let content = ClipNotes::new(vec![note(0, 480, 50), note(480, 480, 52)]).unwrap();
    let source = fixture();
    let smf = content
        .to_smf(&source, &source.tracks()[0], Tick::ZERO)
        .unwrap();
    cx.update(|cx| clipboard::write_system(cx, None, Some(&smf)));
    set_cursor(cx, &view, 2400, 60);
    press(cx, handle, "cmd-v");
    let a = clip_a(cx, &view);
    assert!(
        a.contains(&(2400, 480, 50)) && a.contains(&(2880, 480, 52)),
        "{a:?}"
    );
    // 텍스트만(트랙 머리 없는 MML 조각)
    cx.update(|cx| clipboard::write_system(cx, Some("o5 l8 c d"), None));
    set_cursor(cx, &view, 4800, 60);
    press(cx, handle, "cmd-v");
    let a = clip_a(cx, &view);
    assert!(
        a.contains(&(4800, 480, 72)) && a.contains(&(5280, 480, 74)),
        "{a:?}"
    );
    // 노트가 없는 텍스트: 알림, 문서 그대로
    cx.update(|cx| clipboard::write_system(cx, Some("hello?"), None));
    let undo = undo_len(cx, &view);
    press(cx, handle, "cmd-v");
    assert_eq!(undo_len(cx, &view), undo);
    let message = last_message(cx, &view);
    assert!(
        message.contains("붙여넣을") && message.contains("계이름 텍스트가 아니다"),
        "{message}"
    );
}

// ---------- 15. 복제·반복 ----------

#[gpui_kit::test]
fn duplicate_and_repeat_append_copies_and_coalesce(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "repeat");
    // cmd-d: 폭 1920 뒤에 사본, 선택은 사본
    select(cx, &view, 0, &[C4, D4]);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-d");
    assert_eq!(
        selection_of(cx, &view),
        vec![(1920, 960, 60), (2880, 960, 62)]
    );
    assert!(clip_a(cx, &view).contains(&(1920, 960, 60)));
    assert_eq!(undo_len(cx, &view), 1);
    press(cx, handle, "cmd-z");
    // r 셋: 선택 없이 커서 노트(G4)부터, 누를 때마다 하나 더, 되돌리기 한 번에 원복
    press(cx, handle, "escape");
    set_cursor(cx, &view, 3840, 67);
    let before = clip_a(cx, &view);
    let undo = undo_len(cx, &view);
    press(cx, handle, "r");
    press(cx, handle, "r");
    press(cx, handle, "r");
    let a = clip_a(cx, &view);
    for s in [4800, 5760, 6720] {
        assert!(a.contains(&(s, 960, 67)), "{s}: {a:?}");
    }
    assert_eq!(selection_of(cx, &view), vec![(6720, 960, 67)]);
    assert_eq!(undo_len(cx, &view), undo + 1);
    press(cx, handle, "cmd-z");
    assert_eq!(clip_a(cx, &view), before);
    // 사이에 다른 키가 끼면 두 단위
    press(cx, handle, "escape");
    set_cursor(cx, &view, 3840, 67);
    press(cx, handle, "r");
    press(cx, handle, "l");
    press(cx, handle, "r");
    assert_eq!(undo_len(cx, &view), undo + 2);
}

// ---------- 16. vim ----------

#[gpui_kit::test]
fn vim_p_shift_p_shift_x_shift_d_shift_y(cx: &mut TestAppContext) {
    start(cx, "vim");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    // y y로 커서 노트 복사, p는 커서 + 한 스텝, shift-p는 커서 자리
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "y y");
    // `y y p`가 있어 `y y`는 시퀀스 시간(1 s)이 지나야 실행된다
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(1100));
    cx.run_until_parked();
    assert_eq!(internal(cx), Some(vec![(0, 960, 60)]));
    press(cx, handle, "escape");
    set_cursor(cx, &view, 1920, 60);
    press(cx, handle, "p");
    assert!(clip_a(cx, &view).contains(&(2880, 960, 60)));
    press(cx, handle, "escape");
    press(cx, handle, "shift-p");
    assert!(clip_a(cx, &view).contains(&(1920, 960, 60)));
    // shift-x: `p`의 순서에서 커서 칸 바로 앞 노트 하나. (1920, E4) 앞은 같은 틱의 낮은 음(방금 붙인 C4)이다
    press(cx, handle, "escape");
    set_cursor(cx, &view, 1920, 64);
    press(cx, handle, "shift-x");
    assert!(!clip_a(cx, &view).contains(&(1920, 960, 60)));
    assert!(clip_a(cx, &view).contains(&E4) && clip_a(cx, &view).contains(&D4));
    // shift-y: 커서가 든 마디 복사
    set_cursor(cx, &view, 4000, 60);
    press(cx, handle, "shift-y");
    assert_eq!(internal(cx), Some(vec![(0, 960, 67)]));
    // shift-d: 커서부터 마디 끝까지(음높이 무관)
    set_cursor(cx, &view, 1920, 30);
    press(cx, handle, "shift-d");
    assert_eq!(clip_a(cx, &view), vec![C4, D4, G4]);
    // y y p: 반복
    set_cursor(cx, &view, 3840, 67);
    press(cx, handle, "y y p");
    assert!(clip_a(cx, &view).contains(&(4800, 960, 67)));
}

// ---------- 17. undo·redo 뒤 선택 ----------

#[gpui_kit::test]
fn undo_redo_restore_selection_to_changed_notes(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "undo-select");
    // 삭제를 되돌리면 되살아난 노트가 선택
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "x");
    press(cx, handle, "cmd-z");
    assert_eq!(selection_of(cx, &view), vec![D4]);
    assert!(has_selection(cx, &view));
    // alt-k를 되돌리면 원래 노트
    press(cx, handle, "escape");
    select(cx, &view, 0, &[C4]);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "alt-k");
    assert_eq!(selection_of(cx, &view), vec![(0, 960, 61)]);
    press(cx, handle, "cmd-z");
    assert_eq!(selection_of(cx, &view), vec![C4]);
    // 붙여넣기를 되돌리면 비고, 다시 실행하면 붙인 노트
    press(cx, handle, "cmd-c");
    set_cursor(cx, &view, 2400, 60);
    press(cx, handle, "cmd-v");
    assert_eq!(selection_of(cx, &view), vec![(2400, 960, 60)]);
    press(cx, handle, "cmd-z");
    assert!(selection_of(cx, &view).is_empty());
    assert!(!has_selection(cx, &view));
    press(cx, handle, "cmd-shift-z");
    assert_eq!(selection_of(cx, &view), vec![(2400, 960, 60)]);
}

// ---------- 18. 배치 뷰·입력란 ----------

#[gpui_kit::test]
fn arrangement_view_edit_keys_do_not_change_document(cx: &mut TestAppContext) {
    start(cx, "arrangement");
    let (handle, view) = open(cx, fixture());
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    let before = clips(cx, &view);
    for key in ["x", "cmd-c", "cmd-v", "r", "cmd-d", "cmd-x"] {
        press(cx, handle, key);
        assert_eq!(clips(cx, &view), before, "{key}");
        assert!(last_message(cx, &view).contains("배치 뷰"), "{key}");
    }
    assert_eq!(undo_len(cx, &view), 0);
}

#[gpui_kit::test]
fn text_field_keeps_clipboard_keys(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "field");
    set_cursor(cx, &view, 0, 60);
    // 템포 입력란에 포커스
    press(cx, handle, "t");
    assert!(
        cx.update_window(handle, |_, window, cx| view
            .read(cx)
            .transport_tempo_focused(window, cx))
            .unwrap()
    );
    press(cx, handle, "cmd-c");
    press(cx, handle, "cmd-v");
    assert_eq!(internal(cx), None);
    assert_eq!(undo_len(cx, &view), 0);
    // 입력란으로 넘기는 Action이 이 GPUI 스냅샷에 있다(이름이 바뀌면 입력란의 복사·붙여넣기가 조용히 사라진다)
    for name in ["input::Copy", "input::Cut", "input::Paste"] {
        assert!(
            cx.update(|cx| cx.build_action(name, None).is_ok()),
            "{name}"
        );
    }
}
