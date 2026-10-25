//! S3-T3a 적대적 리뷰 1. 깨뜨리기 시도를 남긴다. 실패하는 시험이 지적의 근거다(`docs/reports/s3-t3a/review-1.md`).
//! 시스템 클립보드는 시험마다 이름 붙은 pasteboard를 쓴다(사람의 클립보드를 덮지 않는다).

use std::collections::BTreeSet;
use std::time::Instant;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, ClipboardItem, Entity, TestAppContext};
use keyscore_app::actions::edit;
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

// ---------- 픽스처(note_edit.rs와 같다) ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review-t3a-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        clipboard::use_named_pasteboard(
            cx,
            &format!("ks-review-t3a-{}-{name}", std::process::id()),
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

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
    cx.update_window(handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
}

fn shown(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.value().map(str::to_owned))
    })
    .unwrap()
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

fn project_with(clips: Vec<Clip>) -> Project {
    let mut project = Project::new();
    let mut track = Track::new(
        "Lead",
        Channel::new(2).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    for c in clips {
        track.insert_clip(c).unwrap();
    }
    project.push_track(track).unwrap();
    project
}

const C4: Triple = (0, 960, 60);
const D4: Triple = (960, 960, 62);
const E4: Triple = (1920, 960, 64);
const G4: Triple = (3840, 960, 67);

/// `note_edit.rs`의 픽스처: 클립 A [0, 2마디) C4 D4 E4 G4, 클립 B [4마디, 5마디) A4, 클립 C [5마디, 6마디) 빈 클립.
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

// ---------- 1. 틈 붙여넣기: 앞 클립이 마디 중간에서 끝날 때 ----------

/// 대조(통과): 앞 클립이 마디 중간(1마디 + 2박)에서 끝나도 자리 틱이 다음 마디(2마디 + 1박)면 새 클립 시작(2마디)은
/// 앞 클립과 겹치지 않는다.
#[gpui_kit::test]
fn paste_in_gap_after_mid_bar_clip_end_makes_new_clip(cx: &mut TestAppContext) {
    let project = project_with(vec![
        clip(0, BAR + 2 * QUARTER, &[note(0, 960, 60)]),
        clip(4 * BAR, BAR, &[]),
    ]);
    let (handle, view) = setup_with(cx, "gap-mid-bar", project);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-c");
    // 틈: [1.5마디, 4마디). 자리 틱 2마디 + 1박
    set_cursor(cx, &view, 2 * BAR + QUARTER, 60);
    press(cx, handle, "cmd-v");
    let all = clips(cx, &view);
    assert_eq!(
        all.len(),
        3,
        "틈 붙여넣기가 새 클립을 만들지 않았다: {all:?}, 알림 {:?}",
        last_message(cx, &view)
    );
}

/// 계획 4 「자리 틱이 클립 밖(틈)이면 새 클립」(REQ 5.10). 앞 클립이 마디 중간(1마디 + 2박)에서 끝나고 자리 틱이 같은
/// 마디의 틈(1마디 + 3박)이면 새 클립 시작(자리 틱이 든 마디의 시작 = 1마디)이 앞 클립과 겹쳐 틈인데도 붙지 않는다.
/// 계획은 「다음 클립과 겹치면 거부」만 적었다. SMF 가져오기·클립 줄이기가 만드는 클립은 마디에 맞지 않는 끝이 흔하다.
#[gpui_kit::test]
fn paste_in_gap_in_same_bar_as_prev_clip_end_makes_new_clip(cx: &mut TestAppContext) {
    let project = project_with(vec![
        clip(0, BAR + 2 * QUARTER, &[note(0, 960, 60)]),
        clip(4 * BAR, BAR, &[]),
    ]);
    let (handle, view) = setup_with(cx, "gap-same-bar", project);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-c");
    set_cursor(cx, &view, BAR + 3 * QUARTER, 60);
    press(cx, handle, "cmd-v");
    let all = clips(cx, &view);
    assert_eq!(
        all.len(),
        3,
        "틈 붙여넣기가 새 클립을 만들지 않았다: {all:?}, 알림 {:?}",
        last_message(cx, &view)
    );
}

// ---------- 2. vim `y y p` 연속의 되돌리기 묶음 ----------

/// 계획 5 「같은 키를 연달아 누른 복제·반복은 undo 한 단위」, vim `y y p`는 `edit::Repeat`. 시퀀스의 접두 키(`y`)는
/// 그 자체로는 Action이 없어 키 입력 관찰자가 묶음을 끊는다면 `y y p y y p`는 두 단위가 된다.
#[gpui_kit::test]
fn vim_yyp_repeated_is_one_undo_unit(cx: &mut TestAppContext) {
    start(cx, "vim-yyp");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = open(cx, fixture());
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 3840, 67);
    press(cx, handle, "y y p");
    press(cx, handle, "y y p");
    press(cx, handle, "y y p");
    let a = clip_a(cx, &view);
    for s in [4800, 5760, 6720] {
        assert!(a.contains(&(s, 960, 67)), "{s}: {a:?}");
    }
    assert_eq!(undo_len(cx, &view), 1, "y y p 셋이 한 단위가 아니다");
}

