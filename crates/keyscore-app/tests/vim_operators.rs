//! S3-T3c vim 오퍼레이터·텍스트 객체·비주얼 선택·카운트(REQ 5.8.1, 5.7, KM-5). 헤드리스 창에서 키를 눌러
//! 문서·클립보드·선택·커서·모드·상태줄을 본다. 규칙은 `docs/reports/s3-t3c/plan.md` 「끝의 정의」, 결정은
//! `docs/decisions/2026-10-26-s3-t3c-vim-operators.md`.

use std::collections::BTreeSet;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::clipboard;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, View};
use keyscore_app::pianoroll::vim::{Operator, VisualKind};
use keyscore_app::pianoroll::{Cursor, Selection};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};

const BAR: u64 = 3840;

type Triple = (u64, u32, u8);

const C4: Triple = (0, 960, 60);
const D4: Triple = (960, 960, 62);
const E4: Triple = (1920, 960, 64);
const G4: Triple = (1920, 960, 67);
const D4B: Triple = (2880, 960, 62);
const A4: Triple = (3840, 960, 69);
const ALL: [Triple; 6] = [C4, D4, E4, G4, D4B, A4];

// ---------- 픽스처 ----------

fn start(cx: &mut TestAppContext, name: &str, preset: Preset) {
    let dir = std::env::temp_dir().join(format!("ks-vimop-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        clipboard::use_named_pasteboard(
            cx,
            &format!("ks-test-{}-vimop-{name}", std::process::id()),
        );
        if preset == Preset::Vim {
            keymap::set_preset(cx, Preset::Vim);
        }
    });
}

fn note(t: Triple) -> Note {
    Note::new(
        Tick::new(t.0),
        t.1,
        Pitch::new(t.2).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

fn triple(n: &Note) -> Triple {
    (n.start().get(), n.length(), n.pitch().get())
}

/// 클립 A [0, 2마디): C4 D4 (E4+G4) D4B | A4. 클립 B [4마디, 5마디): A4.
fn fixture() -> Project {
    let mut project = Project::new();
    let mut track = Track::new(
        "Lead",
        Channel::new(1).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    let mut a = Clip::new(Tick::ZERO, 2 * BAR).unwrap();
    for n in ALL {
        a.insert_note(note(n)).unwrap();
    }
    track.insert_clip(a).unwrap();
    let mut b = Clip::new(Tick::new(4 * BAR), BAR).unwrap();
    b.insert_note(note((0, 960, 69))).unwrap();
    track.insert_clip(b).unwrap();
    project.push_track(track).unwrap();
    project
}

fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

/// 창을 열고 피아노롤로 들어간다(`enter` = `clip::Open`).
fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(fixture(), cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, handle);
    press(cx, handle, "enter");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::PianoRoll
    );
    (handle, view)
}

fn setup(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name, Preset::Vim);
    open(cx)
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, keys: &str) {
    cx.simulate_keystrokes(handle, keys);
    cx.run_until_parked();
    render(cx, handle);
}

/// 시퀀스 대기 시간(1 s)을 넘긴다.
fn wait(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.executor().advance_clock(Duration::from_millis(1100));
    cx.run_until_parked();
    render(cx, handle);
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

fn clip_a(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<Triple> {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        track
            .clips()
            .next()
            .unwrap()
            .1
            .notes()
            .iter()
            .map(triple)
            .collect()
    })
}

fn clips_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<(u64, u64)> {
    cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        track
            .clips()
            .map(|(_, c)| (c.start().get(), c.length()))
            .collect()
    })
}

fn selection_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<Triple> {
    cx.read(|cx| {
        let mut v: Vec<_> = view.read(cx).selection().notes().map(triple).collect();
        v.sort_unstable();
        v
    })
}

