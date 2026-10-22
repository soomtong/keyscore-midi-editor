//! S2-T7c 적대적 리뷰 2회차 시험. 깨뜨리지 못한 시도만 남긴다. 깨진 재현은
//! `docs/reports/s2-t7c/review-2.md` 부록에 있다.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{edit, file};
use keyscore_app::autosave::{self, AUTOSAVE_DELAY};
use keyscore_app::document::ProjectDocument;
use keyscore_app::files;
use keyscore_app::import_export::import_text_into;
use keyscore_app::window::{ProjectWindow, open_project_windows};
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, Tick, ksq};
use keyscore_text::Mode;

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rv-t7c2-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
}

fn activate(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| {
        window.activate_window();
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn open(cx: &mut TestAppContext, p: Project) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| open_project_window(p, cx)).unwrap();
    activate(cx, handle);
    (handle, view)
}

fn open_file(cx: &mut TestAppContext, path: &Path) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| files::open_path(path, cx)).unwrap();
    activate(cx, handle);
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

fn windows(cx: &mut TestAppContext) -> Vec<Entity<ProjectWindow>> {
    let open = cx.update(|cx| cx.windows());
    cx.update(|cx| open_project_windows(cx))
        .into_iter()
        .filter(|w| open.contains(&cx.read(|cx| w.read(cx).window_handle())))
        .collect()
}

fn text_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> String {
    cx.read(|cx| ksq::to_string(view.read(cx).document().read(cx).project()))
}

fn dirty(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).document().read(cx).is_dirty())
}

fn sample() -> Project {
    ksq::from_str(
        r#"{"tracks": [{"name": "Lead", "channel": 1,
            "clips": [{"start": 0, "length": 3840, "notes": [[0, 480, 60, 51492]]}]}]}"#,
    )
    .unwrap()
    .0
}

fn sample_mid() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../keyscore-midi/tests/data/smf/alouette.mid")
}

fn write_text(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

fn autosaves(cx: &mut TestAppContext) -> usize {
    let dir = cx.read(autosave::autosave_dir);
    std::fs::read_dir(dir).map_or(0, |entries| {
        entries
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "ksq"))
            .count()
    })
}

// ---------- 리뷰 1 지적 1 회귀: 열린 파일 거부 ----------

/// 거부 시트에 답한다. 있어야 한다.
fn dismiss_refusal(cx: &mut TestAppContext) {
    assert!(cx.has_pending_prompt(), "거부 시트가 없다");
    cx.simulate_prompt_answer("확인");
    cx.run_until_parked();
}

/// 이 창 자신의 `.ksq`에 내보내면 거부한다. 거부 뒤 흐름이 풀려 다른 경로로는 내보낸다.
#[gpui_kit::test]
fn export_to_own_file_is_refused_then_flow_continues(cx: &mut TestAppContext) {
    let dir = start(cx, "own");
    let path = dir.join("own.ksq");
    std::fs::write(&path, ksq::to_string(&sample())).unwrap();
    let before = std::fs::read(&path).unwrap();
    let (handle, view) = open_file(cx, &path);
    run(cx, handle, file::ExportSmf);
    answer_new_path(cx, path.clone());
    dismiss_refusal(cx);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    run(cx, handle, file::ExportText);
    answer_new_path(cx, path.clone());
    dismiss_refusal(cx);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!dirty(cx, &view));
    // 흐름이 막히지 않았다.
    run(cx, handle, file::ExportSmf);
    answer_new_path(cx, dir.join("ok.mid"));
    assert!(!cx.has_pending_prompt());
    assert!(dir.join("ok.mid").exists());
}

