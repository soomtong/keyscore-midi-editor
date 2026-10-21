//! S2-T7a 적대적 리뷰 2회차(`docs/reports/s2-t7a/review-2.md`). 깨뜨리지 못한 시도만 남긴다. 깨뜨린 시도는
//! 리뷰 문서 부록에 있다.

use std::path::{Path, PathBuf};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, MenuItem, TestAppContext};
use keyscore_app::actions::{app, file, track, window as window_actions};
use keyscore_app::window::{ProjectWindow, open_project_windows};
use keyscore_app::{files, init, open_project_window};
use keyscore_core::{Project, ksq};

fn start(cx: &mut TestAppContext, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-review-t7a-r2-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    dir
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

fn activate(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn touch(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    run(cx, handle, track::New);
}

fn dirty(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> bool {
    cx.read(|cx| view.read(cx).document().read(cx).is_dirty())
}

fn doc_path(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Option<PathBuf> {
    cx.read(|cx| {
        view.read(cx)
            .document()
            .read(cx)
            .path()
            .map(Path::to_path_buf)
    })
}

fn is_open(cx: &mut TestAppContext, handle: AnyWindowHandle) -> bool {
    cx.update(|cx| cx.windows().contains(&handle))
}

fn answer(cx: &mut TestAppContext, button: &str) {
    assert!(cx.has_pending_prompt(), "시트가 없다");
    cx.simulate_prompt_answer(button);
    cx.run_until_parked();
}

fn answer_new_path(cx: &mut TestAppContext, path: Option<PathBuf>) {
    assert!(cx.did_prompt_for_new_path(), "새 경로 대화상자가 없다");
    cx.simulate_new_path_selection(move |_| path);
    cx.run_until_parked();
}

fn prompt_text(cx: &mut TestAppContext) -> String {
    let (msg, detail) = cx.pending_prompt().expect("시트가 없다");
    format!("{msg}\n{detail}")
}

fn start_quit(cx: &mut TestAppContext) -> std::rc::Rc<std::cell::Cell<Option<bool>>> {
    let result = std::rc::Rc::new(std::cell::Cell::new(None));
    let slot = result.clone();
    cx.update(|cx| {
        let task = files::confirm_close_all(cx);
        cx.spawn(async move |_| slot.set(Some(task.await))).detach();
    });
    cx.run_until_parked();
    result
}

fn sample() -> Project {
    ksq::from_str(
        r#"{"tracks": [{"name": "Lead", "channel": 1,
            "clips": [{"start": 0, "length": 3840, "notes": [[0, 480, 60, 51492]]}]}]}"#,
    )
    .unwrap()
    .0
}

fn write_sample(path: &Path) {
    std::fs::write(path, ksq::to_string(&sample())).unwrap();
}

const NEWER: &str = r#"{"format_version": 7, "future": {"x": 1}, "tracks": []}"#;

/// 상위 버전 파일을 열고 고친 창.
fn open_newer(
    cx: &mut TestAppContext,
    path: &Path,
) -> (AnyWindowHandle, Entity<ProjectWindow>, Vec<u8>) {
    std::fs::write(path, NEWER).unwrap();
    let before = std::fs::read(path).unwrap();
    let (handle, view) = open_file(cx, path);
    touch(cx, handle);
    (handle, view, before)
}

// ---------- 원본이 바뀌면 안 되는 경로(지적 1의 수정 확인) ----------

/// 연 뒤에 만든 심볼릭 링크로 원본을 가리켜 다른 이름으로 저장해도 원본은 그대로다.
#[gpui_kit::test]
fn review_r2_save_as_newer_original_via_symlink_made_after_open_is_refused(
    cx: &mut TestAppContext,
) {
    let dir = start(cx, "newer-symlink");
    let path = dir.join("future.ksq");
    let (handle, view, before) = open_newer(cx, &path);
    let link = dir.join("alias.ksq");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    run(cx, handle, file::SaveAs);
    answer_new_path(cx, Some(link.clone()));
    assert!(
        prompt_text(cx).contains("덮어쓸 수 없다"),
        "{}",
        prompt_text(cx)
    );
    answer(cx, "확인");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(dirty(cx, &view));
    assert!(!cx.has_pending_prompt());
}

/// 하드 링크 다른 이름으로 원본을 골라도 원본은 그대로다.
#[gpui_kit::test]
fn review_r2_save_as_newer_original_via_hard_link_is_refused(cx: &mut TestAppContext) {
    let dir = start(cx, "newer-hardlink");
    let path = dir.join("future.ksq");
    let (handle, _, before) = open_newer(cx, &path);
    let other = dir.join("other-name.ksq");
    std::fs::hard_link(&path, &other).unwrap();
    run(cx, handle, file::Save);
    answer(cx, "다른 이름으로 저장");
    answer_new_path(cx, Some(other.clone()));
    answer(cx, "확인");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read(&other).unwrap(), before);
}