fn select(cx: &mut TestAppContext, view: &Entity<ProjectWindow>, notes: &[Triple]) {
    let (track, clip) = cx.read(|cx| {
        let track = &view.read(cx).document().read(cx).project().tracks()[0];
        (track.id(), track.clips().next().unwrap().0)
    });
    let set: BTreeSet<Note> = notes.iter().map(|&t| note(t)).collect();
    view.update(cx, |this, cx| {
        let mut sel = Selection::default();
        sel.set(track, clip, set);
        this.set_selection(sel, cx);
    });
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

fn pending(
    cx: &mut TestAppContext,
    view: &Entity<ProjectWindow>,
) -> Option<(Operator, Option<u32>)> {
    cx.read(|cx| view.read(cx).pending_operator())
}

fn visual(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<VisualKind> {
    cx.read(|cx| view.read(cx).piano_roll().visual())
}

fn status_mode(cx: &mut TestAppContext, handle: AnyWindowHandle) -> String {
    cx.update_window(handle, |_, window, _| {
        window
            .find("status-mode")
            .label()
            .unwrap_or_default()
            .to_owned()
    })
    .unwrap()
}

/// 내부 클립보드의 노트(가장 이른 노트 기준).
fn internal(cx: &mut TestAppContext) -> Option<Vec<Triple>> {
    cx.read(|cx| {
        clipboard::internal(cx).map(|c| {
            let mut v: Vec<Triple> = c.notes().iter().map(triple).collect();
            v.sort_unstable();
            v
        })
    })
}

/// 원래 노트를 가장 이른 시작 기준으로.
fn relative(notes: &[Triple]) -> Vec<Triple> {
    let first = notes.iter().map(|n| n.0).min().unwrap_or(0);
    let mut v: Vec<Triple> = notes.iter().map(|&(s, l, p)| (s - first, l, p)).collect();
    v.sort_unstable();
    v
}

fn without(gone: &[Triple]) -> Vec<Triple> {
    ALL.iter().copied().filter(|n| !gone.contains(n)).collect()
}

// ---------- 1. 게이트: 오퍼레이터 × 대상 ----------

/// 대상 한 줄: 오퍼레이터 뒤에 칠 키(`None`이면 같은 오퍼레이터를 다시), 시작 칸, 대상 노트, 모션이면
/// 범위의 앞쪽 칸(객체·겹친 오퍼레이터면 `None`).
struct Row {
    keys: Option<&'static str>,
    from: (u64, u8),
    notes: &'static [Triple],
    front: Option<(u64, u8)>,
}

const ROWS: &[Row] = &[
    Row {
        keys: Some("w"),
        from: (960, 62),
        notes: &[D4],
        front: Some((960, 62)),
    },
    Row {
        keys: Some("b"),
        from: (960, 62),
        notes: &[C4],
        front: Some((0, 60)),
    },
    Row {
        keys: Some("l"),
        from: (960, 62),
        notes: &[D4],
        front: Some((960, 62)),
    },
    // 같은 틱의 낮은 음은 읽기 순서에서 앞이다(계획 4절): C4(0, 60)는 [(0, 62), (960, 62)) 밖
    Row {
        keys: Some("h"),
        from: (960, 62),
        notes: &[],
        front: Some((0, 62)),
    },
    Row {
        keys: Some("e"),
        from: (960, 62),
        notes: &[D4, E4],
        front: Some((960, 62)),
    },
    Row {
        keys: Some("shift-e"),
        from: (960, 62),
        notes: &[D4, E4, G4, D4B],
        front: Some((960, 62)),
    },
    Row {
        keys: Some("f"),
        from: (960, 62),
        notes: &[D4, E4, G4, D4B],
        front: Some((960, 62)),
    },
    Row {
        keys: Some("shift-f"),
        from: (2880, 62),
        notes: &[D4, E4, G4, D4B],
        front: Some((960, 62)),
    },
    Row {
        keys: Some("shift-w"),
        from: (960, 62),
        notes: &[D4],
        front: Some((960, 62)),
    },
    Row {
        keys: Some("shift-b"),
        from: (2880, 62),
        notes: &[E4, G4],
        front: Some((1920, 62)),
    },
    Row {
        keys: Some("g g"),
        from: (960, 62),
        notes: &[C4, D4],
        front: Some((0, 62)),
    },
    Row {
        keys: Some("shift-g"),
        from: (960, 62),
        notes: &[D4, E4, G4, D4B, A4],
        front: Some((960, 62)),
    },
    Row {
        keys: Some("}"),
        from: (960, 62),
        notes: &[D4, E4, G4, D4B, A4],
        front: Some((960, 62)),
    },
    Row {
        keys: Some("i n"),
        from: (960, 62),
        notes: &[D4],
        front: None,
    },
    Row {
        keys: Some("i b"),
        from: (960, 62),
        notes: &[C4, D4, E4, G4, D4B],
        front: None,
    },
    Row {
        keys: Some("a c"),
        from: (960, 62),
        notes: &ALL,
        front: None,
    },
    Row {
        keys: None,
        from: (960, 62),
        notes: &[D4],
        front: None,
    },
];

#[gpui_kit::test]
fn operator_times_target_table(cx: &mut TestAppContext) {
    start(cx, "table", Preset::Vim);
    for (op, key) in [
        ("d", Operator::Delete),
        ("y", Operator::Yank),
        ("c", Operator::Change),
    ]
    .map(|(k, o)| (o, k))
    {
        for row in ROWS {
            let (handle, view) = open(cx);
            set_cursor(cx, &view, row.from.0, row.from.1);
            let label = format!("{key} {}", row.keys.unwrap_or(key));
            let undo = undo_len(cx, &view);
            // 클립보드를 표시 노트(A4)로 채워 두어 복사가 일어났는지 구별한다
            set_cursor(cx, &view, 3840, 69);
            press(cx, handle, "cmd-c");
            set_cursor(cx, &view, row.from.0, row.from.1);
            press(cx, handle, &label);
            if op == Operator::Yank && row.keys.is_none() {
                // 빠르게 친 `y y`는 `y y p`의 접두라 대기 시간 뒤 `edit::Copy`다(키맵 그대로)
                wait(cx, handle);
            }
            let want_rel = relative(row.notes);
            match op {
                Operator::Delete => {
                    assert_eq!(clip_a(cx, &view), without(row.notes), "{label}");
                    assert_eq!(
                        undo_len(cx, &view),
                        undo + usize::from(!row.notes.is_empty()),
                        "{label}"
                    );
                    assert_eq!(mode(cx, &view), Mode::Normal, "{label}");
                }
                Operator::Yank => {
                    assert_eq!(clip_a(cx, &view), ALL.to_vec(), "{label}");
                    assert_eq!(undo_len(cx, &view), undo, "{label}");
                    if row.notes.is_empty() {
                        assert_eq!(internal(cx), Some(relative(&[A4])), "{label}");
                    } else {
                        assert_eq!(internal(cx), Some(want_rel.clone()), "{label}");
                    }
                    assert_eq!(mode(cx, &view), Mode::Normal, "{label}");
                }
                Operator::Change => {
                    assert_eq!(clip_a(cx, &view), without(row.notes), "{label}");
                    assert_eq!(mode(cx, &view), Mode::Step, "{label}");
                }
            }
            assert_eq!(pending(cx, &view), None, "{label} 뒤에 대기가 남았다");
            // 커서(계획 3절)
            let want_cursor = match (op, row.front) {
                // 노트가 없으면 d·y는 아무것도 바꾸지 않는다
                (Operator::Delete | Operator::Yank, _) if row.notes.is_empty() => row.from,
                (Operator::Delete | Operator::Yank, Some(front)) => front,
                (Operator::Delete | Operator::Yank, None) => row.from,
                (Operator::Change, Some(front)) => (front.0, row.from.1),
                (Operator::Change, None) => (
                    row.notes.iter().map(|n| n.0).min().unwrap_or(row.from.0),
                    row.from.1,
                ),
            };
            assert_eq!(cursor(cx, &view), want_cursor, "{label}");
            if row.notes.is_empty() && op != Operator::Change {
                assert!(last_message(cx, &view).contains("노트가 없다"), "{label}");
            }
        }
    }
}

// ---------- 5·6. 카운트 ----------

#[gpui_kit::test]
fn count_moves_cursor(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "count-move");
    set_cursor(cx, &view, 0, 62);
    press(cx, handle, "3 l");
    assert_eq!(cursor(cx, &view), (2880, 62));
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "2 w");
    assert_eq!(cursor(cx, &view), (1920, 64));
    // 더 갈 수 없으면 거기서 멈춘다(마지막 노트는 클립 B의 A4)
    press(cx, handle, "9 9 w");
    assert_eq!(cursor(cx, &view), (4 * BAR, 69));
    // g g는 카운트를 받되 쓰지 않는다
    set_cursor(cx, &view, 2880, 62);
    press(cx, handle, "3 g g");
    assert_eq!(cursor(cx, &view), (0, 62));
    assert_eq!(cx.read(keymap::pending_count), None);
}

