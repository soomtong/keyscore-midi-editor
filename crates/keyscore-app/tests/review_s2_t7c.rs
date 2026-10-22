//! S2-T7c 적대적 리뷰 1회차 시험. 깨뜨리지 못한 시도만 남긴다. 깨진 재현은
//! `docs/reports/s2-t7c/review-1.md` 부록에 있다.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::{edit, file, track};
use keyscore_app::autosave::{self, AUTOSAVE_DELAY};
use keyscore_app::document::ProjectDocument;
use keyscore_app::files;
use keyscore_app::import_export::import_text_into;
use keyscore_app::window::{ProjectWindow, open_project_windows};
use keyscore_app::{init, open_project_window};
use keyscore_core::{Project, Tick, ksq};
use keyscore_text::Mode;

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rv-t7c-{}-{name}", std::process::id()));
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

fn messages(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<String> {
    cx.read(|cx| {
        view.read(cx)
            .messages()
            .iter()
            .map(ToString::to_string)
            .collect()
    })
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

// ---------- 텍스트 가져오기: undo·불변식 ----------

/// 텍스트 가져오기는 undo 하나, redo 하나로 같은 결과로 돌아온다.
#[gpui_kit::test]
fn import_text_undo_redo_round_trips(cx: &mut TestAppContext) {
    let dir = start(cx, "undo-redo");
    let (handle, view) = open(cx, sample());
    let path = write_text(
        &dir,
        "t.txt",
        "@track Lead ch1\nc d e\n@track Bass ch2\no2 c1\n@track Pad ch3\n[c e g]1\n",
    );
    run(cx, handle, file::ImportText);
    answer_open(cx, path);
    let after = text_of(cx, &view);
    run(cx, handle, edit::Undo);
    assert_eq!(text_of(cx, &view), ksq::to_string(&sample()));
    run(cx, handle, edit::Redo);
    assert_eq!(text_of(cx, &view), after);
}

/// 빈 텍스트(블록 없음)는 문서를 바꾸지 않고 「바뀜」도 켜지 않는다.
#[gpui_kit::test]
fn import_empty_text_keeps_document_clean(cx: &mut TestAppContext) {
    let dir = start(cx, "empty");
    let (handle, view) = open(cx, sample());
    let path = write_text(&dir, "empty.txt", "// 주석만\n");
    run(cx, handle, file::ImportText);
    answer_open(cx, path);
    assert_eq!(text_of(cx, &view), ksq::to_string(&sample()));
    assert!(!dirty(cx, &view));
}

/// 같은 이름 블록 여럿은 파서가 클립 하나로 합친다. 같은 트랙에 겹치는 클립 둘이 생기지 않는다.
#[test]
fn import_text_same_name_blocks_become_one_clip() {
    let mut doc = ProjectDocument::new(Project::new()).unwrap();
    let parsed =
        keyscore_text::parse("@track A ch1\nc d\n@track A ch1\ne f g\n", Mode::Lenient).unwrap();
    let summary = import_text_into(&mut doc, &parsed, Tick::ZERO).unwrap();
    assert_eq!(summary.blocks, 1);
    assert_eq!(doc.project().tracks().len(), 1);
    assert_eq!(doc.project().tracks()[0].clips().count(), 1);
}

/// 기존 트랙 「A」·「A 2」가 모두 겹치면 새 트랙은 「A 3」이다(텍스트 이름에는 공백이 없어 「A 2」 블록은 쓸 수 없다).
#[test]
fn import_text_new_names_do_not_collide_with_each_other() {
    let base = ksq::from_str(
        r#"{"tracks": [
            {"name": "A", "channel": 1, "clips": [{"start": 0, "length": 3840, "notes": []}]},
            {"name": "A 2", "channel": 1, "clips": [{"start": 0, "length": 3840, "notes": []}]}]}"#,
    )
    .unwrap()
    .0;
    let mut doc = ProjectDocument::new(base).unwrap();
    let parsed = keyscore_text::parse("@track A ch1\nc\n@track B ch1\nd\n", Mode::Lenient).unwrap();
    import_text_into(&mut doc, &parsed, Tick::ZERO).unwrap();
    let mut names: Vec<String> = doc
        .project()
        .tracks()
        .iter()
        .map(|t| t.name().to_owned())
        .collect();
    let n = names.len();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), n, "이름이 겹친다: {names:?}");
    assert_eq!(names, vec!["A", "A 2", "A 3", "B"]);
}

