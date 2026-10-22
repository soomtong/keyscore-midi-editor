//! S2-T7c SMF·계이름 텍스트 가져오기·내보내기(IO-2, IO-3, 결정 2026-10-22-s2-t7c-import-export).
//! 대화상자는 GPUI 시험 플랫폼이 대신 답한다.

use std::path::{Path, PathBuf};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{edit, file, track};
use keyscore_app::window::{ProjectWindow, open_project_windows};
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, Tick, ksq};
use keyscore_midi::{SmfFile, Song};

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-impexp-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn open(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.activate_window();
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    (handle, view)
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn answer_open(cx: &mut TestAppContext, path: PathBuf) {
    assert!(cx.did_prompt_for_paths(), "파일 대화상자가 없다");
    cx.simulate_path_prompt_response(move |_| Some(vec![path]));
    cx.run_until_parked();
}

fn answer_new_path(cx: &mut TestAppContext, path: PathBuf) {
    assert!(cx.did_prompt_for_new_path(), "새 경로 대화상자가 없다");
    cx.simulate_new_path_selection(move |_| Some(path));
    cx.run_until_parked();
}

fn prompt_text(cx: &mut TestAppContext) -> String {
    let (msg, detail) = cx.pending_prompt().expect("시트가 없다");
    format!("{msg}\n{detail}")
}

fn answer(cx: &mut TestAppContext, button: &str) {
    assert!(cx.has_pending_prompt(), "시트가 없다");
    cx.simulate_prompt_answer(button);
    cx.run_until_parked();
}

fn windows(cx: &mut TestAppContext) -> Vec<Entity<ProjectWindow>> {
    let open = cx.update(|cx| cx.windows());
    cx.update(|cx| open_project_windows(cx))
        .into_iter()
        .filter(|w| open.contains(&cx.read(|cx| w.read(cx).window_handle())))
        .collect()
}

fn messages(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<String> {
    cx.read(|cx| {
        view.read(cx)
            .messages()
            .iter()
            .map(ToString::to_string)
            .collect()
    })
}

fn project_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Project {
    cx.read(|cx| {
        let text = ksq::to_string(view.read(cx).document().read(cx).project());
        ksq::from_str(&text).unwrap().0
    })
}

fn dirty(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).document().read(cx).is_dirty())
}

fn note_count(p: &Project) -> usize {
    p.tracks()
        .iter()
        .flat_map(keyscore_core::Track::clips)
        .map(|(_, c)| c.notes().len())
        .sum()
}

fn sample_mid() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../keyscore-midi/tests/data/smf/alouette.mid")
}

fn sample() -> Project {
    ksq::from_str(
        r#"{"tempo": [[0, 500000]], "tracks": [{"name": "Lead", "channel": 1,
            "clips": [{"start": 0, "length": 3840, "notes": [[0, 480, 60, 51492]]}]}]}"#,
    )
    .unwrap()
    .0
}

// ---------- SMF ----------

#[gpui_kit::test]
fn import_smf_opens_new_window_with_summary(cx: &mut TestAppContext) {
    start(cx, "smf-import");
    let (handle, _) = open(cx, sample());
    run(cx, handle, track::New); // 손댄 창은 남는다
    run(cx, handle, file::ImportSmf);
    answer_open(cx, sample_mid());
    let all = windows(cx);
    assert_eq!(all.len(), 2, "새 창이 없다");
    let imported = all
        .iter()
        .find(|w| cx.read(|cx| w.read(cx).window_handle()) != handle)
        .unwrap()
        .clone();
    let expected = Song::from_smf(&SmfFile::read(&std::fs::read(sample_mid()).unwrap()).unwrap())
        .unwrap()
        .to_project()
        .unwrap();
    assert_eq!(
        ksq::to_string(&project_of(cx, &imported)),
        ksq::to_string(&expected)
    );
    assert!(dirty(cx, &imported), "가져온 창은 저장 안 된 상태다");
    assert!(cx.read(|cx| imported.read(cx).document().read(cx).path().is_none()));
    let msgs = messages(cx, &imported);
    assert!(
        msgs.iter()
            .any(|m| m.contains("가져옴") && m.contains("alouette")),
        "{msgs:?}"
    );
}

