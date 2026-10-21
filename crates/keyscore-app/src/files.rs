//! 파일 열기·저장·닫기 흐름(S2-T7a, IO-1, 7.5).
//!
//! 대화상자와 시트는 GPUI 플랫폼 것(macOS 표준 시트)을 쓴다. 흐름은 비동기 작업이고, 시트가 떠 있는
//! 동안에도 문서는 편집할 수 있다. 한 창에는 흐름이 하나만 돈다(`ProjectWindow::file_flow`). 같은 창에
//! 시트를 겹쳐 띄우지 않기 위해서다.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use gpui_kit::{
    AnyWindowHandle, App, AsyncApp, Context, Entity, PathPromptOptions, PromptButton, PromptLevel,
    Task, Window,
};
use keyscore_core::Project;
use keyscore_core::ksq::{self, Compression, KsqError};

use crate::OpenError;
use crate::actions::file;
use crate::actions::window::Close;
use crate::app_state::AppSettings;
use crate::document::{ReadOnly, UNTITLED};
use crate::window::{ProjectWindow, project_windows};

/// 최근 파일 목록의 길이.
pub const MAX_RECENT_FILES: usize = 10;

/// 시트 단추. 시험이 이 글자로 답한다.
const SAVE: &str = "저장";
const DONT_SAVE: &str = "저장 안 함";
const CANCEL: &str = "취소";
const SAVE_AS: &str = "다른 이름으로 저장";
const OK: &str = "확인";

/// 파일 작업 오류.
#[derive(Debug, thiserror::Error)]
pub enum FileError {
    /// 읽지 못했다.
    #[error("{0}")]
    Read(KsqError),
    /// 쓰지 못했다.
    #[error("{0}")]
    Write(KsqError),
    /// 파일에 쓸 권한이 없다(결정 2026-10-21-s2-t7-save-policy-and-split 1 A).
    #[error("파일에 쓸 권한이 없다")]
    PermissionDenied,
    /// 새 버전 앱이 만든 원본 위에 다른 이름으로 저장하려 했다(결정 2026-10-12-ksq-newer-version-save A).
    #[error("새 버전 앱이 만든 원본은 덮어쓸 수 없다. 다른 파일을 고른다")]
    NewerFormatOriginal,
    /// 다른 창이 이미 그 파일을 열고 있다.
    #[error("다른 창이 이미 이 파일을 열고 있다: {}", .0.display())]
    AlreadyOpen(PathBuf),
    /// 창을 열지 못했다.
    #[error("{0}")]
    Window(#[from] OpenError),
}

/// 파일 작업 Action을 건다(`init`이 부른다). 저장·다른 이름·닫기는 창의 처리기다(`ProjectWindow`).
pub(crate) fn register(cx: &mut App) {
    cx.on_action(|_: &file::Open, cx| open_dialog(cx));
    cx.on_action(|action: &file::OpenRecent, cx| open_recent(PathBuf::from(&action.0), cx));
}

/// 프로젝트를 쓴다. `.ksqz`면 gzip, 그 밖은 평문이다. 쓰기는 임시 파일 + rename이라 도중에 죽어도 원본은
/// 옛 내용 그대로다.
///
/// # Errors
///
/// 쓸 권한이 없으면 [`FileError::PermissionDenied`], 그 밖의 실패는 [`FileError::Write`].
pub fn save_project(path: &Path, project: &Project) -> Result<(), FileError> {
    check_writable(path)?;
    let compression = if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ksqz"))
    {
        Compression::Gzip
    } else {
        Compression::None
    };
    ksq::write_file(path, project, compression).map_err(|e| match e {
        KsqError::Io(io) if io.kind() == io::ErrorKind::PermissionDenied => {
            FileError::PermissionDenied
        }
        e => FileError::Write(e),
    })
}

/// 있는 파일에 쓸 권한이 있는지 본다. 임시 파일 + rename 쓰기는 폴더 권한만 보므로 그대로 두면 0444 파일도
/// 바뀐다(결정 1 A: 거부). 쓰기로 열기만 하고 내용·시각은 건드리지 않는다. 일반 파일만 연다(FIFO를 쓰기로
/// 열면 멈춘다).
fn check_writable(path: &Path) -> Result<(), FileError> {
    match fs::metadata(path) {
        Ok(meta) if meta.is_file() => match fs::OpenOptions::new().write(true).open(path) {
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                Err(FileError::PermissionDenied)
            }
            _ => Ok(()),
        },
        _ => Ok(()),
    }
}