/// 대조: 기본 프리셋 `r r r`은 한 단위다(구현자 시험과 같다). 위 시험이 프리셋 차이임을 보인다.
#[gpui_kit::test]
fn default_r_repeated_is_one_undo_unit(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "r-control", fixture());
    set_cursor(cx, &view, 3840, 67);
    press(cx, handle, "r r r");
    assert_eq!(undo_len(cx, &view), 1);
}

// ---------- 3. 입력란 포커스에서 맨 글자 키 ----------

/// 트랙 이름 입력란에 글자를 친다. `x`(`edit::Delete`)·`r`(`edit::Repeat`)는 이 Task가 처리기를 단 키다. 처리기가
/// 입력란 포커스에서 아무것도 하지 않고 돌아가도 Action을 「처리함」으로 삼키면 글자가 입력란에 들어가지 않는다.
#[gpui_kit::test]
fn edit_letter_keys_in_name_field_are_typed(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "name-field", fixture());
    set_cursor(cx, &view, 0, 60);
    click(cx, handle, "inspector-name");
    press(cx, handle, "cmd-a backspace");
    press(cx, handle, "x r");
    assert_eq!(clip_a(cx, &view), vec![C4, D4, E4, G4]);
    assert_eq!(undo_len(cx, &view), 0);
    assert_eq!(shown(cx, handle, "inspector-name").as_deref(), Some("xr"));
}

/// 대조: 이 Task 전부터 처리기가 있던 맨 글자 키(`l` 커서 오른쪽)도 같은지 본다(이 Task 탓인지 가른다).
#[gpui_kit::test]
fn preexisting_letter_key_in_name_field_is_typed(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "name-field-l", fixture());
    set_cursor(cx, &view, 0, 60);
    click(cx, handle, "inspector-name");
    press(cx, handle, "cmd-a backspace");
    press(cx, handle, "l");
    assert_eq!(shown(cx, handle, "inspector-name").as_deref(), Some("l"));
}

/// 메뉴로 온 붙여넣기(키 입력 없음)는 입력란의 붙여넣기로 간다(계획 「입력란 포커스면 `input::Paste`」). 구현자
/// 시험은 클립보드·문서가 그대로인 것만 보고 입력란에 실제로 들어갔는지는 보지 않는다.
#[gpui_kit::test]
fn menu_paste_in_tempo_field_reaches_input(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "tempo-menu-paste", fixture());
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a backspace");
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string("90".to_owned())));
    run(cx, handle, edit::Paste);
    assert_eq!(undo_len(cx, &view), 0);
    assert_eq!(shown(cx, handle, "transport-tempo").as_deref(), Some("90"));
}

/// 키(`cmd-v`)로 온 붙여넣기도 입력란에 들어간다.
#[gpui_kit::test]
fn key_paste_in_tempo_field_reaches_input(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "tempo-key-paste", fixture());
    click(cx, handle, "transport-tempo");
    press(cx, handle, "cmd-a backspace");
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string("90".to_owned())));
    press(cx, handle, "cmd-v");
    assert_eq!(undo_len(cx, &view), 0);
    assert_eq!(shown(cx, handle, "transport-tempo").as_deref(), Some("90"));
}

// ---------- 4. 같은 값의 노트 ----------

