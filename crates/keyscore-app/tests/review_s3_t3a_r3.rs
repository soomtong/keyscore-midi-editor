//! S3-T3a 적대적 리뷰 3. 리뷰 2 지적 1의 수정(있던 클립 연장이 마디 중간 다음 클립 시작에서 멈춤)을 붙여넣기·복제·
//! 반복·vim `p` 경로에서 다시 깨뜨려 본다(`docs/reports/s3-t3a/review-3.md`). 시스템 클립보드는 시험마다 이름 붙은
//! pasteboard를 쓴다.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::clipboard;
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::View;
use keyscore_app::pianoroll::Cursor;
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Clip, Note, Pitch, Project, Tick, TimeSignature, Track, TrackColor, Velocity,
};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

type Triple = (u64, u32, u8);

// ---------- 픽스처(review_s3_t3a.rs와 같다) ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-review3-t3a-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| {
        init(cx, dir.join("settings.json"));
        clipboard::use_named_pasteboard(
            cx,
            &format!("ks-review3-t3a-{}-{name}", std::process::id()),
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

fn assert_no_overlap(all: &[(u64, u64, Vec<Triple>)]) {
    for w in all.windows(2) {
        assert!(w[0].0 + w[0].1 <= w[1].0, "겹침: {all:?}");
    }
}

// ---------- 리뷰 2 지적 1 수정의 앱 경로 ----------

/// 클립 A [0, 1마디) E4(2880, 8분음표), B는 마디 중간(5000)에서 시작. `r` 넷: 사본 셋(3360, 3840, 4320)은 A를
/// B 시작(5000)까지 늘려 들어가고, 넷째(4800..5280)는 B에 닿아 거부된다. `cmd-z` 한 번에 노트와 A 길이가 돌아온다.
#[gpui_kit::test]
fn repeat_chain_fills_up_to_mid_bar_next_clip_and_undoes_once(cx: &mut TestAppContext) {
    let project = project_with(vec![
        clip(0, BAR, &[note(2880, 480, 64)]),
        clip(5000, BAR, &[]),
    ]);
    let (handle, view) = setup_with(cx, "repeat-mid-next", project);
    set_cursor(cx, &view, 2880, 64);
    press(cx, handle, "r");
    press(cx, handle, "r");
    press(cx, handle, "r");
    let all = clips(cx, &view);
    assert_eq!(all[0].1, 5000, "{all:?} {:?}", last_message(cx, &view));
    assert_eq!(
        all[0].2,
        vec![
            (2880, 480, 64),
            (3360, 480, 64),
            (3840, 480, 64),
            (4320, 480, 64)
        ]
    );
    assert_eq!(selection_of(cx, &view), vec![(4320, 480, 64)]);
    press(cx, handle, "r");
    assert_eq!(
        clips(cx, &view)[0].2.len(),
        4,
        "{:?}",
        last_message(cx, &view)
    );
    assert!(last_message(cx, &view).starts_with("반복:"));
    assert_no_overlap(&clips(cx, &view));
    assert_eq!(undo_len(cx, &view), 1);
    press(cx, handle, "cmd-z");
    let all = clips(cx, &view);
    assert_eq!((all[0].1, all[0].2.clone()), (BAR, vec![(2880, 480, 64)]));
    press(cx, handle, "cmd-shift-z");
    assert_eq!(clips(cx, &view)[0].1, 5000);
    // redo 뒤 선택은 「새로 나타난 노트」 셋 전부다(계획 6). 마지막 사본 하나가 아니다(동작 기록)
    assert_eq!(
        selection_of(cx, &view),
        vec![(3360, 480, 64), (3840, 480, 64), (4320, 480, 64)]
    );
}

/// 1마디부터 3/4. A [0, 1마디) C4, B는 3/4 마디 안(5840)에서 시작. C4를 3000에 붙이면 A는 5840까지 늘어난다.
#[gpui_kit::test]
fn paste_after_meter_change_stops_at_mid_bar_next_clip(cx: &mut TestAppContext) {
    let mut project = project_with(vec![
        clip(0, BAR, &[note(0, 960, 60)]),
        clip(5840, BAR, &[]),
    ]);
    project
        .meter_map_mut()
        .set(Tick::new(BAR), TimeSignature::new(3, 4).unwrap());
    let (handle, view) = setup_with(cx, "paste-meter-mid-next", project);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-c");
    set_cursor(cx, &view, 3000, 60);
    press(cx, handle, "cmd-v");
    let all = clips(cx, &view);
    assert_eq!(
        (all[0].1, all[0].2.clone()),
        (5840, vec![(0, 960, 60), (3000, 960, 60)]),
        "{:?}",
        last_message(cx, &view)
    );
    assert_eq!(selection_of(cx, &view), vec![(3000, 960, 60)]);
}

/// vim `p`(커서 + 한 스텝): 커서 1920에서 2박 C4를 붙이면 자리 2880, 끝 4800. A는 B(5000) 앞까지 늘어난다.
#[gpui_kit::test]
fn vim_p_near_clip_end_extends_up_to_mid_bar_next_clip(cx: &mut TestAppContext) {
    start(cx, "vim-p-mid-next");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let project = project_with(vec![
        clip(0, BAR, &[note(0, 1920, 60)]),
        clip(5000, BAR, &[]),
    ]);
    let (handle, view) = open(cx, project);
    enter_piano_roll(cx, handle, &view);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-c");
    set_cursor(cx, &view, 1920, 60);
    press(cx, handle, "p");
    let all = clips(cx, &view);
    assert_eq!(
        (all[0].1, all[0].2.clone()),
        (5000, vec![(0, 1920, 60), (2880, 1920, 60)]),
        "{:?}",
        last_message(cx, &view)
    );
}

/// 숨김 꼬리(3000, 1920틱, 끝 4920)가 있는 A [0, 1마디)와 꼬리 구간 안(4500)에서 시작하는 B. C4(1박)를 3500에
/// 붙이면 A는 4500까지 늘고 꼬리는 그대로다. 꼬리 노트를 `cmd-d`하면(사본 4920..) B에 닿아 거부된다.
#[gpui_kit::test]
fn paste_into_clip_with_hidden_tail_before_next_clip(cx: &mut TestAppContext) {
    let mut project = Project::new();
    let mut track = Track::new(
        "Lead",
        Channel::new(2).unwrap(),
        TrackColor::new(0).unwrap(),
    );
    let a = track
        .insert_clip(clip(0, 2 * BAR, &[note(0, 960, 60), note(3000, 1920, 64)]))
        .unwrap();
    let right = track.split_clip(a, Tick::new(BAR)).unwrap();
    track.remove_clip(right).unwrap();
    track.insert_clip(clip(4500, BAR, &[])).unwrap();
    project.push_track(track).unwrap();
    let (handle, view) = setup_with(cx, "paste-tail", project);
    set_cursor(cx, &view, 0, 60);
    press(cx, handle, "cmd-c");
    set_cursor(cx, &view, 3500, 60);
    press(cx, handle, "cmd-v");
    let all = clips(cx, &view);
    assert_eq!(
        (all[0].1, all[0].2.clone()),
        (4500, vec![(0, 960, 60), (3000, 1920, 64), (3500, 960, 60)]),
        "{:?}",
        last_message(cx, &view)
    );
    press(cx, handle, "escape");
    set_cursor(cx, &view, 3000, 64);
    press(cx, handle, "cmd-d");
    assert_eq!(
        clips(cx, &view)[0].2.len(),
        3,
        "{:?}",
        last_message(cx, &view)
    );
    assert!(last_message(cx, &view).starts_with("복제:"));
    press(cx, handle, "cmd-z");
    assert_eq!(clips(cx, &view)[0].1, BAR);
}