/// 파일을 새 창으로 연다. 이미 열린 파일이면 그 창을 앞으로 가져온다. 최근 파일에 넣는다.
///
/// # Errors
///
/// [`FileError`].
pub fn open_path(
    path: &Path,
    cx: &mut App,
) -> Result<(AnyWindowHandle, Entity<ProjectWindow>), FileError> {
    let path = path
        .canonicalize()
        .map_err(|e| FileError::Read(KsqError::Io(e)))?;
    if let Some(view) = window_with_path(&path, None, cx) {
        let handle = view.read(cx).window_handle();
        let _ = handle.update(cx, |_, window, _| window.activate_window());
        add_recent(&path, cx);
        return Ok((handle, view));
    }
    let (project, warnings) = ksq::read_file(&path).map_err(FileError::Read)?;
    let read_only = warnings
        .iter()
        .find_map(ksq::Warning::newer_format_version)
        .map(ReadOnly::NewerFormat);
    let (handle, view) = crate::open_project_window(project, cx)?;
    let name = file_name(&path);
    view.update(cx, |w, cx| {
        w.document()
            .update(cx, |d, _| d.set_file(path.clone(), read_only));
        for warning in &warnings {
            w.say(format!("{name}: {warning}"), cx);
        }
    });
    refresh(&view, cx);
    add_recent(&path, cx);
    Ok((handle, view))
}

/// 바뀐 창마다 차례로 닫기 시트를 띄운다. 모두 저장하거나 버렸으면 참, 하나라도 취소하면 거짓이다
/// (`app::Quit`이 참일 때만 끝낸다). 답한 창은 바로 닫는다(macOS 「변경 사항 검토」와 같다).
///
/// 시트는 창 단위라 그동안 다른 창을 고치거나 새 창을 열 수 있다. 그래서 시작할 때의 목록이 아니라 매번
/// 지금 바뀐 창을 다시 찾고, 남지 않을 때까지 묻는다(리뷰 1 지적 2).
pub fn confirm_close_all(cx: &mut App) -> Task<bool> {
    cx.spawn(async move |cx| {
        loop {
            let Some(view) = cx.update(|cx| unsaved_windows(cx).into_iter().next()) else {
                return true;
            };
            let (handle, busy) = cx.update(|cx| {
                let w = view.read(cx);
                (w.window_handle(), w.file_flow)
            });
            // 다른 흐름의 시트가 떠 있으면 그 답을 모른다. 끝내지 않는다.
            if busy {
                return false;
            }
            set_busy(&view, true, cx);
            let _ = handle.update(cx, |_, window, _| window.activate_window());
            let close = confirm_close(&view, cx).await;
            set_busy(&view, false, cx);
            if !close {
                return false;
            }
            close_now(&view, cx);
        }
    })
}

/// 열려 있고 저장 안 된 변경이 있는 프로젝트 창.
pub(crate) fn unsaved_windows(cx: &App) -> Vec<Entity<ProjectWindow>> {
    let open = cx.windows();
    project_windows(cx)
        .into_iter()
        .filter(|w| {
            let w = w.read(cx);
            open.contains(&w.window_handle()) && w.document().read(cx).is_dirty()
        })
        .collect()
}

/// 최근 파일(최근이 먼저).
#[must_use]
pub fn recent_files(cx: &App) -> Vec<PathBuf> {
    recent_entries(cx).iter().map(PathBuf::from).collect()
}

/// 설정의 최근 목록에서 같은 파일을 하나로 합친 것(앞의 것을 남긴다). 손으로 고친 설정은 같은 파일을 다른
/// 모양으로 가질 수 있고, 다음 넣기 전까지는 설정에 그대로 남는다(리뷰 2 지적 4).
fn recent_entries(cx: &App) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for entry in &AppSettings::get(cx).recent_files {
        if !kept
            .iter()
            .any(|k| same_file(Path::new(k), Path::new(entry)))
        {
            kept.push(entry.clone());
        }
    }
    kept
}