#[gpui_kit::test]
fn import_smf_replaces_pristine_window(cx: &mut TestAppContext) {
    start(cx, "smf-pristine");
    let (handle, _) = open(cx, Project::new());
    run(cx, handle, file::ImportSmf);
    answer_open(cx, sample_mid());
    assert!(
        !cx.update(|cx| cx.windows().contains(&handle)),
        "빈 창이 남았다"
    );
    assert_eq!(windows(cx).len(), 1);
}

#[gpui_kit::test]
fn import_smf_save_as_suggests_file_stem(cx: &mut TestAppContext) {
    let dir = start(cx, "smf-save-as");
    let (handle, _) = open(cx, Project::new());
    run(cx, handle, file::ImportSmf);
    answer_open(cx, sample_mid());
    let imported = windows(cx).pop().unwrap();
    let h = cx.read(|cx| imported.read(cx).window_handle());
    run(cx, h, file::Save);
    assert!(cx.did_prompt_for_new_path());
    let target = dir.join("alouette.ksq");
    answer_new_path(cx, target.clone());
    assert!(target.exists());
    assert!(!dirty(cx, &imported));
}

#[gpui_kit::test]
fn import_smf_errors_show_sheet(cx: &mut TestAppContext) {
    let dir = start(cx, "smf-bad");
    let bad = dir.join("broken.mid");
    std::fs::write(&bad, b"MThd not really").unwrap();
    let (handle, _) = open(cx, sample());
    run(cx, handle, file::ImportSmf);
    answer_open(cx, bad);
    assert!(
        prompt_text(cx).contains("가져오지 못했다"),
        "{}",
        prompt_text(cx)
    );
    answer(cx, "확인");
    assert_eq!(windows(cx).len(), 1);
}

#[gpui_kit::test]
fn import_smf_too_long_message_names_the_limit(cx: &mut TestAppContext) {
    let dir = start(cx, "smf-long");
    // PPQ 1, 노트 끝이 2^32 틱 뒤(PPQ 960으로 재배율하면 상한을 넘는다).
    let mut body = Vec::new();
    body.extend([0x00, 0x90, 60, 100]);
    body.extend([0x8F, 0xFF, 0xFF, 0x7F, 0x80, 60, 0]);
    body.extend([0x00, 0xFF, 0x2F, 0x00]);
    let mut bytes = b"MThd".to_vec();
    bytes.extend(6u32.to_be_bytes());
    bytes.extend([0, 0, 0, 1, 0, 1]);
    bytes.extend(b"MTrk");
    bytes.extend(u32::try_from(body.len()).unwrap().to_be_bytes());
    bytes.extend(body);
    let long = dir.join("long.mid");
    std::fs::write(&long, bytes).unwrap();
    let (handle, _) = open(cx, sample());
    run(cx, handle, file::ImportSmf);
    answer_open(cx, long);
    let text = prompt_text(cx);
    assert!(
        text.contains("너무 길다") && text.contains("시간"),
        "{text}"
    );
    answer(cx, "확인");
}

