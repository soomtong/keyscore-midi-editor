//! S3-T2 명령줄(`:` 접두 ex 명령, 결정 A12), vim 프리셋 키(A2·A8·A10·A11), 시퀀스 대기 중 `Esc`(A13),
//! `operator::Cancel`, 미리듣기 설정(PRE-1). REQ 5.8, 5.8.1, 7.5, UI-2.

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::args::{Int, SettingsSection};
use keyscore_app::actions::{operator, settings};
use keyscore_app::app_state::AppSettings;
use keyscore_app::commands::ex::{self, ExCommand};
use keyscore_app::keymap::{self, Preset};
use keyscore_app::mode::{Mode, View};
use keyscore_app::settings::Settings;
use keyscore_app::settings_window;
use keyscore_app::window::{ProjectWindow, open_project_windows};
use keyscore_app::{init, open_project_window};
use keyscore_core::{Channel, Clip, Note, Pitch, Project, Tick, Track, TrackColor, Velocity};

const QUARTER: u64 = 960;
const BAR: u64 = QUARTER * 4;

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-cmdline-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
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

fn type_text(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    cx.simulate_input(handle, text);
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

fn note(start: u64, len: u32, pitch: u8) -> Note {
    Note::new(
        Tick::new(start),
        len,
        Pitch::new(pitch).unwrap(),
        Velocity::from_midi1(100).unwrap(),
    )
    .unwrap()
}

/// 클립 하나(0–4마디): C4@0, D4@960, C4@3840(2마디 첫 박), E4@2마디.
fn fixture() -> Project {
    let mut project = Project::new();
    let mut track = Track::new("T", Channel::new(1).unwrap(), TrackColor::new(0).unwrap());
    let mut clip = Clip::new(Tick::ZERO, 4 * BAR).unwrap();
    for n in [
        note(0, 960, 60),
        note(960, 960, 62),
        note(3840, 960, 60),
        note(2 * BAR, 960, 64),
    ] {
        clip.insert_note(n).unwrap();
    }
    track.insert_clip(clip).unwrap();
    project.push_track(track).unwrap();
    project
}

fn cursor_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> (u64, u8) {
    cx.read(|cx| {
        let c = view.read(cx).piano_roll().cursor();
        (c.tick.get(), c.pitch.get())
    })
}

fn selection_len(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> usize {
    cx.read(|cx| view.read(cx).selection().len())
}

fn view_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> View {
    cx.read(|cx| view.read(cx).mode_state().view())
}

fn mode_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Mode {
    cx.read(|cx| view.read(cx).mode_state().mode())
}

fn pending(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    cx.update_window(handle, |_, window, _| window.has_pending_keystrokes())
        .unwrap()
}

fn label_of(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &str) -> Option<String> {
    let id = id.to_owned();
    cx.update_window(handle, |_, window, _| {
        window
            .try_find(id)
            .and_then(|e| e.label().map(str::to_owned))
    })
    .unwrap()
}

// ---------- 5. ex 명령 파서 ----------

#[test]
fn ex_parse_maps_w_q_e_new_set_and_rejects_unknown() {
    assert_eq!(ex::parse(":w"), Ok(ExCommand::Write(None)));
    assert_eq!(
        ex::parse(":w  song.ksq "),
        Ok(ExCommand::Write(Some("song.ksq".to_owned())))
    );
    assert_eq!(ex::parse(":q"), Ok(ExCommand::Quit));
    assert_eq!(ex::parse(":e"), Ok(ExCommand::Edit(None)));
    assert_eq!(
        ex::parse(":e /tmp/a b.ksq"),
        Ok(ExCommand::Edit(Some("/tmp/a b.ksq".to_owned())))
    );
    assert_eq!(ex::parse(":new"), Ok(ExCommand::New));
    assert_eq!(ex::parse(":set"), Ok(ExCommand::Set(None)));
    assert_eq!(
        ex::parse(":set keymap"),
        Ok(ExCommand::Set(Some(SettingsSection::Keymap)))
    );
    assert_eq!(
        ex::parse(":set midi_output"),
        Ok(ExCommand::Set(Some(SettingsSection::MidiOutput)))
    );
    assert_eq!(ex::parse(": q"), Ok(ExCommand::Quit), "콜론 뒤 공백은 무시");
    for bad in [
        ":wq",
        ":x",
        ":",
        ":set nothing",
        ":q now",
        ":new a",
        "w",
        "",
    ] {
        assert!(ex::parse(bad).is_err(), "{bad}");
    }
    for cmd in [
        ExCommand::Write(None),
        ExCommand::Write(Some("a".into())),
        ExCommand::Quit,
        ExCommand::Edit(None),
        ExCommand::Edit(Some("a".into())),
        ExCommand::New,
        ExCommand::Set(None),
        ExCommand::Set(Some(SettingsSection::Input)),
    ] {
        assert!(!cmd.label().is_empty(), "{cmd:?}");
    }
    assert!(ex::is_command_line(":w"));
    assert!(ex::is_command_line("  :w"));
    assert!(!ex::is_command_line("w"));
    assert!(!ex::is_command_line(""));
}

// ---------- 12. vim 프리셋 키(결정 A2·A8·A10·A11) ----------

#[gpui_kit::test]
fn vim_preset_keys_from_decisions_a2_a8_a10_a11(cx: &mut TestAppContext) {
    start(cx, "vim-keys");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = open(cx, fixture());
    assert!(cx.update(|cx| keymap::errors(cx).is_empty()));
    // A8: 배치 뷰 enter는 클립 열기를 상속한다
    press(cx, handle, "enter");
    assert_eq!(view_of(cx, &view), View::PianoRoll);
    // A2: g a 뷰 전환
    press(cx, handle, "g a");
    assert_eq!(view_of(cx, &view), View::Arrangement);
    press(cx, handle, "g a");
    assert_eq!(view_of(cx, &view), View::PianoRoll);
    // A8: 피아노롤 enter는 노트 듣기(모드는 그대로). 엔진이 없어 알림만 남는다
    cx.update(|cx| {
        view.read(cx)
            .piano_roll()
            .set_cursor(keyscore_app::pianoroll::Cursor {
                tick: Tick::ZERO,
                pitch: Pitch::new(60).unwrap(),
            })
    });
    press(cx, handle, "enter");
    assert_eq!(
        mode_of(cx, &view),
        Mode::Normal,
        "vim enter는 Step 진입이 아니다"
    );
    let last = cx.read(|cx| view.read(cx).messages().last().cloned());
    assert!(
        last.as_deref().is_some_and(|m| m.contains("MIDI 엔진")),
        "{last:?}"
    );
    // A10: g b 마디 선택, g shift-b 클립 선택
    press(cx, handle, "g b");
    assert_eq!(selection_len(cx, &view), 2, "0마디: C4, D4");
    press(cx, handle, "g shift-b");
    assert_eq!(selection_len(cx, &view), 4);
    press(cx, handle, "escape");
    assert_eq!(selection_len(cx, &view), 0);
    // A11: f / F 와 ; , 반복
    press(cx, handle, "f");
    assert_eq!(cursor_of(cx, &view), (3840, 60), "같은 음높이의 다음 노트");
    press(cx, handle, ";");
    assert_eq!(cursor_of(cx, &view), (3840, 60), "더 없으면 그대로");
    press(cx, handle, ",");
    assert_eq!(cursor_of(cx, &view), (0, 60), "반대 방향");
    press(cx, handle, "shift-f");
    assert_eq!(cursor_of(cx, &view), (0, 60));
    press(cx, handle, "f ;");
    assert_eq!(cursor_of(cx, &view), (3840, 60));
    press(cx, handle, "shift-f");
    assert_eq!(cursor_of(cx, &view), (0, 60));
    press(cx, handle, ",");
    assert_eq!(cursor_of(cx, &view), (3840, 60), "F 뒤의 ,는 앞으로");
    // 프리셋 파일에서 결정 대기 주석이 사라졌다
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/keymaps/vim.json"
    ))
    .unwrap();
    for gone in [
        "결정 대기: A2",
        "결정 대기: A8",
        "결정 대기: A11",
        "결정 대기: A9/A10",
    ] {
        assert!(!text.contains(gone), "{gone}");
    }
    for present in [
        "\"g a\": \"view::Toggle\"",
        "\"enter\": \"note::Audition\"",
        "\"g b\": \"select::Bar\"",
        "\"g shift-b\": \"select::Clip\"",
        "\";\": \"cursor::RepeatFind\"",
        "\",\": \"cursor::RepeatFindReverse\"",
        "\"escape\": \"operator::Cancel\"",
    ] {
        assert!(text.contains(present), "{present}");
    }
}