/// 닫기 시트 [저장] → 상위 버전 시트 [다른 이름으로 저장] → 원본 경로: 쓰지 않고 창·바뀜이 남는다.
#[gpui_kit::test]
fn review_r2_close_save_newer_onto_original_keeps_window_and_original(cx: &mut TestAppContext) {
    let dir = start(cx, "newer-close");
    let path = dir.join("future.ksq");
    let (handle, view, before) = open_newer(cx, &path);
    run(cx, handle, window_actions::Close);
    answer(cx, "저장");
    answer(cx, "다른 이름으로 저장");
    answer_new_path(cx, Some(path.clone()));
    answer(cx, "확인");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(is_open(cx, handle));
    assert!(dirty(cx, &view));
    assert_flow_cleared(cx, handle);
}

/// 끝내기 [저장] → 상위 버전 → 원본 경로: 쓰지 않고 끝내지 않는다.
#[gpui_kit::test]
fn review_r2_quit_save_newer_onto_original_aborts(cx: &mut TestAppContext) {
    let dir = start(cx, "newer-quit");
    let path = dir.join("future.ksq");
    let (handle, _, before) = open_newer(cx, &path);
    let quit = start_quit(cx);
    answer(cx, "저장");
    answer(cx, "다른 이름으로 저장");
    answer_new_path(cx, Some(path.clone()));
    answer(cx, "확인");
    assert_eq!(quit.get(), Some(false));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(is_open(cx, handle));
}