/// 창을 닫아도 되는가. 참이면 바로 닫아도 된다(재생은 이미 멈췄다). 바뀐 창이면 시트를 띄우고 거짓을
/// 돌려주며, 답에 따라 흐름이 스스로 창을 닫는다. 창 닫기 단추와 `window::Close`가 함께 쓴다.
pub(crate) fn request_close(view: &Entity<ProjectWindow>, cx: &mut App) -> bool {
    let (busy, dirty) = {
        let w = view.read(cx);
        (w.file_flow, w.document().read(cx).is_dirty())
    };
    if busy {
        return false;
    }
    if !dirty {
        view.update(cx, ProjectWindow::stop_playback);
        return true;
    }
    set_busy_now(view, true, cx);
    let view = view.clone();
    cx.spawn(async move |cx| {
        let close = confirm_close(&view, cx).await;
        set_busy(&view, false, cx);
        if close {
            close_now(&view, cx);
        }
    })
    .detach();
    false
}

impl ProjectWindow {
    pub(crate) fn file_save(&mut self, _: &file::Save, _: &mut Window, cx: &mut Context<Self>) {
        self.run_file_flow(cx, async |view, cx| {
            save(view, cx).await;
        });
    }

    pub(crate) fn file_save_as(
        &mut self,
        _: &file::SaveAs,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_file_flow(cx, async |view, cx| {
            save_as(view, cx).await;
        });
    }

    pub(crate) fn window_close(&mut self, _: &Close, _: &mut Window, cx: &mut Context<Self>) {
        // 시트가 떠 있으면 그 답을 기다린다.
        if self.file_flow {
            return;
        }
        let view = cx.entity();
        // 처리기 안에서는 이 창을 닫을 수 없다(디스패치 중). 끝난 뒤에 묻고 닫는다.
        cx.defer(move |cx| {
            if request_close(&view, cx) {
                close_now_sync(&view, cx);
            }
        });
    }

    /// 이 창에서 흐름 하나를 돌린다. 이미 돌고 있으면 무시한다(시트를 겹치지 않는다).
    fn run_file_flow(
        &mut self,
        cx: &mut Context<Self>,
        flow: impl AsyncFnOnce(&Entity<ProjectWindow>, &mut AsyncApp) + 'static,
    ) {
        if self.file_flow {
            return;
        }
        self.file_flow = true;
        let view = cx.entity();
        cx.spawn(async move |_, cx| {
            flow(&view, cx).await;
            set_busy(&view, false, cx);
        })
        .detach();
    }

    /// 창 제목·편집됨 표시·문서 경로를 문서에 맞춘다.
    pub(crate) fn refresh_file_state(&self, window: &mut Window, cx: &App) {
        let doc = self.document().read(cx);
        let mut title = doc.display_name();
        if doc.read_only().is_some() {
            title.push_str(" (읽기 전용)");
        }
        window.set_window_title(&title);
        window.set_window_edited(doc.is_dirty());
        window.set_document_path(doc.path());
    }
}

// ---------------------------------------------------------------- 흐름