#[gpui_kit::test]
fn export_smf_writes_type1_and_summarizes_losses(cx: &mut TestAppContext) {
    let dir = start(cx, "smf-export");
    let (handle, view) = open(cx, sample());
    run(cx, handle, track::New); // 노트 없는 트랙(다른 프로그램에서 사라질 수 있다)
    run(cx, handle, file::ExportSmf);
    let path = dir.join("out.mid");
    answer_new_path(cx, path.clone());
    let file = SmfFile::read(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(file.format(), keyscore_midi::SmfFormat::Parallel);
    let back = Song::from_smf(&file).unwrap().to_project().unwrap();
    assert_eq!(note_count(&back), 1);
    let msgs = messages(cx, &view);
    assert!(
        msgs.iter()
            .any(|m| m.contains("내보냄") && m.contains("out.mid")),
        "{msgs:?}"
    );
    assert!(
        msgs.iter().any(|m| m.contains("트랙 1개")),
        "빈 트랙 손실이 요약에 없다: {msgs:?}"
    );
    // 내보내기는 문서를 바꾸지 않는다.
    assert!(cx.read(|cx| view.read(cx).document().read(cx).path().is_none()));
}

// ---------- 계이름 텍스트 ----------

fn write_text(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

#[gpui_kit::test]
fn import_text_at_start_puts_overlapping_block_on_new_track(cx: &mut TestAppContext) {
    let dir = start(cx, "text-import");
    let (handle, view) = open(cx, sample());
    let path = write_text(
        &dir,
        "tune.txt",
        "@track Lead ch1\no4 l4 c d e f\n@track Bass ch2\no2 l1 c\n",
    );
    run(cx, handle, file::ImportText);
    answer_open(cx, path);
    let p = project_of(cx, &view);
    // 정지 상태라 처음에 놓는다. Lead의 기존 클립(0–3840)과 겹치므로 「Lead 2」 새 트랙이다.
    let names: Vec<&str> = p.tracks().iter().map(keyscore_core::Track::name).collect();
    assert_eq!(names, vec!["Lead", "Lead 2", "Bass"]);
    assert_eq!(p.tracks()[0].clips().count(), 1, "기존 트랙을 건드렸다");
    assert_eq!(note_count(&p), 1 + 4 + 1);
    assert!(dirty(cx, &view));
    let msgs = messages(cx, &view);
    assert!(
        msgs.iter()
            .any(|m| m.contains("가져옴") && m.contains("tune.txt")),
        "{msgs:?}"
    );
    // 가져오기는 undo 하나다.
    run(cx, handle, edit::Undo);
    assert_eq!(
        ksq::to_string(&project_of(cx, &view)),
        ksq::to_string(&sample())
    );
}

#[test]
fn import_text_into_uses_position_and_matches_track_names() {
    let mut doc = keyscore_app::document::ProjectDocument::new(sample()).unwrap();
    let parsed = keyscore_text::parse(
        "@track Lead ch1\no4 l4 c d e f\n@track Bass ch2\no2 l1 c\n",
        keyscore_text::Mode::Lenient,
    )
    .unwrap();
    let summary =
        keyscore_app::import_export::import_text_into(&mut doc, &parsed, Tick::new(7680)).unwrap();
    let p = doc.project();
    let names: Vec<&str> = p.tracks().iter().map(keyscore_core::Track::name).collect();
    assert_eq!(
        names,
        vec!["Lead", "Bass"],
        "겹치지 않으면 같은 이름 트랙에 넣는다"
    );
    let starts: Vec<u64> = p.tracks()[0]
        .clips()
        .map(|(_, c)| c.start().get())
        .collect();
    assert_eq!(starts, vec![0, 7680]);
    assert_eq!(p.tracks()[1].clips().next().unwrap().1.start().get(), 7680);
    assert_eq!(summary.new_tracks, 1);
    assert_eq!(summary.notes, 5);
    assert_eq!(doc.undo_len(), 1);
}

#[test]
fn import_text_into_names_new_tracks_uniquely() {
    let mut doc = keyscore_app::document::ProjectDocument::new(sample()).unwrap();
    // 「Lead 2」가 이미 있으면 「Lead 3」이다.
    let parsed =
        keyscore_text::parse("@track Lead ch1\nc\n", keyscore_text::Mode::Lenient).unwrap();
    keyscore_app::import_export::import_text_into(&mut doc, &parsed, Tick::ZERO).unwrap();
    keyscore_app::import_export::import_text_into(&mut doc, &parsed, Tick::ZERO).unwrap();
    let names: Vec<String> = doc
        .project()
        .tracks()
        .iter()
        .map(|t| t.name().to_owned())
        .collect();
    assert_eq!(names, vec!["Lead", "Lead 2", "Lead 3"]);
}

#[gpui_kit::test]
fn import_text_header_is_reported_not_applied(cx: &mut TestAppContext) {
    let dir = start(cx, "text-header");
    let (handle, view) = open(cx, sample());
    let path = write_text(&dir, "h.txt", "@tempo 90\n@track Pad ch3\nc1\n");
    run(cx, handle, file::ImportText);
    answer_open(cx, path);
    let p = project_of(cx, &view);
    assert_eq!(
        p.tempo_map().tempo_at(Tick::ZERO),
        sample().tempo_map().tempo_at(Tick::ZERO),
        "헤더 템포를 반영했다"
    );
    let msgs = messages(cx, &view);
    assert!(msgs.iter().any(|m| m.contains("@tempo")), "{msgs:?}");
}

#[gpui_kit::test]
fn import_text_syntax_error_shows_sheet_and_keeps_document(cx: &mut TestAppContext) {
    let dir = start(cx, "text-bad");
    let (handle, view) = open(cx, sample());
    let path = write_text(&dir, "bad.txt", "@track Lead ch1\nc d [e f\n");
    run(cx, handle, file::ImportText);
    answer_open(cx, path);
    let text = prompt_text(cx);
    assert!(
        text.contains("가져오지 못했다") && text.contains('2'),
        "{text}"
    );
    answer(cx, "확인");
    assert_eq!(
        ksq::to_string(&project_of(cx, &view)),
        ksq::to_string(&sample())
    );
    assert!(!dirty(cx, &view));
}

#[gpui_kit::test]
fn export_text_writes_file_and_summarizes_losses(cx: &mut TestAppContext) {
    let dir = start(cx, "text-export");
    // 벨로시티 40000은 텍스트 기본값(MIDI 1.0 100)이 아니라 잃는다.
    let quiet = ksq::from_str(
        r#"{"tracks": [{"name": "Lead", "channel": 1,
            "clips": [{"start": 0, "length": 3840, "notes": [[0, 480, 60, 40000]]}]}]}"#,
    )
    .unwrap()
    .0;
    let (handle, view) = open(cx, quiet);
    run(cx, handle, file::ExportText);
    let path = dir.join("out.txt");
    answer_new_path(cx, path.clone());
    let text = std::fs::read_to_string(&path).unwrap();
    let back = keyscore_text::parse(&text, keyscore_text::Mode::Lenient).unwrap();
    assert_eq!(back.tracks().len(), 1);
    assert_eq!(back.tracks()[0].clip().notes().len(), 1);
    let msgs = messages(cx, &view);
    assert!(
        msgs.iter()
            .any(|m| m.contains("내보냄") && m.contains("out.txt")),
        "{msgs:?}"
    );
    assert!(msgs.iter().any(|m| m.contains("벨로시티")), "{msgs:?}");
    assert!(!dirty(cx, &view));
}