#[gpui_kit::test]
fn operator_count_multiplies(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "count-op");
    for (keys, gone) in [
        ("2 d w", &[C4, D4][..]),
        ("d 2 w", &[C4, D4][..]),
        ("2 d 2 w", &[C4, D4, E4, G4][..]),
    ] {
        let (handle2, view2) = open(cx);
        set_cursor(cx, &view2, 0, 60);
        press(cx, handle2, keys);
        assert_eq!(clip_a(cx, &view2), without(gone), "{keys}");
        assert_eq!(undo_len(cx, &view2), 1, "{keys}");
    }
    // 오퍼레이터 앞 카운트는 상태줄에 보인다
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "3 d");
    assert_eq!(pending(cx, &view), Some((Operator::Delete, Some(3))));
    assert_eq!(status_mode(cx, handle), "NORMAL 3d");
}

// ---------- 7·8·9. 대기 취소 ----------

#[gpui_kit::test]
fn pitch_motion_cancels_operator(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "pitch-cancel");
    for keys in ["d j", "d k", "d ]", "y [", "c j"] {
        set_cursor(cx, &view, 960, 62);
        press(cx, handle, keys);
        assert_eq!(clip_a(cx, &view), ALL.to_vec(), "{keys}");
        assert_eq!(cursor(cx, &view), (960, 62), "{keys}");
        assert_eq!(pending(cx, &view), None, "{keys}");
        assert_eq!(mode(cx, &view), Mode::Normal, "{keys}");
        assert!(last_message(cx, &view).contains("취소"), "{keys}");
    }
}