/// 저장. 성공하면 참.
async fn save(view: &Entity<ProjectWindow>, cx: &mut AsyncApp) -> bool {
    let (handle, path, read_only, name) = cx.update(|cx| {
        let w = view.read(cx);
        let d = w.document().read(cx);
        (
            w.window_handle(),
            d.path().map(Path::to_path_buf),
            d.read_only(),
            d.display_name(),
        )
    });
    if let Some(ReadOnly::NewerFormat(version)) = read_only {
        let answer = ask(
            handle,
            PromptLevel::Warning,
            &format!("“{name}”은 새 버전 앱이 만든 파일이라 덮어쓸 수 없다"),
            Some(&format!(
                "파일 형식 버전 {version}은 이 앱이 아는 버전({})보다 새롭다. 다른 이름으로 저장하면 원본은 그대로 남는다.",
                ksq::FORMAT_VERSION
            )),
            &[PromptButton::ok(SAVE_AS), PromptButton::cancel(CANCEL)],
            cx,
        )
        .await;
        return answer == Some(0) && save_as(view, cx).await;
    }
    let Some(path) = path else {
        return save_as(view, cx).await;
    };
    match write(view, &path, cx) {
        Ok(()) => true,
        Err(FileError::PermissionDenied) => {
            let answer = ask(
                handle,
                PromptLevel::Warning,
                &format!("“{name}”에 쓸 권한이 없다"),
                Some("읽기 전용 파일이다. 다른 이름으로 저장하면 원본은 그대로 남는다."),
                &[PromptButton::ok(SAVE_AS), PromptButton::cancel(CANCEL)],
                cx,
            )
            .await;
            answer == Some(0) && save_as(view, cx).await
        }
        Err(e) => {
            report_save_error(handle, &name, &e, cx).await;
            false
        }
    }
}

/// 다른 이름으로 저장. 성공하면 참.
async fn save_as(view: &Entity<ProjectWindow>, cx: &mut AsyncApp) -> bool {
    let (handle, current, read_only) = cx.update(|cx| {
        let w = view.read(cx);
        let d = w.document().read(cx);
        (
            w.window_handle(),
            d.path().map(Path::to_path_buf),
            d.read_only(),
        )
    });
    let directory = current
        .as_deref()
        .and_then(Path::parent)
        .map_or_else(default_directory, Path::to_path_buf);
    let suggested = current
        .as_deref()
        .map_or_else(|| format!("{UNTITLED}.ksq"), file_name);
    let chosen = cx.update(|cx| cx.prompt_for_new_path(&directory, Some(&suggested)));
    let Ok(Ok(Some(path))) = chosen.await else {
        return false;
    };
    let name = file_name(&path);
    // 상위 버전 원본은 다른 이름으로 저장해도 덮어쓰지 않는다(리뷰 1 지적 1). 링크·하드 링크로 가리켜도 같다.
    if read_only.is_some()
        && cx.update(|cx| is_document_file(view.read(cx).document().read(cx), &path))
    {
        report_save_error(handle, &name, &FileError::NewerFormatOriginal, cx).await;
        return false;
    }
    if cx.update(|cx| window_with_path(&path, Some(view), cx).is_some()) {
        let target = normalize_new(&path);
        report_save_error(handle, &name, &FileError::AlreadyOpen(target), cx).await;
        return false;
    }
    match write(view, &path, cx) {
        Ok(()) => {
            let saved = cx.update(|cx| {
                view.read(cx)
                    .document()
                    .read(cx)
                    .path()
                    .map(Path::to_path_buf)
            });
            if let Some(saved) = saved {
                cx.update(|cx| add_recent(&saved, cx));
            }
            true
        }
        Err(e) => {
            report_save_error(handle, &name, &e, cx).await;
            false
        }
    }
}

/// 닫아도 되는가: 바뀌지 않았거나, [저장]해서 성공했거나, [저장 안 함]이면 참.
async fn confirm_close(view: &Entity<ProjectWindow>, cx: &mut AsyncApp) -> bool {
    let (handle, dirty, name) = cx.update(|cx| {
        let w = view.read(cx);
        let d = w.document().read(cx);
        (w.window_handle(), d.is_dirty(), d.display_name())
    });
    if !dirty {
        return true;
    }
    let answer = ask(
        handle,
        PromptLevel::Warning,
        &format!("“{name}”의 변경 사항을 저장하겠는가?"),
        Some("저장하지 않으면 변경 사항을 잃는다."),
        &[
            PromptButton::ok(SAVE),
            PromptButton::cancel(CANCEL),
            PromptButton::new(DONT_SAVE),
        ],
        cx,
    )
    .await;
    match answer {
        Some(0) => save(view, cx).await,
        Some(2) => true,
        _ => false,
    }
}