/// 계획 1 「같은 값의 노트가 여럿이면 그 값의 노트 전부가 대상」. 계획 시험 10이 「같은 값 둘 포함」을 적었지만
/// 실제 시험에는 없다. 지우고 되돌리면 둘 다 돌아오고 선택이 그 값이다.
#[gpui_kit::test]
fn delete_same_value_twins_removes_both_and_undo_selects(cx: &mut TestAppContext) {
    let twin = note(960, 960, 62);
    let project = project_with(vec![clip(
        0,
        2 * BAR,
        &[note(0, 960, 60), twin.clone(), twin.clone()],
    )]);
    let (handle, view) = setup_with(cx, "twins", project);
    select(cx, &view, 0, &[D4]);
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "x");
    assert_eq!(clip_a(cx, &view), vec![C4]);
    assert_eq!(undo_len(cx, &view), 1);
    press(cx, handle, "cmd-z");
    assert_eq!(clip_a(cx, &view), vec![C4, D4, D4]);
    assert_eq!(selection_of(cx, &view), vec![D4]);
    // 복사도 둘 다(내부 내용에 같은 노트 둘)
    press(cx, handle, "cmd-c");
    assert_eq!(
        cx.read(|cx| clipboard::internal(cx).map(|c| c.notes().len())),
        Some(2)
    );
}

// ---------- 5. 묶인 반복이 클립을 늘렸을 때 ----------

/// `r` 넷이 클립을 3마디로 늘린다. 되돌리기 한 번에 노트와 클립 길이가 함께 돌아온다.
#[gpui_kit::test]
fn coalesced_repeat_with_extension_undoes_length(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "repeat-extend", fixture());
    let before = clips(cx, &view);
    set_cursor(cx, &view, 3840, 67);
    press(cx, handle, "r r r r");
    assert_eq!(clips(cx, &view)[0].1, 3 * BAR);
    assert_eq!(undo_len(cx, &view), 1);
    press(cx, handle, "cmd-z");
    assert_eq!(clips(cx, &view), before);
}

// ---------- 6. 틈 붙여넣기의 undo·redo 뒤 선택 ----------

#[gpui_kit::test]
fn gap_paste_undo_redo_selects_new_clip_notes(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "gap-redo", fixture());
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-c");
    set_cursor(cx, &view, 2 * BAR + QUARTER, 60);
    press(cx, handle, "cmd-v");
    assert_eq!(clips(cx, &view).len(), 4);
    assert_eq!(selection_of(cx, &view), vec![(QUARTER, 960, 60)]);
    press(cx, handle, "cmd-z");
    assert_eq!(clips(cx, &view).len(), 3);
    assert!(selection_of(cx, &view).is_empty());
    press(cx, handle, "cmd-shift-z");
    assert_eq!(clips(cx, &view).len(), 4);
    assert_eq!(selection_of(cx, &view), vec![(QUARTER, 960, 60)]);
    assert_eq!(selection_clip(cx, &view), Some(ids(cx, &view, 1).1));
    assert!(has_selection(cx, &view));
}

// ---------- 7. 출처 판정: 내부 복사 뒤 다른 앱의 텍스트 ----------

/// 앱 안에서 복사한 뒤 다른 앱이 텍스트를 올리면(`changeCount`가 바뀜) 내부 구조가 아니라 그 텍스트를 붙인다.
#[gpui_kit::test]
fn external_text_after_internal_copy_wins(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "external-after-internal", fixture());
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-c");
    cx.update(|cx| clipboard::write_system(cx, Some("o5 l4 c"), None));
    set_cursor(cx, &view, 2400, 60);
    press(cx, handle, "cmd-v");
    let a = clip_a(cx, &view);
    assert!(a.contains(&(2400, 960, 72)), "{a:?}");
    assert!(!a.contains(&(2400, 960, 60)), "{a:?}");
}

/// 앱을 다시 띄운 것과 같은 상황(내부 없음, 우리가 쓴 SMF·텍스트가 시스템 클립보드에 남음)에서 붙이면 SMF로
/// 읽어 같은 (시작, 길이, 음높이)가 된다.
#[gpui_kit::test]
fn paste_after_restart_reads_own_smf(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "restart", fixture());
    select(cx, &view, 0, &[C4, D4, E4]);
    press(cx, handle, "cmd-c");
    let system = cx.read(clipboard::read_system);
    // 다시 띄움: 내부 상태 없이 같은 pasteboard 내용을 다시 쓴다(changeCount가 바뀐다)
    cx.update(|cx| {
        clipboard::write_system(cx, system.text.as_deref(), system.midi.as_deref());
    });
    press(cx, handle, "escape");
    set_cursor(cx, &view, 5 * BAR, 60);
    press(cx, handle, "cmd-v");
    assert_eq!(
        clips(cx, &view)[2].2,
        vec![(0, 960, 60), (960, 960, 62), (1920, 960, 64)]
    );
}