// ---------- 13. 시퀀스 대기 중 Esc ----------

#[gpui_kit::test]
fn escape_during_pending_sequence_only_cancels_sequence(cx: &mut TestAppContext) {
    start(cx, "esc-pending");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, view) = open(cx, fixture());
    press(cx, handle, "enter");
    press(cx, handle, "g b");
    assert_eq!(selection_len(cx, &view), 2);
    press(cx, handle, "3");
    assert_eq!(cx.update(|cx| keymap::pending_count(cx)), Some(3));
    cx.simulate_keystrokes(handle, "g");
    assert!(pending(cx, handle), "g 뒤는 시퀀스 대기");
    press(cx, handle, "escape");
    assert!(!pending(cx, handle), "Esc는 대기를 비운다");
    assert_eq!(selection_len(cx, &view), 2, "선택 해제는 실행되지 않는다");
    assert_eq!(
        cx.update(|cx| keymap::pending_count(cx)),
        None,
        "카운트도 비운다"
    );
    assert_eq!(view_of(cx, &view), View::PianoRoll);
    // 대기가 비었으니 다음 g g 는 새 시퀀스다
    cx.update(|cx| {
        view.read(cx)
            .piano_roll()
            .set_cursor(keyscore_app::pianoroll::Cursor {
                tick: Tick::new(2 * BAR),
                pitch: Pitch::new(60).unwrap(),
            })
    });
    press(cx, handle, "g g");
    assert_eq!(cursor_of(cx, &view), (0, 60), "g g = 클립 처음");
    // 대기가 없을 때의 Esc는 평소처럼 선택을 비운다
    press(cx, handle, "escape");
    assert_eq!(selection_len(cx, &view), 0);
    // 기본 프리셋의 g는 바인딩이 없어 대기가 생기지 않고 Esc는 평소대로다
    cx.update(|cx| keymap::set_preset(cx, Preset::Default));
    let (handle, view) = open(cx, fixture());
    press(cx, handle, "enter");
    cx.update(|cx| {
        view.read(cx)
            .piano_roll()
            .set_cursor(keyscore_app::pianoroll::Cursor {
                tick: Tick::ZERO,
                pitch: Pitch::new(60).unwrap(),
            })
    });
    press(cx, handle, "b");
    assert_eq!(selection_len(cx, &view), 2);
    cx.simulate_keystrokes(handle, "g");
    assert!(!pending(cx, handle));
    press(cx, handle, "escape");
    assert_eq!(selection_len(cx, &view), 0);
}