/// 문서를 `path`에 쓰고 저장한 것으로 표시한다.
fn write(view: &Entity<ProjectWindow>, path: &Path, cx: &mut AsyncApp) -> Result<(), FileError> {
    cx.update(|cx| {
        save_project(path, view.read(cx).document().read(cx).project())?;
        // 쓴 뒤의 실제 파일(링크면 대상)로 정규화한다. 실패하면 고른 경로 그대로다.
        let saved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let document = view.read(cx).document().clone();
        document.update(cx, |d, _| d.mark_saved(saved));
        refresh(view, cx);
        Ok(())
    })
}

async fn report_save_error(
    handle: AnyWindowHandle,
    name: &str,
    error: &FileError,
    cx: &mut AsyncApp,
) {
    ask(
        handle,
        PromptLevel::Critical,
        &format!("“{name}”을 저장하지 못했다"),
        Some(&error.to_string()),
        &[PromptButton::ok(OK)],
        cx,
    )
    .await;
}

/// 시트를 띄우고 누른 단추의 번호를 기다린다. 창이 없어졌으면 `None`.
async fn ask(
    handle: AnyWindowHandle,
    level: PromptLevel,
    message: &str,
    detail: Option<&str>,
    answers: &[PromptButton],
    cx: &mut AsyncApp,
) -> Option<usize> {
    let answer = handle
        .update(cx, |_, window, cx| {
            window.prompt(level, message, detail, answers, cx)
        })
        .ok()?;
    answer.await.ok()
}

// ---------------------------------------------------------------- 열기

fn open_dialog(cx: &mut App) {
    let origin = active_project_window(cx);
    let chosen = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: true,
        prompt: Some("열기".into()),
    });
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(paths))) = chosen.await else {
            return;
        };
        cx.update(|cx| open_paths(&paths, origin.as_ref(), false, cx));
    })
    .detach();
}

fn open_recent(path: PathBuf, cx: &mut App) {
    let origin = active_project_window(cx);
    open_paths(&[path], origin.as_ref(), true, cx);
}

/// 파일들을 연다. 실패는 시트 하나로 모아 알린다. 하나라도 열었고 `origin`이 손대지 않은 새 창이면 그
/// 창을 닫는다(macOS 문서 앱 관례). 그 창에 저장 흐름이 돌고 있으면(대화상자가 떠 있다) 닫지 않는다. 닫으면
/// 대화상자의 답이 보이지 않는 문서를 쓴다(리뷰 1 지적 3). `forget_failed`면 열지 못한 파일을 최근 목록에서 뺀다.
fn open_paths(
    paths: &[PathBuf],
    origin: Option<&Entity<ProjectWindow>>,
    forget_failed: bool,
    cx: &mut App,
) {
    let mut opened = None;
    let mut failures = Vec::new();
    for path in paths {
        match open_path(path, cx) {
            Ok((handle, _)) => opened = opened.or(Some(handle)),
            Err(e) => {
                if forget_failed {
                    remove_recent(path, cx);
                }
                failures.push(format!("{}: {e}", path.display()));
            }
        }
    }
    let origin_handle = origin.map(|o| o.read(cx).window_handle());
    if let (Some(origin), Some(handle)) = (origin, opened)
        && origin_handle != Some(handle)
        && !origin.read(cx).file_flow
        && origin.read(cx).document().read(cx).is_pristine()
    {
        close_now_sync(origin, cx);
    }
    if failures.is_empty() {
        return;
    }
    // 시트를 띄울 창: 연 창, 명령을 낸 창, 그 밖의 창 가운데 흐름이 돌지 않는 첫 창. 없으면 새 창. 흐름이 도는
    // 창에는 이미 시트가 떠 있을 수 있어 겹치지 않게 피하고, 띄우는 동안 그 창의 흐름으로 표시한다(리뷰 2
    // 지적 3).
    let views = project_windows(cx);
    let open = cx.windows();
    let free = |handle: AnyWindowHandle, cx: &App| {
        views
            .iter()
            .find(|v| v.read(cx).window_handle() == handle)
            .filter(|v| open.contains(&handle) && !v.read(cx).file_flow)
            .cloned()
    };
    let host = opened
        .and_then(|h| free(h, cx))
        .or_else(|| origin_handle.and_then(|h| free(h, cx)))
        .or_else(|| {
            views
                .iter()
                .find_map(|v| free(v.read(cx).window_handle(), cx))
        })
        .or_else(|| {
            crate::open_project_window(Project::new(), cx)
                .ok()
                .map(|(_, v)| v)
        });
    let Some(host) = host else {
        eprintln!("KeyScore: 파일을 열지 못했다: {}", failures.join("; "));
        return;
    };
    let message = if paths.len() == 1 {
        format!("“{}”을 열지 못했다", file_name(&paths[0]))
    } else {
        "파일을 열지 못했다".to_owned()
    };
    let detail = failures.join("\n");
    set_busy_now(&host, true, cx);
    cx.spawn(async move |cx| {
        let handle = cx.update(|cx| host.read(cx).window_handle());
        ask(
            handle,
            PromptLevel::Critical,
            &message,
            Some(&detail),
            &[PromptButton::ok(OK)],
            cx,
        )
        .await;
        set_busy(&host, false, cx);
    })
    .detach();
}