#[gpui_kit::test]
fn other_key_cancels_operator_without_running(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "other-cancel");
    set_cursor(cx, &view, 960, 62);
    // x(커서 노트 삭제), u(되돌리기), g a(뷰 전환), i x(i 뒤 객체가 아닌 키), ctrl-space(재생)
    for keys in ["d x", "d u", "d g a", "d i x", "c ctrl-space", "d shift-x"] {
        press(cx, handle, keys);
        assert_eq!(clip_a(cx, &view), ALL.to_vec(), "{keys}");
        assert_eq!(pending(cx, &view), None, "{keys}");
        assert_eq!(mode(cx, &view), Mode::Normal, "{keys}");
        assert_eq!(
            cx.read(|cx| view.read(cx).mode_state().view()),
            View::PianoRoll,
            "{keys}"
        );
        assert!(
            last_message(cx, &view).contains("오퍼레이터를 취소"),
            "{keys}"
        );
    }
    assert_eq!(
        cx.read(|cx| view.read(cx).transport_state()),
        keyscore_app::engine::TransportState::Stopped
    );
    // i 단독은 바인딩이 없어(null) 시간이 지나도 Step으로 가지 않고 대기가 남는다
    press(cx, handle, "d i");
    wait(cx, handle);
    assert_eq!(mode(cx, &view), Mode::Normal);
    assert_eq!(pending(cx, &view), Some((Operator::Delete, None)));
    // A13: 시퀀스 대기 중(i 뒤) Esc는 시퀀스만 취소한다. 다음 Esc가 오퍼레이터를 취소한다
    press(cx, handle, "escape");
    assert_eq!(pending(cx, &view), Some((Operator::Delete, None)));
    press(cx, handle, "escape");
    assert_eq!(pending(cx, &view), None);
}