// ---------- 8. 다른 트랙의 선택 ----------

/// 선택이 다른 트랙(트랙 1)에 있고 현재 트랙이 트랙 0이면 삭제·반복은 거부한다(5.3).
#[gpui_kit::test]
fn edit_with_selection_on_other_track_is_rejected(cx: &mut TestAppContext) {
    let mut project = fixture();
    let mut bass = Track::new(
        "Bass",
        Channel::new(3).unwrap(),
        TrackColor::new(1).unwrap(),
    );
    bass.insert_clip(clip(0, 2 * BAR, &[note(0, 960, 40)]))
        .unwrap();
    project.push_track(bass).unwrap();
    let (handle, view) = setup_with(cx, "other-track", project);
    let (bass_id, bass_clip) = cx.read(|cx| {
        let t = &view.read(cx).document().read(cx).project().tracks()[1];
        (t.id(), t.clips().next().unwrap().0)
    });
    view.update(cx, |this, cx| {
        let mut sel = Selection::default();
        sel.set(bass_id, bass_clip, BTreeSet::from([note(0, 960, 40)]));
        this.set_selection(sel, cx);
    });
    set_cursor(cx, &view, 0, 60);
    for key in ["x", "r", "cmd-d", "cmd-x"] {
        press(cx, handle, key);
        assert_eq!(undo_len(cx, &view), 0, "{key}");
    }
}

// ---------- 9. 큰 트랙의 undo 비용 ----------

/// `select_appeared`는 undo·redo마다 현재 트랙 전체를 복제하고 클립마다 다중집합 차를 구한다. 노트 10만 개 트랙에서
/// 노트 하나를 지우고 되돌리는 시간을 잰다(제안의 근거. 단언은 느슨하다).
#[gpui_kit::test]
fn undo_on_large_track_cost(cx: &mut TestAppContext) {
    let notes: Vec<Note> = (0..100_000u64)
        .map(|i| note(i * 120, 120, 40 + u8::try_from(i % 40).unwrap()))
        .collect();
    let length = (100_000 * 120u64).div_ceil(BAR) * BAR;
    let mut c = Clip::new(Tick::ZERO, length).unwrap();
    for n in &notes {
        c.insert_note(n.clone()).unwrap();
    }
    let (handle, view) = setup_with(cx, "large-undo", project_with(vec![c]));
    set_cursor(cx, &view, 0, 40);
    press(cx, handle, "x");
    let t = Instant::now();
    press(cx, handle, "cmd-z");
    let undo = t.elapsed();
    let t = Instant::now();
    press(cx, handle, "cmd-shift-z");
    let redo = t.elapsed();
    eprintln!("노트 10만 트랙: undo {undo:?}, redo {redo:?}");
    assert_eq!(selection_of(cx, &view), Vec::<Triple>::new());
    press(cx, handle, "cmd-z");
    assert_eq!(selection_of(cx, &view), vec![(0, 120, 40)]);
}

// ---------- 10. 배치 뷰에서 undo ----------

/// 뷰를 바꾸면 FSM이 노트 선택을 비운다(S3-T2, 배치 뷰의 선택은 S4의 클립 선택). 배치 뷰에서 노트 삭제를 되돌리면
/// `select_appeared`가 화면에 없는 피아노롤 노트 선택을 만들어 배치 뷰에 `Selection` 컨텍스트가 켜진다.
#[gpui_kit::test]
fn undo_in_arrangement_view_leaves_no_note_selection(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "arrangement-undo", fixture());
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "x");
    press(cx, handle, "v");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    press(cx, handle, "cmd-z");
    assert_eq!(clip_a(cx, &view), vec![C4, D4, E4, G4]);
    assert!(
        !has_selection(cx, &view),
        "배치 뷰에 노트 선택: {:?}",
        selection_of(cx, &view)
    );
}

// ---------- 11. 큰 외부 텍스트 붙여넣기 비용 ----------