// ---------------------------------------------------------------- 최근 파일

fn add_recent(path: &Path, cx: &mut App) {
    let entry = path.display().to_string();
    let before = AppSettings::get(cx).recent_files.clone();
    AppSettings::update(cx, |s| {
        // 손으로 고친 설정은 정규화하지 않은 경로를 가질 수 있다. 같은 파일이면 모양이 달라도 뺀다(리뷰 1 지적 4).
        s.recent_files.retain(|p| !same_file(Path::new(p), path));
        s.recent_files.insert(0, entry);
        s.recent_files.truncate(MAX_RECENT_FILES);
    });
    cx.add_recent_document(path);
    if AppSettings::get(cx).recent_files != before {
        crate::set_menus(cx);
    }
}

/// 최근 목록에서 뺀다. 목록은 정규화한 경로라, 다른 모양(상대 경로, 링크)으로 가리켜도 같은 파일이면 뺀다.
fn remove_recent(path: &Path, cx: &mut App) {
    let target = normalize_new(path);
    let same = |p: &String| Path::new(p) == path || normalize_new(Path::new(p)) == target;
    if !AppSettings::get(cx).recent_files.iter().any(same) {
        return;
    }
    AppSettings::update(cx, |s| s.recent_files.retain(|p| !same(p)));
    crate::set_menus(cx);
}

/// 최근 파일 메뉴·팔레트의 이름: 파일 이름. 같은 이름이 여럿이면 서로 구별될 때까지 위 폴더를 붙인다
/// (「song.ksq — x/proj」, 리뷰 1 지적 5).
pub(crate) fn recent_labels(cx: &App) -> Vec<(String, String)> {
    let recent = &recent_entries(cx);
    let names: Vec<String> = recent.iter().map(|p| file_name(Path::new(p))).collect();
    recent
        .iter()
        .zip(&names)
        .map(|(path, name)| {
            let others: Vec<&String> = recent
                .iter()
                .zip(&names)
                .filter(|(p, n)| *n == name && *p != path)
                .map(|(p, _)| p)
                .collect();
            let label = if others.is_empty() {
                name.clone()
            } else {
                format!("{name} — {}", distinct_folder(path, &others))
            };
            (label, path.clone())
        })
        .collect()
}