/// 다른 (새) 창이 상위 버전 원본을 하드 링크 이름으로 골라 다른 이름으로 저장해도 거부한다.
#[gpui_kit::test]
fn review_r2_other_window_save_as_onto_newer_original_hard_link_is_refused(
    cx: &mut TestAppContext,
) {
    let dir = start(cx, "newer-other");
    let path = dir.join("future.ksq");
    let (_, _, before) = open_newer(cx, &path);
    let other = dir.join("other-name.ksq");
    std::fs::hard_link(&path, &other).unwrap();
    let (untitled, _) = open(cx, sample());
    run(cx, untitled, file::SaveAs);
    answer_new_path(cx, Some(other));
    assert!(prompt_text(cx).contains("이미"), "{}", prompt_text(cx));
    answer(cx, "확인");
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

// ---------- 끝내기 반복(지적 2의 수정 확인) ----------

/// `app::Quit` Action 경로: 바뀐 창 둘에 [저장 안 함] → 둘 다 닫히고 시트가 더 뜨지 않는다(재발송이 돌지 않는다).
#[gpui_kit::test]
fn review_r2_quit_action_all_dont_save_closes_all_without_extra_sheet(cx: &mut TestAppContext) {
    start(cx, "quit-action-all");
    let (a, _) = open(cx, sample());
    touch(cx, a);
    let (b, _) = open(cx, sample());
    touch(cx, b);
    run(cx, b, app::Quit);
    answer(cx, "저장 안 함");
    answer(cx, "저장 안 함");
    assert!(!cx.has_pending_prompt(), "끝내기가 다시 물었다");
    assert!(!is_open(cx, a) && !is_open(cx, b));
}

/// 끝내기 시트 중 `app::Quit`을 또 내도 같은 창에 시트가 겹치지 않고, 첫 끝내기는 이어진다.
#[gpui_kit::test]
fn review_r2_quit_action_twice_does_not_stack_sheets(cx: &mut TestAppContext) {
    start(cx, "quit-twice");
    let (a, a_view) = open(cx, sample());
    touch(cx, a);
    let (b, b_view) = open(cx, sample());
    touch(cx, b);
    run(cx, b, app::Quit);
    assert!(cx.has_pending_prompt());
    run(cx, b, app::Quit);
    // 첫 시트(창 a)에 답하면 둘째 시트(창 b)가 뜬다. 겹쳤다면 a의 시트가 두 번 줄에 선다.
    let first = prompt_text(cx);
    answer(cx, "저장 안 함");
    assert!(!is_open(cx, a));
    assert!(cx.has_pending_prompt());
    assert_ne!(prompt_text(cx), "", "{first}");
    answer(cx, "저장 안 함");
    assert!(!cx.has_pending_prompt());
    assert!(!is_open(cx, b));
    let _ = (a_view, b_view);
}

/// 바뀐 창에 다른 이름 저장 대화상자가 떠 있으면 끝내기는 묻지 않고 멈춘다. 대화상자는 그대로 답할 수 있다.
#[gpui_kit::test]
fn review_r2_quit_with_pending_save_as_dialog_aborts_and_dialog_still_works(
    cx: &mut TestAppContext,
) {
    let dir = start(cx, "quit-busy");
    let (a, a_view) = open(cx, sample());
    touch(cx, a);
    run(cx, a, file::SaveAs);
    let quit = start_quit(cx);
    assert_eq!(quit.get(), Some(false));
    assert!(!cx.has_pending_prompt());
    let target = dir.join("a.ksq");
    answer_new_path(cx, Some(target.clone()));
    assert!(target.exists());
    assert!(!dirty(cx, &a_view));
    let quit = start_quit(cx);
    assert_eq!(quit.get(), Some(true));
}

/// 끝내기 시트 중 셋째 창을 닫기 Action으로 닫으려 하면 그 창의 시트가 따로 뜨고, 끝내기는 그 창에서 멈춘다.
/// 묻지 않고 끝나는 경로는 없다.
#[gpui_kit::test]
fn review_r2_quit_while_other_window_close_sheet_never_quits_unasked(cx: &mut TestAppContext) {
    start(cx, "quit-close-mix");
    let (a, _) = open(cx, sample());
    touch(cx, a);
    let (b, b_view) = open(cx, sample());
    touch(cx, b);
    let quit = start_quit(cx);
    run(cx, b, window_actions::Close);
    answer(cx, "저장 안 함"); // 창 a(끝내기)
    assert_eq!(
        quit.get(),
        Some(false),
        "닫기 시트가 떠 있는 창 b를 건너뛰었다"
    );
    assert!(dirty(cx, &b_view));
    answer(cx, "취소"); // 창 b(닫기)
    assert!(is_open(cx, b));
    assert!(!cx.has_pending_prompt());
}

// ---------- 열기의 창 바꾸기(지적 3의 수정 확인) ----------

/// 다른 이름 저장 대화상자가 떠 있는 손대지 않은 창은 열기가 닫지 않고, 그 뒤 대화상자 답이 그 창에 반영된다.
#[gpui_kit::test]
fn review_r2_open_keeps_busy_pristine_window_and_its_dialog_completes(cx: &mut TestAppContext) {
    let dir = start(cx, "open-busy");
    let other = dir.join("other.ksq");
    write_sample(&other);
    let (handle, view) = open(cx, Project::new());
    run(cx, handle, file::SaveAs);
    run(cx, handle, file::Open);
    cx.simulate_path_prompt_response(move |_| Some(vec![other]));
    cx.run_until_parked();
    assert!(is_open(cx, handle));
    let target = dir.join("kept.ksq");
    answer_new_path(cx, Some(target.clone()));
    assert!(target.exists());
    assert_eq!(doc_path(cx, &view), Some(target.canonicalize().unwrap()));
    assert_flow_cleared(cx, handle);
    assert_eq!(cx.read(|cx| open_project_windows(cx).len()), 2);
}

// ---------- 같은 파일 두 창 ----------

/// 두 새 창이 차례로 같은 새 경로를 고르면 둘째는 거부된다.
#[gpui_kit::test]
fn review_r2_two_windows_save_as_same_new_path_second_refused(cx: &mut TestAppContext) {
    let dir = start(cx, "same-new");
    let (a, a_view) = open(cx, sample());
    let (b, b_view) = open(cx, Project::new());
    touch(cx, b);
    run(cx, a, file::SaveAs);
    run(cx, b, file::SaveAs);
    let target = dir.join("both.ksq");
    answer_new_path(cx, Some(target.clone()));
    let after_a = std::fs::read(&target).unwrap();
    answer_new_path(cx, Some(target.clone()));
    assert!(prompt_text(cx).contains("이미"), "{}", prompt_text(cx));
    answer(cx, "확인");
    assert_eq!(std::fs::read(&target).unwrap(), after_a);
    assert!(doc_path(cx, &a_view).is_some());
    assert!(doc_path(cx, &b_view).is_none());
}

/// 다른 이름 저장 대화상자가 떠 있는 사이 같은 파일을 다른 창에 열었다면, 그 경로를 고를 때 거부된다.
#[gpui_kit::test]
fn review_r2_save_as_onto_file_opened_meanwhile_is_refused(cx: &mut TestAppContext) {
    let dir = start(cx, "opened-meanwhile");
    let path = dir.join("song.ksq");
    write_sample(&path);
    let before = std::fs::read(&path).unwrap();
    let (a, _) = open(cx, Project::new());
    touch(cx, a);
    run(cx, a, file::SaveAs);
    let _ = open_file(cx, &path);
    answer_new_path(cx, Some(path.clone()));
    assert!(prompt_text(cx).contains("이미"), "{}", prompt_text(cx));
    answer(cx, "확인");
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

/// 하드 링크 판정(`(dev, ino)`)이 다른 파일을 같은 파일로 보지 않는다: 같은 내용의 복사본은 따로 열린다.
#[gpui_kit::test]
fn review_r2_copies_with_same_content_open_in_separate_windows(cx: &mut TestAppContext) {
    let dir = start(cx, "copies");
    let a = dir.join("a.ksq");
    let b = dir.join("b.ksq");
    write_sample(&a);
    std::fs::copy(&a, &b).unwrap();
    let (ha, _) = open_file(cx, &a);
    let (hb, _) = open_file(cx, &b);
    assert_ne!(ha, hb);
    assert_eq!(cx.read(files::recent_files).len(), 2);
}

// ---------- 최근 파일 ----------

/// 이름이 같은 셋(`x/proj`, `y/proj`, `x/other`)의 라벨이 서로 다르다.
#[gpui_kit::test]
fn review_r2_recent_labels_distinct_three_way(cx: &mut TestAppContext) {
    let dir = start(cx, "labels3");
    let paths = [
        dir.join("x/proj/song.ksq"),
        dir.join("y/proj/song.ksq"),
        dir.join("x/other/song.ksq"),
    ];
    for p in &paths {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        write_sample(p);
        cx.update(|cx| files::open_path(p, cx)).unwrap();
    }
    let labels: Vec<String> = cx.update(|cx| {
        keyscore_app::commands::palette_entries(cx)
            .iter()
            .filter(|e| e.name() == "file::OpenRecent")
            .map(|e| e.label().to_string())
            .collect()
    });
    assert_eq!(labels.len(), 3, "{labels:?}");
    for i in 0..3 {
        for j in (i + 1)..3 {
            assert_ne!(labels[i], labels[j], "{labels:?}");
        }
    }
}

/// 하드 링크 두 이름을 차례로 열어도 최근 목록에는 하나다(같은 창을 앞으로 가져온다).
#[gpui_kit::test]
fn review_r2_recent_hard_link_names_kept_once(cx: &mut TestAppContext) {
    let dir = start(cx, "recent-hardlink");
    let a = dir.join("a.ksq");
    write_sample(&a);
    let b = dir.join("b.ksq");
    std::fs::hard_link(&a, &b).unwrap();
    cx.update(|cx| files::open_path(&a, cx)).unwrap();
    cx.update(|cx| files::open_path(&b, cx)).unwrap();
    assert_eq!(cx.read(files::recent_files).len(), 1);
}

// ---------- UI-11 ----------

/// 파일·창·끝내기 Action이 모두 메뉴에 있다.
#[gpui_kit::test]
fn review_r2_file_actions_are_all_in_menus(cx: &mut TestAppContext) {
    fn walk(items: &[MenuItem], cx: &gpui_kit::App, out: &mut Vec<String>) {
        for item in items {
            match item {
                MenuItem::Action { action, .. } => {
                    if let Some(target) = keyscore_app::commands::menu_target(action.as_ref(), cx) {
                        out.push(target.name().to_string());
                    }
                }
                MenuItem::Submenu(menu) => walk(&menu.items, cx, out),
                _ => {}
            }
        }
    }
    let dir = start(cx, "ui11");
    let path = dir.join("song.ksq");
    write_sample(&path);
    cx.update(|cx| files::open_path(&path, cx)).unwrap();
    let names = cx.update(|cx| {
        let mut out = Vec::new();
        for menu in keyscore_app::commands::menus(cx) {
            walk(&menu.items, cx, &mut out);
        }
        out
    });
    for want in [
        "file::New",
        "file::Open",
        "file::Save",
        "file::SaveAs",
        "file::OpenRecent",
        "window::Close",
        "app::Quit",
    ] {
        assert!(names.iter().any(|n| n == want), "{want}가 메뉴에 없다");
    }
}

/// 흐름이 풀렸는가: 다른 이름 저장을 다시 내면 새 대화상자가 뜬다(취소한다).
fn assert_flow_cleared(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    assert!(!cx.did_prompt_for_new_path(), "이미 대화상자가 떠 있다");
    run(cx, handle, file::SaveAs);
    assert!(cx.did_prompt_for_new_path(), "흐름이 풀리지 않았다");
    answer_new_path(cx, None);
}

// ---------- 포커스 ----------

/// 포커스가 없는 바뀐 창에 `window::Close`를 내도 시트로 묻는다(전역 `Close` 처리기로 빠져 바로 닫히지 않는다).
#[gpui_kit::test]
fn review_r2_close_without_focus_still_asks(cx: &mut TestAppContext) {
    start(cx, "close-nofocus");
    let (handle, view) = open(cx, sample());
    touch(cx, handle);
    cx.update_window(handle, |_, window, cx| window.blur(cx))
        .unwrap();
    cx.run_until_parked();
    run(cx, handle, window_actions::Close);
    assert!(
        is_open(cx, handle) || !dirty(cx, &view),
        "바뀐 창이 묻지 않고 닫혔다"
    );
}

/// 포커스가 없는 창에 저장을 내도 저장된다.
#[gpui_kit::test]
fn review_r2_save_without_focus_saves(cx: &mut TestAppContext) {
    let dir = start(cx, "save-nofocus");
    let path = dir.join("s.ksq");
    write_sample(&path);
    let (handle, view) = open_file(cx, &path);
    touch(cx, handle);
    cx.update_window(handle, |_, window, cx| window.blur(cx))
        .unwrap();
    cx.run_until_parked();
    run(cx, handle, file::Save);
    assert!(
        !dirty(cx, &view),
        "포커스가 없으면 저장이 아무 일도 하지 않는다"
    );
}

// ---------- 리뷰 2 부록 A(수정 확인) ----------

/// 상위 버전 원본을 연 뒤 Finder에서 옮기면, 옮긴 원본을 「다른 이름으로 저장」으로 골라 덮을 수 있다.
#[gpui_kit::test]
fn review_r2_save_as_onto_moved_newer_original_keeps_original(cx: &mut TestAppContext) {
    let dir = start(cx, "newer-moved");
    let path = dir.join("future.ksq");
    let (handle, _, before) = open_newer(cx, &path);
    let moved = dir.join("moved.ksq");
    std::fs::rename(&path, &moved).unwrap();
    run(cx, handle, file::Save);
    answer(cx, "다른 이름으로 저장");
    answer_new_path(cx, Some(moved.clone()));
    if cx.has_pending_prompt() {
        answer(cx, "확인");
    }
    assert_eq!(
        std::fs::read(&moved).unwrap(),
        before,
        "옮긴 상위 버전 원본이 버전 1로 덮였다"
    );
}

/// 연 파일을 바깥에서 이름을 바꾼 뒤 새 이름으로 열면 같은 파일이 두 창에 열린다.
#[gpui_kit::test]
fn review_r2_open_renamed_file_activates_existing_window(cx: &mut TestAppContext) {
    let dir = start(cx, "renamed");
    let path = dir.join("a.ksq");
    write_sample(&path);
    let (first, _) = open_file(cx, &path);
    let renamed = dir.join("b.ksq");
    std::fs::rename(&path, &renamed).unwrap();
    let (again, _) = cx.update(|cx| files::open_path(&renamed, cx)).unwrap();
    assert_eq!(again, first, "같은 파일(inode)이 두 창에 열렸다");
}

/// 열기 실패 시트가 떠 있는 바뀐 창에 끝내기를 내면 같은 창에 시트가 겹친다.
#[gpui_kit::test]
fn review_r2_quit_during_open_error_sheet_does_not_stack(cx: &mut TestAppContext) {
    let dir = start(cx, "quit-open-error");
    let bad = dir.join("broken.ksq");
    std::fs::write(&bad, "{ not json").unwrap();
    let (handle, _) = open(cx, sample());
    touch(cx, handle);
    run(cx, handle, file::Open);
    cx.simulate_path_prompt_response(move |_| Some(vec![bad]));
    cx.run_until_parked();
    assert!(prompt_text(cx).contains("열지 못했다"));
    let quit = start_quit(cx);
    answer(cx, "확인");
    assert!(
        !(cx.has_pending_prompt() && quit.get().is_none()),
        "열기 실패 시트 뒤에 끝내기 시트가 같은 창에 겹쳐 있었다"
    );
}

/// 열기 실패 시트가 떠 있는 바뀐 창에 닫기를 내면 같은 창에 시트가 겹친다.
#[gpui_kit::test]
fn review_r2_close_during_open_error_sheet_does_not_stack(cx: &mut TestAppContext) {
    let dir = start(cx, "close-open-error");
    let bad = dir.join("broken.ksq");
    std::fs::write(&bad, "{ not json").unwrap();
    let (handle, _) = open(cx, sample());
    touch(cx, handle);
    run(cx, handle, file::Open);
    cx.simulate_path_prompt_response(move |_| Some(vec![bad]));
    cx.run_until_parked();
    run(cx, handle, window_actions::Close);
    answer(cx, "확인");
    assert!(
        !cx.has_pending_prompt(),
        "열기 실패 시트 뒤에 닫기 시트가 같은 창에 겹쳐 있었다"
    );
}

/// 설정에 같은 파일이 두 모양으로 있으면 시작하자마자 메뉴에 두 줄이다.
#[gpui_kit::test]
fn review_r2_recent_noncanonical_duplicate_at_startup_shown_once(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!(
        "ks-review-t7a-r2-{}-startup-dup",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let path = dir.join("song.ksq");
    write_sample(&path);
    let roundabout = dir.join("sub/../song.ksq");
    std::fs::write(
        dir.join("settings.json"),
        serde_json::json!({ "recent_files": [
            path.canonicalize().unwrap().display().to_string(),
            roundabout.display().to_string()
        ] })
        .to_string(),
    )
    .unwrap();
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let n = cx.update(|cx| {
        keyscore_app::commands::palette_entries(cx)
            .iter()
            .filter(|e| e.name() == "file::OpenRecent")
            .count()
    });
    assert_eq!(n, 1, "같은 파일이 최근 파일에 두 줄");
}