#[gpui_kit::test]
fn escape_cancels_operator(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "esc-cancel");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d escape");
    assert_eq!(pending(cx, &view), None);
    press(cx, handle, "w");
    assert_eq!(cursor(cx, &view), (1920, 64));
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
}

// ---------- 10·11. y 접두 ----------

#[gpui_kit::test]
fn yank_prefix_then_object_or_motion(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "y-prefix");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "y i b");
    assert_eq!(mode(cx, &view), Mode::Normal);
    assert_eq!(internal(cx), Some(relative(&[C4, D4, E4, G4, D4B])));
    press(cx, handle, "y w");
    assert_eq!(cursor(cx, &view), (960, 62));
    assert_eq!(internal(cx), Some(relative(&[D4])));
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "3 y w");
    assert_eq!(internal(cx), Some(relative(&[C4, D4, E4])));
    assert_eq!(cursor(cx, &view), (0, 60));
    assert_eq!(pending(cx, &view), None);
    // 대기 시간 뒤의 y는 대기에 든다. 다시 y는 겹친 오퍼레이터(커서 노트 복사)
    set_cursor(cx, &view, 2880, 62);
    press(cx, handle, "y");
    wait(cx, handle);
    assert_eq!(pending(cx, &view), Some((Operator::Yank, None)));
    assert_eq!(status_mode(cx, handle), "NORMAL y");
    press(cx, handle, "y");
    wait(cx, handle);
    assert_eq!(internal(cx), Some(relative(&[D4B])));
    assert_eq!(pending(cx, &view), None);
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
}

#[gpui_kit::test]
fn yank_prefix_escape_cancels(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "y-esc");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "y escape");
    wait(cx, handle);
    assert_eq!(pending(cx, &view), None);
    assert_eq!(internal(cx), None);
    press(cx, handle, "w");
    assert_eq!(cursor(cx, &view), (1920, 64));
}

// ---------- 12. 선택이 있으면 바로 ----------

#[gpui_kit::test]
fn operator_with_selection_acts_now(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "with-selection");
    set_cursor(cx, &view, 3840, 69);
    press(cx, handle, "g b");
    assert_eq!(selection_of(cx, &view), vec![A4]);
    press(cx, handle, "y");
    // y는 `y y`의 접두라 대기 시간 뒤에 실행된다
    wait(cx, handle);
    assert_eq!(internal(cx), Some(relative(&[A4])));
    assert_eq!(pending(cx, &view), None);
    // 비주얼이 아닌 선택은 y 뒤에도 남는다
    assert_eq!(selection_of(cx, &view), vec![A4]);
    press(cx, handle, "d");
    assert_eq!(clip_a(cx, &view), without(&[A4]));
    assert_eq!(pending(cx, &view), None);
    assert!(selection_of(cx, &view).is_empty());
    set_cursor(cx, &view, 960, 62);
    select(cx, &view, &[D4, E4]);
    press(cx, handle, "c");
    assert_eq!(clip_a(cx, &view), without(&[A4, D4, E4]));
    assert_eq!(mode(cx, &view), Mode::Step);
    assert_eq!(cursor(cx, &view), (960, 62));
}

// ---------- 13. 배치 뷰 ----------

#[gpui_kit::test]
fn operator_in_arrangement_says_piano_roll(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "arrangement");
    press(cx, handle, "g a");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    press(cx, handle, "d");
    assert_eq!(pending(cx, &view), None);
    assert!(last_message(cx, &view).contains("피아노롤"));
    press(cx, handle, "v");
    assert_eq!(visual(cx, &view), None);
    assert!(last_message(cx, &view).contains("피아노롤"));
}

// ---------- 14. 상태줄 ----------