/// `path`의 부모 폴더 꼬리를 `others`의 것과 다를 때까지 늘린다. 끝까지 같으면 부모 경로 전체다.
fn distinct_folder(path: &str, others: &[&String]) -> String {
    let folders = |p: &str| -> Vec<String> {
        Path::new(p)
            .parent()
            .map(|d| {
                d.components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    };
    let tail = |parts: &[String], k: usize| parts[parts.len().saturating_sub(k)..].join("/");
    let mine = folders(path);
    let theirs: Vec<Vec<String>> = others.iter().map(|p| folders(p)).collect();
    for k in 1..=mine.len() {
        let candidate = tail(&mine, k);
        if theirs.iter().all(|t| tail(t, k) != candidate) {
            return candidate;
        }
    }
    Path::new(path)
        .parent()
        .map_or_else(String::new, |d| d.display().to_string())
}

// ---------------------------------------------------------------- 도움 함수

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// 아직 없을 수 있는 경로의 정규형: 있으면 정규화, 없으면 부모를 정규화하고 이름을 붙인다.
fn normalize_new(path: &Path) -> PathBuf {
    if let Ok(p) = path.canonicalize() {
        return p;
    }
    match (path.parent().map(Path::canonicalize), path.file_name()) {
        (Some(Ok(parent)), Some(name)) => parent.join(name),
        _ => path.to_path_buf(),
    }
}

fn default_directory() -> PathBuf {
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
    let documents = home.join("Documents");
    if documents.is_dir() { documents } else { home }
}

/// `path`를 연 창(`except`는 뺀다).
fn window_with_path(
    path: &Path,
    except: Option<&Entity<ProjectWindow>>,
    cx: &App,
) -> Option<Entity<ProjectWindow>> {
    project_windows(cx)
        .into_iter()
        .filter(|w| except.is_none_or(|e| e != w))
        .find(|w| is_document_file(w.read(cx).document().read(cx), path))
}

/// 같은 파일인가: 정규형 경로가 같거나(링크·`..`·대소문자 등은 `canonicalize`가 푼다), 둘 다 있고 같은
/// 장치의 같은 inode다(하드 링크, 리뷰 1 지적 6).
fn same_file(a: &Path, b: &Path) -> bool {
    if a == b || normalize_new(a) == normalize_new(b) {
        return true;
    }
    matches!((FileId::of(a), FileId::of(b)), (Some(x), Some(y)) if x == y)
}

/// 문서의 파일이 `path`와 같은가: 경로로, 또는 열거나 저장한 때의 정체로(바깥에서 옮기거나 이름을 바꾼 경우,
/// 리뷰 2 지적 1·2).
fn is_document_file(doc: &crate::document::ProjectDocument, path: &Path) -> bool {
    doc.path().is_some_and(|p| same_file(p, path))
        || doc.file_id().is_some_and(|id| FileId::of(path) == Some(id))
}

/// 파일 정체: 장치와 inode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileId {
    dev: u64,
    ino: u64,
}

impl FileId {
    /// 있는 파일의 정체. 없거나 읽을 수 없으면 `None`.
    pub(crate) fn of(path: &Path) -> Option<FileId> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            fs::metadata(path).ok().map(|m| FileId {
                dev: m.dev(),
                ino: m.ino(),
            })
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            None
        }
    }
}

fn active_project_window(cx: &App) -> Option<Entity<ProjectWindow>> {
    let active = cx.active_window()?;
    project_windows(cx)
        .into_iter()
        .find(|w| w.read(cx).window_handle() == active)
}

fn refresh(view: &Entity<ProjectWindow>, cx: &mut App) {
    let handle = view.read(cx).window_handle();
    let _ = handle.update(cx, |_, window, cx| {
        view.read(cx).refresh_file_state(window, cx);
    });
}

fn set_busy(view: &Entity<ProjectWindow>, busy: bool, cx: &mut AsyncApp) {
    cx.update(|cx| set_busy_now(view, busy, cx));
}

fn set_busy_now(view: &Entity<ProjectWindow>, busy: bool, cx: &mut App) {
    view.update(cx, |w, _| w.file_flow = busy);
}

/// 재생을 멈추고 창을 닫는다(확인은 끝났다).
fn close_now(view: &Entity<ProjectWindow>, cx: &mut AsyncApp) {
    cx.update(|cx| close_now_sync(view, cx));
}

fn close_now_sync(view: &Entity<ProjectWindow>, cx: &mut App) {
    view.update(cx, ProjectWindow::stop_playback);
    let handle = view.read(cx).window_handle();
    let _ = handle.update(cx, |_, window, _| window.remove_window());
}

/// 최근 목록 라벨을 계산한 횟수(시험이 메뉴 생성의 캐시를 본다).
#[doc(hidden)]
#[must_use]
pub fn recent_label_computations(_cx: &App) -> u64 {
    todo!()
}
