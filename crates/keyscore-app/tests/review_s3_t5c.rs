//! S3-T5c 적대적 리뷰 1회차 시험(`docs/reports/s3-t5c/review-1.md`). 헤드리스 창, MIDI 없음.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::KeyLayout;
use keyscore_app::actions::{cursor, edit};
use keyscore_app::app_state::AppSettings;
use keyscore_app::keyboard_view::KeyCap;
use keyscore_app::mode::{Mode, ModeEvent};
use keyscore_app::pianoroll::scene::{self, Scene, SceneInput, Shade, row_shade};
use keyscore_app::pianoroll::{Cursor, MaxLenCache, Selection, Viewport};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{init, open_project_window};
use keyscore_core::{
    Channel, Edit, Key, MeterMap, Pitch, PitchClass, Project, Tick, Track, TrackColor, TrackId,
    TrackProp,
};

// ---------- 픽스처(keyboard_view_ui.rs와 같은 꼴) ----------

fn start(cx: &mut TestAppContext, name: &str) {
    let dir = std::env::temp_dir().join(format!("ks-rv-t5c-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
}

fn track(name: &str, channel: u8) -> Track {
    Track::new(
        name,
        Channel::new(channel).unwrap(),
        TrackColor::new(0).unwrap(),
    )
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

/// 피아노롤 뷰의 Step 모드로 연다. 커서는 틱 0, C4.
fn step(
    cx: &mut TestAppContext,
    name: &str,
    p: Project,
    layout: KeyLayout,
) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    start(cx, name);
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| AppSettings::update(cx, |s| s.input.key_layout = layout));
    cx.run_until_parked();
    render(cx, handle);
    event(cx, handle, &view, ModeEvent::ToggleView);
    cx.update(|cx| {
        view.read(cx).piano_roll().set_cursor(Cursor {
            tick: Tick::ZERO,
            pitch: Pitch::new(60).unwrap(),
        });
    });
    render(cx, handle);
    event(cx, handle, &view, ModeEvent::Enter(Mode::Step));
    (handle, view)
}

fn apply(cx: &mut TestAppContext, handle: AnyWindowHandle, view: &Entity<ProjectWindow>, e: Edit) {
    cx.update(|cx| {
        view.read(cx).document().clone().update(cx, |d, cx| {
            d.apply(vec![e]).unwrap();
            cx.notify();
        });
    });
    cx.run_until_parked();
    render(cx, handle);
}

fn shown(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<KeyCap> {
    cx.read(|cx| view.read(cx).keyboard_view(cx)).unwrap()
}

fn shaded(caps: &[KeyCap]) -> bool {
    caps.iter().any(|c| c.out_of_key)
}

fn d_major() -> Key {
    Key::Major(PitchClass::new(2).unwrap())
}

fn current(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<TrackId> {
    cx.read(|cx| view.read(cx).current_track())
}

// ---------- 1. 트랙 전환·채널 변경·undo·redo 뒤 건반 뷰 ----------

#[gpui_kit::test]
fn keyboard_view_follows_switch_between_melody_and_drum_tracks(cx: &mut TestAppContext) {
    let mut p = Project::new();
    p.push_track(track("Lead", 1)).unwrap();
    p.push_track(track("Drums", 10)).unwrap();
    let ids: Vec<TrackId> = p.tracks().iter().map(Track::id).collect();
    let (handle, view) = step(cx, "switch", p, KeyLayout::Piano);
    apply(cx, handle, &view, Edit::SetKey(d_major()));
    assert_eq!(current(cx, &view), Some(ids[0]));
    assert!(shaded(&shown(cx, &view)), "멜로디: D장조 명암");

    run(cx, handle, cursor::LastTrack);
    assert_eq!(current(cx, &view), Some(ids[1]), "드럼 트랙으로 갔다");
    let caps = shown(cx, &view);
    assert!(!shaded(&caps), "드럼: 명암 없음");
    // C4 자리 이름이 음 이름(`C4`)이 아니라 드럼 라벨(`n60`)이다
    assert_eq!(caps[0].name.as_deref(), Some("n60"), "드럼 라벨로 바뀌었다");

    run(cx, handle, cursor::FirstTrack);
    assert_eq!(current(cx, &view), Some(ids[0]));
    assert!(
        shaded(&shown(cx, &view)),
        "멜로디로 돌아오면 명암이 다시 생긴다"
    );
}

#[gpui_kit::test]
fn keyboard_view_follows_channel_change_undo_redo(cx: &mut TestAppContext) {
    let mut p = Project::new();
    p.push_track(track("Lead", 1)).unwrap();
    let id = p.tracks()[0].id();
    let (handle, view) = step(cx, "channel", p, KeyLayout::Piano);
    apply(cx, handle, &view, Edit::SetKey(d_major()));
    assert!(shaded(&shown(cx, &view)));

    apply(
        cx,
        handle,
        &view,
        Edit::SetTrackProp {
            track: id,
            prop: TrackProp::Channel(Channel::new(10).unwrap()),
        },
    );
    assert!(!shaded(&shown(cx, &view)), "1 → 10: 명암 없음");

    run(cx, handle, edit::Undo);
    assert!(shaded(&shown(cx, &view)), "undo(10 → 1): 명암 되살아남");

    run(cx, handle, edit::Redo);
    assert!(!shaded(&shown(cx, &view)), "redo(1 → 10): 명암 없음");

    // 10 → 다른 채널
    apply(
        cx,
        handle,
        &view,
        Edit::SetTrackProp {
            track: id,
            prop: TrackProp::Channel(Channel::new(11).unwrap()),
        },
    );
    assert!(shaded(&shown(cx, &view)), "10 → 11: 명암 되살아남");

    run(cx, handle, edit::Undo);
    assert!(!shaded(&shown(cx, &view)), "undo(11 → 10): 명암 없음");
}

// ---------- 2. 피아노롤 장면 행 명암 ----------

fn rows(track: Option<&Track>, key: Key) -> Vec<(u8, Shade)> {
    let mut cache = MaxLenCache::default();
    cache.refresh(track, 0);
    let selection = Selection::default();
    let mut out = Scene::default();
    // 128행이 모두 보이는 뷰포트
    let vp = Viewport {
        first_tick: 0.0,
        px_per_tick: 0.1,
        first_row: 0.0,
        row_height: 1.0,
        width: 100.0,
        height: 128.0,
    };
    scene::build(
        &SceneInput {
            track,
            current: None,
            cache: &cache,
            meter: &MeterMap::default(),
            key,
            cursor: Cursor {
                tick: Tick::ZERO,
                pitch: Pitch::new(60).unwrap(),
            },
            step_ticks: 240,
            selection: &selection,
            playhead: None,
            viewport: vp,
        },
        &mut out,
    );
    out.rows.iter().map(|r| (r.pitch.get(), r.shade)).collect()
}

fn all_keys() -> Vec<Key> {
    let mut keys = vec![Key::None];
    for pc in 0..12 {
        keys.push(Key::Major(PitchClass::new(pc).unwrap()));
        keys.push(Key::Minor(PitchClass::new(pc).unwrap()));
    }
    keys
}

/// 채널 16개 × 조성 25개 × 128행. 드럼(10)만 조성 없음, 나머지는 조성대로(멜로디 회귀 없음).
#[test]
fn scene_rows_shade_by_channel_and_key_everywhere() {
    for ch in 1..=16u8 {
        let t = track("T", ch);
        for key in all_keys() {
            let got = rows(Some(&t), key);
            assert_eq!(got.len(), 128, "ch{ch} {key:?}");
            let effective = if ch == 10 { Key::None } else { key };
            for (pitch, shade) in got {
                assert_eq!(
                    shade,
                    row_shade(effective, Pitch::new(pitch).unwrap()),
                    "ch{ch} {key:?} pitch {pitch}"
                );
            }
        }
    }
}

/// 트랙이 없으면 드럼이 아니다. 조성 명암 그대로.
#[test]
fn scene_rows_without_track_keep_key_shading() {
    let got = rows(None, d_major());
    // C4(60)는 D장조 밖
    let c4 = got.iter().find(|(p, _)| *p == 60).unwrap();
    assert_eq!(c4.1, Shade::Dark);
}

/// 같은 장면 버퍼를 재사용할 때(창의 `prepaint`와 같다) 멜로디 → 드럼 → 멜로디로 바꿔도 낡은 행이 남지 않는다.
#[test]
fn scene_buffer_reuse_does_not_keep_stale_shading() {
    let melody = track("L", 1);
    let drums = track("D", 10);
    let selection = Selection::default();
    let mut cache = MaxLenCache::default();
    let mut out = Scene::default();
    let vp = Viewport {
        first_tick: 0.0,
        px_per_tick: 0.1,
        first_row: 0.0,
        row_height: 1.0,
        width: 100.0,
        height: 128.0,
    };
    for (t, expect_key) in [
        (&melody, d_major()),
        (&drums, Key::None),
        (&melody, d_major()),
    ] {
        cache.refresh(Some(t), 0);
        scene::build(
            &SceneInput {
                track: Some(t),
                current: None,
                cache: &cache,
                meter: &MeterMap::default(),
                key: d_major(),
                cursor: Cursor {
                    tick: Tick::ZERO,
                    pitch: Pitch::new(60).unwrap(),
                },
                step_ticks: 240,
                selection: &selection,
                playhead: None,
                viewport: vp,
            },
            &mut out,
        );
        assert_eq!(out.rows.len(), 128);
        for r in &out.rows {
            assert_eq!(r.shade, row_shade(expect_key, r.pitch), "{:?}", t.channel());
        }
    }
}

// ---------- 3. 검은건반 이름 글 넘침 ----------

/// 보이는 건반 중 이름이 그려진 검은건반마다 「글 + 4 px ≤ 건반」인지. 넘치는 것을 모아 돌려준다.
fn overflowing(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
) -> Vec<String> {
    let caps = shown(cx, view);
    let mut bad = Vec::new();
    for (i, cap) in caps.iter().enumerate() {
        if cap.name.is_none() || !cap.black {
            continue;
        }
        let key_id = gpui_kit::ElementId::Name(format!("keyboard-key-{i}").into());
        let name_id = gpui_kit::ElementId::Name(format!("keyboard-name-{i}").into());
        let (key, name) = cx
            .update_window(handle, move |_, window, _| {
                (
                    window.find(key_id).bounds(),
                    window.try_find(name_id).map(|e| e.bounds()),
                )
            })
            .unwrap();
        let Some(name) = name else { continue };
        let (kw, nw) = (f32::from(key.size.width), f32::from(name.size.width));
        if nw + 4.0 > kw {
            bad.push(format!("{i} {:?}: 글 {nw} 건반 {kw}", cap.name));
        }
    }
    bad
}

fn octave(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> u8 {
    cx.read(|cx| view.read(cx).step_status().octave)
}

/// 모든 옥타브를 돌며 넘침을 모은다.
fn sweep_octaves(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ProjectWindow>,
) -> Vec<String> {
    for _ in 0..12 {
        press(cx, handle, "[");
    }
    let mut bad = Vec::new();
    let mut last = u8::MAX;
    loop {
        let o = octave(cx, view);
        if o == last {
            break;
        }
        last = o;
        for b in overflowing(cx, handle, view) {
            bad.push(format!("oct{o} {b}"));
        }
        press(cx, handle, "]");
    }
    bad
}

fn wide(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.simulate_window_resize(
        handle,
        gpui_kit::size(gpui_kit::px(2400.), gpui_kit::px(1400.)),
    );
    cx.run_until_parked();
    render(cx, handle);
}

fn one_track(channel: u8) -> Project {
    let mut p = Project::new();
    p.push_track(track("T", channel)).unwrap();
    p
}

#[gpui_kit::test]
fn black_key_drum_names_fit_every_octave_piano(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "fit-piano", one_track(10), KeyLayout::Piano);
    wide(cx, handle);
    let bad = sweep_octaves(cx, handle, &view);
    assert!(bad.is_empty(), "{bad:#?}");
}

#[gpui_kit::test]
fn black_key_drum_names_fit_every_octave_solfege(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "fit-solfege", one_track(10), KeyLayout::Solfege);
    wide(cx, handle);
    let bad = sweep_octaves(cx, handle, &view);
    assert!(bad.is_empty(), "{bad:#?}");
}

/// 기본 창 크기에서도 넘치지 않는가(시험 창 기본 크기, 넓히지 않음).
#[gpui_kit::test]
fn black_key_drum_names_fit_default_window(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "fit-default", one_track(10), KeyLayout::Piano);
    let key = cx
        .update_window(handle, |_, window, _| {
            window.find("keyboard-key-1").bounds()
        })
        .unwrap();
    let bad = sweep_octaves(cx, handle, &view);
    assert!(
        bad.is_empty(),
        "검은건반 너비 {:?}: {bad:#?}",
        key.size.width
    );
}

/// 멜로디 트랙은 C 건반(흰건반)에만 이름을 그린다. 검은건반 이름 자리는 비어야 한다.
#[gpui_kit::test]
fn melody_black_keys_draw_no_name(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "fit-melody", one_track(1), KeyLayout::Piano);
    wide(cx, handle);
    let bad = sweep_octaves(cx, handle, &view);
    assert!(bad.is_empty(), "{bad:#?}");
    let caps = shown(cx, &view);
    for (i, cap) in caps.iter().enumerate().filter(|(_, c)| c.black) {
        let name_id = gpui_kit::ElementId::Name(format!("keyboard-name-{i}").into());
        let w = cx
            .update_window(handle, move |_, window, _| {
                window
                    .try_find(name_id)
                    .map(|e| f32::from(e.bounds().size.width))
            })
            .unwrap();
        assert!(
            w.is_none_or(|w| w < 0.5),
            "{i} {:?} 이름 너비 {w:?}",
            cap.name
        );
    }
}

/// 최소 창 크기(`PROJECT_WINDOW_MIN_SIZE` 720×480)에서도 드럼 라벨이 넘치는가. 계획은 「넓은 창」만 다뤘다.
/// 이 크기에서도 검은건반은 24 px로, 넘침은 `n100`~`n127`뿐이다(넓은 창과 같다).
#[gpui_kit::test]
fn black_key_drum_names_fit_min_window(cx: &mut TestAppContext) {
    let (handle, view) = step(cx, "fit-narrow", one_track(10), KeyLayout::Piano);
    cx.simulate_window_resize(
        handle,
        gpui_kit::size(gpui_kit::px(720.), gpui_kit::px(480.)),
    );
    cx.run_until_parked();
    render(cx, handle);
    let key = cx
        .update_window(handle, |_, window, _| {
            window.find("keyboard-key-1").bounds()
        })
        .unwrap();
    let bad = sweep_octaves(cx, handle, &view);
    assert!(
        bad.is_empty(),
        "검은건반 너비 {:?}: {bad:#?}",
        key.size.width
    );
}