#[gpui_kit::test]
fn status_shows_pending_operator_and_visual(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "status");
    set_cursor(cx, &view, 960, 62);
    assert_eq!(status_mode(cx, handle), "NORMAL");
    press(cx, handle, "d");
    assert_eq!(status_mode(cx, handle), "NORMAL d");
    press(cx, handle, "escape");
    press(cx, handle, "c");
    assert_eq!(status_mode(cx, handle), "NORMAL c");
    press(cx, handle, "escape");
    press(cx, handle, "v");
    assert_eq!(status_mode(cx, handle), "VISUAL");
    press(cx, handle, "shift-v");
    assert_eq!(status_mode(cx, handle), "V-LINE");
    press(cx, handle, "ctrl-v");
    assert_eq!(status_mode(cx, handle), "V-BLOCK");
    press(cx, handle, "escape");
    assert_eq!(status_mode(cx, handle), "NORMAL");
    assert_eq!(visual(cx, &view), None);
}

// ---------- 15·16·17. 비주얼 ----------

#[gpui_kit::test]
fn visual_modes_select_by_kind(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "visual-kinds");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v");
    assert_eq!(visual(cx, &view), Some(VisualKind::Note));
    assert_eq!(selection_of(cx, &view), vec![D4]);
    press(cx, handle, "w w");
    assert_eq!(selection_of(cx, &view), vec![D4, E4, G4]);
    // 좁아질 수 있다
    press(cx, handle, "b");
    assert_eq!(selection_of(cx, &view), vec![D4, E4]);
    press(cx, handle, "escape");
    // V: 시간 범위의 모든 음높이
    set_cursor(cx, &view, 960, 30);
    press(cx, handle, "shift-v l");
    assert_eq!(visual(cx, &view), Some(VisualKind::Time));
    assert_eq!(selection_of(cx, &view), vec![D4, E4, G4]);
    press(cx, handle, "escape");
    // Ctrl+v: 직사각형. j k도 넓힌다
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "ctrl-v l k k");
    assert_eq!(visual(cx, &view), Some(VisualKind::Block));
    assert_eq!(selection_of(cx, &view), vec![D4, E4]);
    // 카운트도 받는다
    press(cx, handle, "escape");
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "v 2 w");
    assert_eq!(selection_of(cx, &view), vec![C4, D4, E4]);
}

#[gpui_kit::test]
fn visual_toggle_switch_and_escape(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "visual-toggle");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v");
    press(cx, handle, "v");
    assert_eq!(visual(cx, &view), None);
    assert!(selection_of(cx, &view).is_empty());
    assert!(!cx.read(|cx| view.read(cx).mode_state().has_selection()));
    // 종류만 바꾸고 기준 칸은 그대로
    set_cursor(cx, &view, 960, 30);
    press(cx, handle, "v l shift-v");
    assert_eq!(visual(cx, &view), Some(VisualKind::Time));
    assert_eq!(selection_of(cx, &view), vec![D4, E4, G4]);
    press(cx, handle, "escape");
    assert_eq!(visual(cx, &view), None);
    assert!(selection_of(cx, &view).is_empty());
    // 비주얼이 끝난 뒤 이동은 선택을 만들지 않는다
    press(cx, handle, "l");
    assert!(selection_of(cx, &view).is_empty());
}

#[gpui_kit::test]
fn visual_objects_replace_selection(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "visual-objects");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v i b");
    assert_eq!(mode(cx, &view), Mode::Normal);
    assert_eq!(visual(cx, &view), Some(VisualKind::Note));
    assert_eq!(selection_of(cx, &view), vec![C4, D4, E4, G4, D4B]);
    press(cx, handle, "i n");
    assert_eq!(selection_of(cx, &view), vec![D4]);
    press(cx, handle, "a c");
    assert_eq!(selection_of(cx, &view), ALL.to_vec());
    // i 단독은 Step으로 가지 않는다
    press(cx, handle, "i");
    wait(cx, handle);
    assert_eq!(mode(cx, &view), Mode::Normal);
    assert_eq!(visual(cx, &view), Some(VisualKind::Note));
}

// ---------- 18. 비주얼의 오퍼레이터 ----------