// ---------- 14. operator::Cancel ----------

#[gpui_kit::test]
fn operator_cancel_clears_count(cx: &mut TestAppContext) {
    start(cx, "op-cancel");
    cx.update(|cx| keymap::set_preset(cx, Preset::Vim));
    let (handle, _view) = open(cx, fixture());
    press(cx, handle, "enter");
    press(cx, handle, "3");
    assert_eq!(cx.update(|cx| keymap::pending_count(cx)), Some(3));
    run(cx, handle, operator::Cancel);
    assert_eq!(cx.update(|cx| keymap::pending_count(cx)), None);
    // 명령 표에 있다(팔레트로 부를 수 있다)
    assert!(keyscore_app::commands::spec("operator::Cancel").is_some());
    assert!(keyscore_app::commands::spec("palette::OpenCommandLine").is_some());
    assert!(keyscore_app::commands::spec("settings::TogglePreviewOnMove").is_some());
    assert!(keyscore_app::commands::spec("settings::SetPreviewLength").is_some());
}

// ---------- 15. `:` 명령줄 ----------

#[gpui_kit::test]
fn colon_opens_palette_with_ex_prefix_and_runs_commands(cx: &mut TestAppContext) {
    start(cx, "colon");
    let (handle, view) = open(cx, fixture());
    let query = |cx: &mut TestAppContext| cx.read(|cx| view.read(cx).palette_query(cx));
    assert_eq!(query(cx), None);
    press(cx, handle, ":");
    assert_eq!(
        query(cx),
        Some(":".to_owned()),
        "`:`는 접두가 채워진 팔레트"
    );
    type_text(cx, handle, "set");
    assert_eq!(query(cx), Some(":set".to_owned()));
    let row = label_of(cx, handle, "palette-row-ex");
    assert!(
        row.as_deref().is_some_and(|l| l.contains("설정")),
        "{row:?}"
    );
    press(cx, handle, "enter");
    assert_eq!(query(cx), None, "실행하면 닫힌다");
    assert!(
        cx.update(|cx| settings_window::current(cx).is_some()),
        ":set은 설정 창을 연다"
    );
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    // 모르는 명령은 실행 불가 줄이고 Enter로 닫히지 않는다
    press(cx, handle, ":");
    type_text(cx, handle, "wq");
    let row = label_of(cx, handle, "palette-row-ex");
    assert!(
        row.as_deref().is_some_and(|l| l.contains("모르는")),
        "{row:?}"
    );
    press(cx, handle, "enter");
    assert_eq!(query(cx), Some(":wq".to_owned()));
    press(cx, handle, "escape");
    assert_eq!(query(cx), None);
    // :new 는 새 창
    let before = cx.read(|cx| open_project_windows(cx).len());
    press(cx, handle, ":");
    type_text(cx, handle, "new");
    press(cx, handle, "enter");
    assert_eq!(cx.read(|cx| open_project_windows(cx).len()), before + 1);
    // 접두 없는 질의는 Action 검색 그대로
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    press(cx, handle, "cmd-shift-p");
    assert_eq!(query(cx), Some(String::new()), "Cmd+Shift+P는 빈 질의");
    type_text(cx, handle, "view::toggle");
    assert!(label_of(cx, handle, "palette-row-view::Toggle").is_some());
    assert!(label_of(cx, handle, "palette-row-ex").is_none());
    press(cx, handle, "escape");
    // `:`를 두 번 누르면 질의는 하나의 콜론이다
    press(cx, handle, ":");
    press(cx, handle, ":");
    assert_eq!(query(cx), Some(":".to_owned()));
    press(cx, handle, "escape");
    // :e 경로 는 그 파일을 연다
    let dir = std::env::temp_dir().join(format!("ks-cmdline-e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("song.ksq");
    keyscore_app::files::save_project(&path, &fixture()).unwrap();
    let before = cx.read(|cx| open_project_windows(cx).len());
    press(cx, handle, ":");
    type_text(cx, handle, &format!("e {}", path.display()));
    press(cx, handle, "enter");
    assert_eq!(cx.read(|cx| open_project_windows(cx).len()), before + 1);
    let opened = cx.read(|cx| {
        open_project_windows(cx)
            .iter()
            .any(|w| w.read(cx).document().read(cx).path() == Some(path.as_path()))
    });
    assert!(opened);
}

// ---------- 19. 미리듣기 설정 ----------

#[test]
fn preview_settings_round_trip_and_defaults() {
    let dir = std::env::temp_dir().join(format!("ks-cmdline-preview-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    let (s, warnings) = Settings::load(&path);
    assert!(warnings.is_empty());
    assert!(s.preview.on_move, "기본 켬(5.3)");
    assert_eq!(s.preview.length_ms, 200, "기본 200 ms(PRE-1)");
    std::fs::write(
        &path,
        r#"{"preview": {"on_move": false, "length_ms": 50, "later": 1}, "other": true}"#,
    )
    .unwrap();
    let (mut s, warnings) = Settings::load(&path);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(!s.preview.on_move);
    assert_eq!(s.preview.length_ms, 50);
    s.preview.length_ms = 300;
    s.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"later\": 1"), "모르는 키 보존: {text}");
    assert!(text.contains("\"other\": true"));
    let (again, _) = Settings::load(&path);
    assert_eq!(again.preview.length_ms, 300);
    assert!(!again.preview.on_move);
    // 잘못된 값은 그 필드만 기본값 + 경고
    std::fs::write(&path, r#"{"preview": {"on_move": "yes", "length_ms": 0}}"#).unwrap();
    let (s, warnings) = Settings::load(&path);
    assert!(s.preview.on_move);
    assert_eq!(s.preview.length_ms, 200);
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    std::fs::write(&path, r#"{"preview": {"length_ms": 20000}}"#).unwrap();
    let (s, warnings) = Settings::load(&path);
    assert_eq!(s.preview.length_ms, 200, "상한 10000 ms");
    assert_eq!(warnings.len(), 1);
    std::fs::write(&path, r#"{"preview": 3}"#).unwrap();
    let (s, warnings) = Settings::load(&path);
    assert!(s.preview.on_move);
    assert_eq!(warnings.len(), 1);
}

#[gpui_kit::test]
fn preview_settings_have_controls_and_actions(cx: &mut TestAppContext) {
    start(cx, "preview-ui");
    let (handle, _view) = open(cx, fixture());
    run(
        cx,
        handle,
        settings::Open(keyscore_app::actions::args::Opt(Some(
            SettingsSection::Input,
        ))),
    );
    let (sh, _sv) = cx
        .update(|cx| settings_window::current(cx))
        .expect("설정 창");
    cx.update_window(sh, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    render(cx, sh);
    let toggle = label_of(cx, sh, "settings-preview-on-move");
    assert!(
        toggle.as_deref().is_some_and(|l| l.contains("켬")),
        "{toggle:?}"
    );
    let length = label_of(cx, sh, "settings-preview-length");
    assert!(
        length.as_deref().is_some_and(|l| l.contains("200")),
        "{length:?}"
    );
    run(cx, sh, settings::TogglePreviewOnMove);
    assert!(!cx.read(|cx| AppSettings::get(cx).preview.on_move));
    let toggle = label_of(cx, sh, "settings-preview-on-move");
    assert!(
        toggle.as_deref().is_some_and(|l| l.contains("끔")),
        "{toggle:?}"
    );
    run(cx, sh, settings::SetPreviewLength(Int(500)));
    assert_eq!(cx.read(|cx| AppSettings::get(cx).preview.length_ms), 500);
    let length = label_of(cx, sh, "settings-preview-length");
    assert!(
        length.as_deref().is_some_and(|l| l.contains("500")),
        "{length:?}"
    );
    run(cx, sh, settings::SetPreviewLength(Int(0)));
    assert_eq!(
        cx.read(|cx| AppSettings::get(cx).preview.length_ms),
        500,
        "범위 밖은 무시"
    );
    run(cx, sh, settings::TogglePreviewOnMove);
    assert!(cx.read(|cx| AppSettings::get(cx).preview.on_move));
}