/// 다른 앱에서 긴 글(약 1 MB)을 복사한 뒤 피아노롤에서 `cmd-v`를 누르면 메인 스레드에서 계이름 텍스트로 두 번
/// 읽는다(머리 없음 → `@track` 블록으로 다시). 시간을 잰다(제안의 근거, 단언은 느슨하다).
#[gpui_kit::test]
fn paste_large_plain_text_cost(cx: &mut TestAppContext) {
    let (handle, view) = setup_with(cx, "large-text", fixture());
    let text = "The quick brown fox jumps over the lazy dog. ".repeat(23_000);
    cx.update(|cx| clipboard::write_system(cx, Some(&text), None));
    set_cursor(cx, &view, 0, 60);
    let t = Instant::now();
    press(cx, handle, "cmd-v");
    let took = t.elapsed();
    eprintln!(
        "텍스트 {} 바이트 붙여넣기: {took:?}, 알림 {:?}",
        text.len(),
        last_message(cx, &view)
    );
    assert_eq!(undo_len(cx, &view), 0);
}

// ---------- 12. 큰 선택의 복사·붙여넣기·삭제·복제 비용 ----------

/// 노트 2만 개 클립에서 `cmd-a` → `cmd-c` → `cmd-v`(같은 자리, 노트가 사이사이에 끼어든다) → `cmd-z` → `cmd-a x` →
/// `cmd-z` → `cmd-d`의 시간을 잰다. 노트마다 `Edit::InsertNote`/`RemoveNote`라 정렬 벡터에 끼우기·찾기가 노트 수에 비례한다.
#[gpui_kit::test]
fn large_selection_edit_cost(cx: &mut TestAppContext) {
    const N: u64 = 20_000;
    let length = (N * 120).div_ceil(BAR) * BAR;
    let mut c = Clip::new(Tick::ZERO, length).unwrap();
    for i in 0..N {
        c.insert_note(note(i * 120, 120, 40 + u8::try_from(i % 40).unwrap()))
            .unwrap();
    }
    let (handle, view) = setup_with(cx, "large-edit", project_with(vec![c]));
    set_cursor(cx, &view, 0, 40);
    let timed = |cx: &mut TestAppContext, keys: &str| {
        let t = Instant::now();
        press(cx, handle, keys);
        eprintln!("{keys}: {:?}", t.elapsed());
    };
    timed(cx, "cmd-a");
    timed(cx, "cmd-c");
    timed(cx, "cmd-v");
    assert_eq!(clips(cx, &view)[0].2.len(), 2 * usize::try_from(N).unwrap());
    timed(cx, "cmd-z");
    timed(cx, "cmd-a");
    timed(cx, "x");
    assert!(clips(cx, &view)[0].2.is_empty());
    timed(cx, "cmd-z");
    timed(cx, "escape");
    timed(cx, "cmd-a");
    timed(cx, "cmd-d");
    assert_eq!(clips(cx, &view)[0].2.len(), 2 * usize::try_from(N).unwrap());
}

// ---------- 13. 복제 간격: 「선택의 길이만큼 뒤에」 ----------

/// REQ 7.5 `Cmd+D` 「복제(바로 뒤에 붙여넣기) … 선택의 길이만큼 뒤에」, 5.8 `r` 「선택을 바로 뒤에 반복 복제」.
/// 16분음표(240틱) 하나를 복제하면 사본은 240에 온다. 구현은 한 스텝(960)의 배수로 올려 960에 둔다(720틱 틈).
#[gpui_kit::test]
fn duplicate_sixteenth_lands_right_after_selection(cx: &mut TestAppContext) {
    let project = project_with(vec![clip(0, BAR, &[note(0, 240, 60)])]);
    let (handle, view) = setup_with(cx, "dup-sixteenth", project);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-d");
    assert_eq!(clip_a(cx, &view), vec![(0, 240, 60), (240, 240, 60)]);
}

/// 같은 규칙의 `r r r`: 16분음표 넷이 이어져야 한다(0, 240, 480, 720).
#[gpui_kit::test]
fn repeat_sixteenth_makes_contiguous_run(cx: &mut TestAppContext) {
    let project = project_with(vec![clip(0, BAR, &[note(0, 240, 60)])]);
    let (handle, view) = setup_with(cx, "repeat-sixteenth", project);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "r r r");
    assert_eq!(
        clip_a(cx, &view),
        vec![(0, 240, 60), (240, 240, 60), (480, 240, 60), (720, 240, 60)]
    );
}