/// 열린 파일의 심볼릭 링크·하드 링크로 골라도 거부한다.
#[gpui_kit::test]
fn export_through_link_to_open_file_is_refused(cx: &mut TestAppContext) {
    let dir = start(cx, "links");
    let path = dir.join("a.ksq");
    std::fs::write(&path, ksq::to_string(&sample())).unwrap();
    let before = std::fs::read(&path).unwrap();
    let soft = dir.join("soft.mid");
    std::os::unix::fs::symlink(&path, &soft).unwrap();
    let hard = dir.join("hard.txt");
    std::fs::hard_link(&path, &hard).unwrap();
    let (_ha, _) = open_file(cx, &path);
    let (hb, _) = open(cx, Project::new());
    run(cx, hb, file::ExportSmf);
    answer_new_path(cx, soft);
    dismiss_refusal(cx);
    run(cx, hb, file::ExportText);
    answer_new_path(cx, hard);
    dismiss_refusal(cx);
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

/// 열고 나서 바깥에서 옮긴 파일(새 이름)에 내보내도 거부한다(기억한 파일 정체).
#[gpui_kit::test]
fn export_to_externally_moved_open_file_is_refused(cx: &mut TestAppContext) {
    let dir = start(cx, "moved");
    let path = dir.join("a.ksq");
    std::fs::write(&path, ksq::to_string(&sample())).unwrap();
    let (_ha, _) = open_file(cx, &path);
    let moved = dir.join("b.ksq");
    std::fs::rename(&path, &moved).unwrap();
    let before = std::fs::read(&moved).unwrap();
    let (hb, _) = open(cx, Project::new());
    run(cx, hb, file::ExportText);
    answer_new_path(cx, moved.clone());
    dismiss_refusal(cx);
    assert_eq!(std::fs::read(&moved).unwrap(), before);
}

/// 닫힌 창의 파일, 열리지 않은 `.ksq`는 거부하지 않는다(덮어쓰기 확인은 OS 대화상자 몫).
#[gpui_kit::test]
fn export_to_closed_project_file_is_allowed(cx: &mut TestAppContext) {
    let dir = start(cx, "closed");
    let path = dir.join("a.ksq");
    std::fs::write(&path, ksq::to_string(&sample())).unwrap();
    let (ha, _) = open_file(cx, &path);
    let _ = cx.update_window(ha, |_, window, cx| {
        window.dispatch_action(Box::new(keyscore_app::actions::window::Close), cx);
    });
    cx.run_until_parked();
    let (hb, _) = open(cx, sample());
    run(cx, hb, file::ExportText);
    answer_new_path(cx, path.clone());
    assert!(!cx.has_pending_prompt());
    assert!(std::fs::read_to_string(&path).unwrap().contains("@track"));
}

/// 가져온(경로 없는) 창은 어떤 경로도 막지 않는다. 가져온 창에서 바로 내보낸다.
#[gpui_kit::test]
fn imported_window_can_export(cx: &mut TestAppContext) {
    let dir = start(cx, "imp-export");
    let (handle, _) = open(cx, Project::new());
    run(cx, handle, file::ImportSmf);
    answer_open(cx, sample_mid());
    let imported = windows(cx).pop().unwrap();
    let h = cx.read(|cx| imported.read(cx).window_handle());
    run(cx, h, file::ExportSmf);
    answer_new_path(cx, dir.join("again.mid"));
    assert!(!cx.has_pending_prompt());
    assert!(dir.join("again.mid").exists());
}

// ---------- 리뷰 1 지적 2 회귀: 가져온 창 자동 저장 ----------

/// 가져온 창은 편집 없이 자동 저장되고, 그 창을 버리고 닫으면 저장본도 지운다.
#[gpui_kit::test]
fn imported_window_autosave_is_removed_on_discard(cx: &mut TestAppContext) {
    start(cx, "imp-discard");
    let (handle, _) = open(cx, Project::new());
    run(cx, handle, file::ImportSmf);
    answer_open(cx, sample_mid());
    let imported = windows(cx).pop().unwrap();
    cx.executor()
        .advance_clock(AUTOSAVE_DELAY + Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(autosaves(cx), 1);
    let h = cx.read(|cx| imported.read(cx).window_handle());
    let _ = cx.update_window(h, |_, window, cx| {
        window.dispatch_action(Box::new(keyscore_app::actions::window::Close), cx);
    });
    cx.run_until_parked();
    assert!(
        cx.has_pending_prompt(),
        "저장 안 된 가져온 창인데 묻지 않는다"
    );
    cx.simulate_prompt_answer("저장 안 함");
    cx.run_until_parked();
    assert_eq!(autosaves(cx), 0);
}

// ---------- 리뷰 1 지적 4 회귀: 블록 채널·프로그램 ----------

fn lead(channel: u8, program: Option<u8>, bank: Option<u16>) -> Project {
    let prog = program.map_or(String::new(), |p| format!(r#", "program": {p}"#));
    let bank = bank.map_or(String::new(), |b| format!(r#", "bank": {b}"#));
    ksq::from_str(&format!(
        r#"{{"tracks": [{{"name": "Lead", "channel": {channel}{prog}{bank},
            "clips": [{{"start": 0, "length": 3840, "notes": [[0, 480, 60, 51492]]}}]}}]}}"#
    ))
    .unwrap()
    .0
}

/// 우리가 내보낸 텍스트(ch·prog·bank를 적는다)를 다시 가져오면 같은 트랙에 들어간다. 새 트랙이 없다.
#[test]
fn reimport_of_own_text_goes_to_same_track() {
    for (ch, prog, bank) in [
        (1, None, None),
        (5, Some(40), None),
        (10, Some(0), Some(300)),
    ] {
        let p = lead(ch, prog, bank);
        let (text, losses) = keyscore_text::project::project_to_text(&p).unwrap();
        assert!(losses.is_empty(), "{losses:?}");
        let parsed = keyscore_text::parse(&text, Mode::Lenient).unwrap();
        let mut doc = ProjectDocument::new(p).unwrap();
        let summary = import_text_into(&mut doc, &parsed, Tick::new(3840)).unwrap();
        assert_eq!(summary.new_tracks, 0, "ch{ch} {prog:?} {bank:?}: {text:?}");
        assert_eq!(doc.project().tracks()[0].clips().count(), 2);
    }
}

/// 블록이 프로그램을 적지 않으면 트랙의 프로그램을 따른다(같은 트랙). 다른 프로그램을 적으면 새 트랙에
/// 그 프로그램으로 들어가고, undo 하나로 새 트랙과 클립이 함께 사라진다.
#[test]
fn block_program_decides_track_and_undo_is_one() {
    let mut doc = ProjectDocument::new(lead(5, Some(40), None)).unwrap();
    let before = ksq::to_string(doc.project());
    let parsed = keyscore_text::parse("@track Lead ch5\nc\n", Mode::Lenient).unwrap();
    let s = import_text_into(&mut doc, &parsed, Tick::new(3840)).unwrap();
    assert_eq!(s.new_tracks, 0);
    let mut doc = ProjectDocument::new(lead(5, Some(40), None)).unwrap();
    let parsed = keyscore_text::parse("@track Lead ch5 prog41\nc\n", Mode::Lenient).unwrap();
    let s = import_text_into(&mut doc, &parsed, Tick::new(3840)).unwrap();
    assert_eq!(s.new_tracks, 1);
    let t = &doc.project().tracks()[1];
    assert_eq!(t.name(), "Lead 2");
    assert_eq!(
        (
            t.channel().get(),
            t.program().map(keyscore_core::Program::get)
        ),
        (5, Some(41))
    );
    assert_eq!(doc.undo_len(), 1);
    doc.undo().unwrap();
    assert_eq!(ksq::to_string(doc.project()), before);
}

/// 같은 트랙의 다른 뱅크도 새 트랙으로 간다.
#[test]
fn block_bank_mismatch_goes_to_new_track() {
    let mut doc = ProjectDocument::new(lead(1, Some(0), Some(1))).unwrap();
    let parsed = keyscore_text::parse("@track Lead ch1 prog0 bank2\nc\n", Mode::Lenient).unwrap();
    let s = import_text_into(&mut doc, &parsed, Tick::new(3840)).unwrap();
    assert_eq!(s.new_tracks, 1);
    assert_eq!(
        doc.project().tracks()[1]
            .bank()
            .map(keyscore_core::Bank::get),
        Some(2)
    );
}

/// 앱 흐름: 새 트랙으로 간 가져오기도 undo 한 번이면 처음 내용으로 돌아온다(바뀜은 undo 뒤에도 켜진 채라는
/// 기존 문서 규칙이라 보지 않는다).
#[gpui_kit::test]
fn import_text_into_new_track_undoes_to_clean(cx: &mut TestAppContext) {
    let dir = start(cx, "undo-clean");
    let path = dir.join("s.ksq");
    std::fs::write(&path, ksq::to_string(&lead(1, None, None))).unwrap();
    let (handle, view) = open_file(cx, &path);
    let before = text_of(cx, &view);
    let txt = write_text(&dir, "t.txt", "@track Lead ch7 prog3\nc d e\n");
    run(cx, handle, file::ImportText);
    answer_open(cx, txt);
    assert!(dirty(cx, &view));
    run(cx, handle, edit::Undo);
    assert_eq!(text_of(cx, &view), before);
}