/// 넣을 자리가 틱 상한을 넘으면 오류이고 문서·undo는 그대로다.
#[test]
fn import_text_at_tick_limit_fails_atomically() {
    let mut doc = ProjectDocument::new(sample()).unwrap();
    let before = ksq::to_string(doc.project());
    let parsed =
        keyscore_text::parse("@track New ch2\nc\n@track Lead ch1\nc\n", Mode::Lenient).unwrap();
    let result = import_text_into(&mut doc, &parsed, Tick::new(u64::MAX - 10));
    assert!(result.is_err());
    assert_eq!(ksq::to_string(doc.project()), before);
    assert_eq!(doc.undo_len(), 0);
    assert!(!doc.is_dirty());
}

/// 겹침 판정의 경계: 기존 클립 끝(3840)에서 바로 시작하면 같은 트랙에 넣는다.
#[test]
fn import_text_adjacent_clip_goes_to_same_track() {
    let mut doc = ProjectDocument::new(sample()).unwrap();
    let parsed = keyscore_text::parse("@track Lead ch1\nc\n", Mode::Lenient).unwrap();
    import_text_into(&mut doc, &parsed, Tick::new(3840)).unwrap();
    assert_eq!(doc.project().tracks().len(), 1);
    assert_eq!(doc.project().tracks()[0].clips().count(), 2);
    // 한 틱 앞이면 겹치므로 새 트랙.
    let mut doc = ProjectDocument::new(sample()).unwrap();
    import_text_into(&mut doc, &parsed, Tick::new(3839)).unwrap();
    assert_eq!(doc.project().tracks().len(), 2);
}

// ---------- SMF 가져오기·창 ----------

/// 손대지 않은 빈 창이 둘이면 흐름을 띄운 창만 닫는다.
#[gpui_kit::test]
fn import_smf_closes_only_the_requesting_pristine_window(cx: &mut TestAppContext) {
    start(cx, "two-pristine");
    let (_other, _) = open(cx, Project::new());
    let (handle, _) = open(cx, Project::new());
    run(cx, handle, file::ImportSmf);
    answer_open(cx, sample_mid());
    assert!(!cx.update(|cx| cx.windows().contains(&handle)));
    assert_eq!(windows(cx).len(), 2);
}

/// 가져오기 실패(깨진 파일)면 빈 창을 닫지 않는다.
#[gpui_kit::test]
fn import_smf_failure_keeps_pristine_window(cx: &mut TestAppContext) {
    let dir = start(cx, "fail-pristine");
    let bad = dir.join("bad.mid");
    std::fs::write(&bad, b"not midi").unwrap();
    let (handle, _) = open(cx, Project::new());
    run(cx, handle, file::ImportSmf);
    answer_open(cx, bad);
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("확인");
    cx.run_until_parked();
    assert!(cx.update(|cx| cx.windows().contains(&handle)));
}