#[gpui_kit::test]
fn visual_operators_act_and_end(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "visual-ops");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v w y");
    wait(cx, handle);
    assert_eq!(internal(cx), Some(relative(&[D4, E4])));
    assert_eq!(visual(cx, &view), None);
    assert!(selection_of(cx, &view).is_empty());
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v w d");
    assert_eq!(clip_a(cx, &view), without(&[D4, E4]));
    assert_eq!(visual(cx, &view), None);
    assert_eq!(pending(cx, &view), None);
    // b는 읽기 순서 앞 노트 G4(1920, 67)로 간다. E4(1920, 64)는 그보다 앞이라 선택 밖
    set_cursor(cx, &view, 2880, 62);
    press(cx, handle, "v b c");
    assert_eq!(clip_a(cx, &view), vec![C4, A4]);
    assert_eq!(mode(cx, &view), Mode::Step);
    // 선택에 작용한 c: 가장 이른 대상 시작 틱, 음높이는 커서 그대로
    assert_eq!(cursor(cx, &view), (1920, 67));
}

// ---------- 19·20. 비주얼이 끝나는 때, 클릭 ----------

#[gpui_kit::test]
fn visual_ends_on_extend_select_and_view(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "visual-ends");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v g b");
    assert_eq!(visual(cx, &view), None);
    assert_eq!(selection_of(cx, &view), vec![C4, D4, E4, G4, D4B]);
    press(cx, handle, "escape");
    press(cx, handle, "v l");
    // Shift 넓히기(기본 프리셋의 ctrl-shift-l)는 비주얼을 끝내고 S3-T2 규칙으로
    press(cx, handle, "ctrl-shift-l");
    assert_eq!(visual(cx, &view), None);
    press(cx, handle, "escape");
    press(cx, handle, "v");
    press(cx, handle, "g a");
    assert_eq!(visual(cx, &view), None);
    press(cx, handle, "g a");
    assert_eq!(visual(cx, &view), None);
    press(cx, handle, "v");
    press(cx, handle, "i");
    wait(cx, handle);
    // 비주얼에서 i 단독은 아무 일도 하지 않는다. 첫 Esc는 i 시퀀스만 취소하고(A13) 둘째 Esc가 비주얼을 끝낸다.
    // Step 진입은 그 뒤의 i
    press(cx, handle, "escape");
    assert_eq!(visual(cx, &view), Some(VisualKind::Note));
    press(cx, handle, "escape i");
    assert_eq!(mode(cx, &view), Mode::Step);
    assert_eq!(visual(cx, &view), None);
}

#[gpui_kit::test]
fn visual_click_extends(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "visual-click");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "v");
    view.update(cx, |this, cx| {
        this.click_piano_roll_cell(
            Cursor {
                tick: Tick::new(2880),
                pitch: Pitch::new(62).unwrap(),
            },
            cx,
        );
    });
    render(cx, handle);
    assert_eq!(visual(cx, &view), Some(VisualKind::Note));
    assert_eq!(selection_of(cx, &view), vec![D4, E4, G4, D4B]);
}

// ---------- 21·22. `.`과 C ----------

#[gpui_kit::test]
fn dot_repeats_operator(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "dot");
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "d w");
    assert_eq!(clip_a(cx, &view), without(&[C4]));
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, ".");
    assert_eq!(clip_a(cx, &view), without(&[C4, D4]));
    // y는 마지막 편집을 바꾸지 않는다: `.`은 여전히 d w
    set_cursor(cx, &view, 1920, 64);
    press(cx, handle, "y w");
    press(cx, handle, ".");
    assert_eq!(clip_a(cx, &view), without(&[C4, D4, E4]));
    // c w 뒤 Esc, `.`은 지우고 Step
    let (handle, view) = open(cx);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "c w");
    assert_eq!(mode(cx, &view), Mode::Step);
    press(cx, handle, "escape");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, ".");
    assert_eq!(clip_a(cx, &view), without(&[C4, D4]));
    assert_eq!(mode(cx, &view), Mode::Step);
    // 카운트와 객체도 같다
    let (handle, view) = open(cx);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "2 d w");
    set_cursor(cx, &view, 1920, 64);
    press(cx, handle, ".");
    assert_eq!(clip_a(cx, &view), vec![D4B, A4]);
    let (handle, view) = open(cx);
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d i n");
    set_cursor(cx, &view, 2880, 62);
    press(cx, handle, ".");
    assert_eq!(clip_a(cx, &view), without(&[D4, D4B]));
}