// ---------- 공통 ----------

#[gpui_kit::test]
fn import_export_respect_file_flow(cx: &mut TestAppContext) {
    start(cx, "flow");
    let (handle, _) = open(cx, sample());
    run(cx, handle, file::ExportText);
    assert!(cx.did_prompt_for_new_path());
    // 대화상자가 떠 있는 동안 다른 흐름은 시작하지 않는다.
    run(cx, handle, file::ImportSmf);
    assert!(!cx.did_prompt_for_paths(), "흐름이 겹쳤다");
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    run(cx, handle, file::ImportSmf);
    assert!(cx.did_prompt_for_paths());
}

#[gpui_kit::test]
fn menu_and_palette_run_import_export(cx: &mut TestAppContext) {
    start(cx, "menus");
    let (handle, _) = open(cx, sample());
    let menus = cx.update(|cx| keyscore_app::commands::menus(cx));
    let file_menu = menus.iter().find(|m| m.name == "파일").expect("파일 메뉴");
    for name in [
        "file::ImportSmf",
        "file::ExportSmf",
        "file::ImportText",
        "file::ExportText",
    ] {
        let found = file_menu.items.iter().any(|item| match item {
            gpui_kit::MenuItem::Action { action, .. } => action.name() == name,
            _ => false,
        });
        assert!(found, "{name}이 파일 메뉴에서 실행할 수 있는 항목이 아니다");
    }
    // 팔레트에도 실행할 수 있는 줄로 있다(UI-11).
    let palette = cx.update(|cx| keyscore_app::commands::palette_entries(cx));
    for name in [
        "file::ImportSmf",
        "file::ExportSmf",
        "file::ImportText",
        "file::ExportText",
    ] {
        assert!(
            palette
                .iter()
                .any(|e| e.name() == name && e.action().is_some()),
            "{name}을 팔레트에서 실행할 수 없다"
        );
    }
    // 팔레트로 실행하면 대화상자가 뜬다.
    let action = palette
        .iter()
        .find(|e| e.name() == "file::ImportText")
        .and_then(|e| e.action())
        .unwrap()
        .boxed_clone();
    cx.update_window(handle, |_, window, cx| {
        keyscore_app::dispatch::run_action(action, window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        cx.did_prompt_for_paths(),
        "팔레트 실행이 가져오기를 시작하지 않았다"
    );
}