/// 가져온 창을 편집하면 자동 저장본이 생긴다(편집 뒤에는 관찰자가 돈다).
#[gpui_kit::test]
fn imported_smf_window_autosaves_after_edit(cx: &mut TestAppContext) {
    start(cx, "autosave-edit");
    let (handle, _) = open(cx, Project::new());
    run(cx, handle, file::ImportSmf);
    answer_open(cx, sample_mid());
    let imported = windows(cx).pop().unwrap();
    let h = cx.read(|cx| imported.read(cx).window_handle());
    run(cx, h, track::New);
    cx.executor()
        .advance_clock(AUTOSAVE_DELAY + Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(autosaves(cx), 1);
}

// ---------- 내보내기 ----------

/// 0444 파일에는 내보내지 않는다(결정 2026-10-21 1 A). 파일·문서 그대로.
#[gpui_kit::test]
fn export_to_read_only_file_is_refused(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = start(cx, "export-0444");
    let target = dir.join("locked.mid");
    std::fs::write(&target, b"keep").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o444)).unwrap();
    let (handle, view) = open(cx, sample());
    run(cx, handle, file::ExportSmf);
    answer_new_path(cx, target.clone());
    assert!(cx.has_pending_prompt(), "거부 시트가 없다");
    cx.simulate_prompt_answer("확인");
    cx.run_until_parked();
    assert_eq!(std::fs::read(&target).unwrap(), b"keep");
    assert!(!dirty(cx, &view));
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o644)).unwrap();
}

/// 내보내기는 문서의 경로·바뀜을 바꾸지 않고, 원래 `.ksq`도 건드리지 않는다.
#[gpui_kit::test]
fn export_keeps_saved_document_and_its_file(cx: &mut TestAppContext) {
    let dir = start(cx, "export-keep");
    let path = dir.join("song.ksq");
    std::fs::write(&path, ksq::to_string(&sample())).unwrap();
    let before = std::fs::read(&path).unwrap();
    let (handle, view) = open_file(cx, &path);
    run(cx, handle, file::ExportText);
    answer_new_path(cx, dir.join("song.txt"));
    run(cx, handle, file::ExportSmf);
    answer_new_path(cx, dir.join("song.mid"));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!dirty(cx, &view));
    let doc_path = cx.read(|cx| {
        view.read(cx)
            .document()
            .read(cx)
            .path()
            .map(Path::to_path_buf)
    });
    assert_eq!(doc_path.as_deref(), Some(path.as_path()));
    assert!(dir.join("song.txt").exists() && dir.join("song.mid").exists());
    // 임시 파일이 남지 않는다.
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp-"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    let msgs = messages(cx, &view);
    assert!(msgs.iter().any(|m| m.contains("song.mid")), "{msgs:?}");
}

/// 문법 오류가 아닌 느슨한 경고는 요약에 수로 나온다.
#[gpui_kit::test]
fn import_text_lenient_warnings_are_summarized(cx: &mut TestAppContext) {
    let dir = start(cx, "warn");
    let (handle, view) = open(cx, sample());
    let path = write_text(&dir, "w.txt", "@track Pad ch3\nc ? d\n");
    run(cx, handle, file::ImportText);
    answer_open(cx, path);
    if cx.has_pending_prompt() {
        // 느슨한 모드도 이 토큰을 오류로 볼 수 있다. 그때는 문서가 그대로여야 한다.
        cx.simulate_prompt_answer("확인");
        cx.run_until_parked();
        assert_eq!(text_of(cx, &view), ksq::to_string(&sample()));
    } else {
        let msgs = messages(cx, &view);
        assert!(msgs.iter().any(|m| m.contains("경고")), "{msgs:?}");
    }
}

/// 백그라운드 가져오기 중에 흐름을 띄운 창을 닫아도 멈추거나 죽지 않고, 가져온 창은 열린다.
#[gpui_kit::test]
fn closing_requesting_window_during_import_still_opens_result(cx: &mut TestAppContext) {
    start(cx, "close-during");
    let (handle, _) = open(cx, sample());
    run(cx, handle, file::ImportSmf);
    assert!(cx.did_prompt_for_paths());
    let p = sample_mid();
    cx.simulate_path_prompt_response(move |_| Some(vec![p]));
    // 대화상자 답 직후, 백그라운드가 끝나기 전에 창을 닫는다.
    let _ = cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(keyscore_app::actions::window::Close), cx);
    });
    cx.run_until_parked();
    assert!(!cx.has_pending_prompt());
    assert!(!cx.update(|cx| cx.windows().contains(&handle)));
    let all = windows(cx);
    assert_eq!(all.len(), 1);
    assert!(dirty(cx, &all[0]), "가져온 창이 아니다");
}