#[gpui_kit::test]
fn change_to_bar_end(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "shift-c");
    set_cursor(cx, &view, 1920, 30);
    press(cx, handle, "shift-c");
    assert_eq!(clip_a(cx, &view), vec![C4, D4, A4]);
    assert_eq!(mode(cx, &view), Mode::Step);
    assert_eq!(cursor(cx, &view), (1920, 30));
    press(cx, handle, "escape");
    set_cursor(cx, &view, 3840, 30);
    press(cx, handle, ".");
    assert_eq!(clip_a(cx, &view), vec![C4, D4]);
    assert_eq!(mode(cx, &view), Mode::Step);
}

// ---------- 23·24. undo, 클릭 ----------

#[gpui_kit::test]
fn operator_is_one_undo_unit(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "undo");
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "d 3 w");
    assert_eq!(clip_a(cx, &view), without(&[C4, D4, E4]));
    assert_eq!(undo_len(cx, &view), 1);
    press(cx, handle, "u");
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
    // undo는 되살린 노트를 선택한다(S3-T3a). 선택이 있으면 c는 바로 작용하므로(결정 3 A) 먼저 비운다
    assert_eq!(selection_of(cx, &view), vec![C4, D4, E4]);
    press(cx, handle, "escape c a c");
    assert_eq!(clip_a(cx, &view), Vec::<Triple>::new());
    assert_eq!(clips_len(cx, &view), vec![(0, 2 * BAR), (4 * BAR, BAR)]);
    press(cx, handle, "escape u");
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
}

#[gpui_kit::test]
fn click_cancels_operator(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "click-cancel");
    set_cursor(cx, &view, 960, 62);
    press(cx, handle, "d");
    assert!(pending(cx, &view).is_some());
    view.update(cx, |this, cx| {
        this.click_piano_roll_cell(
            Cursor {
                tick: Tick::new(0),
                pitch: Pitch::new(60).unwrap(),
            },
            cx,
        );
    });
    render(cx, handle);
    assert_eq!(pending(cx, &view), None);
    press(cx, handle, "w");
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
}

// ---------- 25·26. 기본 프리셋, 팔레트 ----------

#[gpui_kit::test]
fn default_preset_unchanged(cx: &mut TestAppContext) {
    start(cx, "default", Preset::Default);
    let (handle, view) = open(cx);
    set_cursor(cx, &view, 960, 62);
    // 기본 프리셋에서 d는 비어 있다(5.8)
    press(cx, handle, "d");
    assert_eq!(clip_a(cx, &view), ALL.to_vec());
    assert_eq!(pending(cx, &view), None);
    // 기본 프리셋 v는 뷰 전환
    press(cx, handle, "v");
    assert_eq!(
        cx.read(|cx| view.read(cx).mode_state().view()),
        View::Arrangement
    );
    assert_eq!(visual(cx, &view), None);
    // 이동 카운트는 vim 프리셋만(기본 프리셋의 3은 길이 코드)
    press(cx, handle, "v");
    set_cursor(cx, &view, 0, 62);
    press(cx, handle, "l");
    assert_eq!(cursor(cx, &view), (960, 62));
}

#[gpui_kit::test]
fn palette_object_without_operator_says(cx: &mut TestAppContext) {
    let (handle, view) = setup(cx, "palette-object");
    set_cursor(cx, &view, 960, 62);
    cx.update_window(handle, |_, window, cx| {
        keyscore_app::dispatch::run_action(
            Box::new(keyscore_app::actions::object::InnerNote),
            window,
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    assert!(last_message(cx, &view).contains("오퍼레이터"));
    assert_eq!(selection_of(cx, &view), Vec::<Triple>::new());
}