// ---------- 리뷰 1 부록 A(수정 확인) ----------

/// 지적 1: 상위 형식 버전 원본(읽기 전용)에 내보내면 원본이 텍스트로 덮인다.
#[gpui_kit::test]
fn export_over_newer_format_original_keeps_it(cx: &mut TestAppContext) {
    let dir = start(cx, "x-newer");
    let path = dir.join("future.ksq");
    std::fs::write(
        &path,
        r#"{"format_version": 7, "future": {"x": 1}, "tracks": []}"#,
    )
    .unwrap();
    let before = std::fs::read(&path).unwrap();
    let (handle, _view) = open_file(cx, &path);
    run(cx, handle, file::ExportText);
    answer_new_path(cx, path.clone());
    if cx.has_pending_prompt() {
        cx.simulate_prompt_answer("확인");
        cx.run_until_parked();
    }
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "상위 버전 원본이 내보내기로 덮였다"
    );
}

/// 지적 1: 다른 창이 연 `.ksq`에 내보내면 그 파일이 MIDI로 덮이고, 그 창은 깨끗한 채라 닫아도 묻지 않는다.
#[gpui_kit::test]
fn export_over_file_open_in_other_window_is_refused(cx: &mut TestAppContext) {
    let dir = start(cx, "x-other");
    let a = dir.join("a.ksq");
    std::fs::write(&a, ksq::to_string(&sample())).unwrap();
    let before = std::fs::read(&a).unwrap();
    let (_ha, va) = open_file(cx, &a);
    let (hb, _) = open(cx, Project::new());
    run(cx, hb, file::ExportSmf);
    answer_new_path(cx, a.clone());
    if cx.has_pending_prompt() {
        cx.simulate_prompt_answer("확인");
        cx.run_until_parked();
    }
    let a_dirty = dirty(cx, &va);
    assert!(
        std::fs::read(&a).unwrap() == before || a_dirty,
        "다른 창이 연 a.ksq를 MIDI로 덮었는데 그 창은 깨끗하다"
    );
}

/// 지적 2: 가져와 저장 안 된 창(바뀜 켜짐)인데 편집하기 전에는 자동 저장본이 생기지 않는다.
#[gpui_kit::test]
fn imported_smf_window_is_autosaved(cx: &mut TestAppContext) {
    start(cx, "x-autosave");
    let (handle, _) = open(cx, Project::new());
    run(cx, handle, file::ImportSmf);
    answer_open(cx, sample_mid());
    let imported = windows(cx).pop().unwrap();
    assert!(dirty(cx, &imported));
    cx.executor()
        .advance_clock(AUTOSAVE_DELAY + Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(
        autosaves(cx),
        1,
        "가져와 저장 안 된 창의 자동 저장본이 없다"
    );
}

/// 지적 4: 같은 이름 트랙의 채널·프로그램이 블록과 다르면 블록의 ch·prog가 소리 없이 사라진다.
#[test]
fn import_text_into_same_name_track_keeps_block_channel() {
    let mut doc = ProjectDocument::new(sample()).unwrap(); // Lead ch1, 프로그램 없음
    let parsed = keyscore_text::parse("@track Lead ch5 prog40\nc\n", Mode::Lenient).unwrap();
    import_text_into(&mut doc, &parsed, Tick::new(7680)).unwrap();
    let p = doc.project();
    let placed = p
        .tracks()
        .iter()
        .find(|t| t.clips().any(|(_, c)| c.start().get() == 7680))
        .unwrap();
    assert_eq!(
        (placed.channel().get(), placed.program().map(keyscore_core::Program::get)),
        (5, Some(40)),
        "블록의 ch5 prog40이 ch1 트랙에 들어가 바뀌었다"
    );
}
