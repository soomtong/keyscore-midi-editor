//! 자동 저장본과 크래시 후 복구(S2-T7b, IO-5, 8.4).
//!
//! 저장본은 설정 파일 옆 `autosave/`에만 쓴다(앱에서는 `~/Library/Application Support/KeyScore/autosave/`).
//! 프로젝트 폴더와 원래 파일은 건드리지 않는다. 창마다 파일 둘이다.
//!
//! - `<키>.ksq`: 문서 내용(평문 `.ksq`).
//! - `<키>.json`: 메타 `{"path": 원래 경로 또는 null, "read_only": 상위 형식 버전 또는 null,
//!   "session": 세션 id, "pid": 프로세스 id}`.
//!
//! 실행 중인 앱은 `session-<세션 id>.lock`을 배타 잠금으로 쥔다. 잠금을 잡을 수 있는 세션(파일이 없어도)은
//! 끝난 세션이고, 그 저장본이 복구 대상이다. 프로세스가 죽으면 OS가 잠금을 푼다.

use std::fmt::Write as _;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui_kit::{App, AppContext as _, Context, Entity, PromptButton, PromptLevel, Task};
use keyscore_core::{Project, ksq};

use crate::app_state::AppSettings;
use crate::document::ReadOnly;
use crate::window::{ProjectWindow, project_windows};

/// 문서가 바뀐 뒤 저장본이 생기기까지(IO-5).
pub const AUTOSAVE_DELAY: Duration = Duration::from_secs(30);

/// 복구 시트 단추. 시험이 이 글자로 답한다.
const RECOVER: &str = "복구";
const DISCARD: &str = "버리기";

/// 복구할 수 있는 저장본 하나.
#[derive(Debug)]
pub struct Recoverable {
    key: String,
    path: Option<PathBuf>,
    read_only: Option<ReadOnly>,
    pid: Option<u32>,
    project: Project,
}

impl Recoverable {
    /// 원래 파일 경로. 새 프로젝트였으면 없다.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    #[must_use]
    pub fn read_only(&self) -> Option<ReadOnly> {
        self.read_only
    }
}

/// 이 실행의 세션. 잠금 파일을 쥐고 있는 동안 살아 있는 세션이다.
struct Session {
    id: String,
    pid: u32,
    lock: Option<(PathBuf, fs::File)>,
}

impl gpui_kit::Global for Session {}

/// 창 하나의 자동 저장 상태(`ProjectWindow`가 가진다).
pub(crate) struct WindowAutosave {
    /// 새 프로젝트의 키(창마다 다르다).
    untitled_key: String,
    /// 지금 저장본이 있는 키.
    written: Option<String>,
    /// 타이머 또는 쓰기가 진행 중이다.
    pending: Option<Task<()>>,
    /// 실패를 이미 알렸다(성공하면 다시 알린다).
    failure_shown: bool,
    /// 창을 닫았다. 뷰가 아직 살아 있어도(진행 중인 작업, 다른 참조) 더 쓰지 않고, 끝난 쓰기는 지운다.
    closed: bool,
}

impl WindowAutosave {
    pub(crate) fn new() -> Self {
        WindowAutosave {
            untitled_key: untitled_key(),
            written: None,
            pending: None,
            failure_shown: false,
            closed: false,
        }
    }
}

/// 세션 잠금을 잡는다(`init`이 부른다). 잠금을 못 잡아도 앱은 뜬다. 그때는 다른 실행이 이 세션의 저장본을
/// 복구 대상으로 볼 수 있다.
pub(crate) fn init(cx: &mut App) {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let id = format!("{pid}-{nanos}");
    let dir = autosave_dir(cx);
    let lock = fs::create_dir_all(&dir).ok().and_then(|()| {
        let path = dir.join(format!("session-{id}.lock"));
        let file = fs::File::create(&path).ok()?;
        file.try_lock().ok()?;
        Some((path, file))
    });
    cx.set_global(Session { id, pid, lock });
    // 정상 종료에는 바뀐 창이 없다(끝내기가 물었다). 취소할 수 없는 종료(Dock 메뉴·로그아웃)에서는 바뀐 창의
    // 저장본을 지금 쓰고, 다음 실행이 복구를 제안한다. 잠금 파일은 지운다(없으면 끝난 세션이다).
    cx.on_app_quit(|cx| {
        flush_all(cx);
        if let Some((path, _)) = cx.global_mut::<Session>().lock.take() {
            let _ = fs::remove_file(path);
        }
        async {}
    })
    .detach();
}

/// 저장본 폴더.
#[must_use]
pub fn autosave_dir(cx: &App) -> PathBuf {
    AppSettings::path(cx).with_file_name("autosave")
}

/// 끝난 세션의 저장본(내용과 메타를 읽을 수 있는 것만).
#[must_use]
pub fn find_recoverable(cx: &App) -> Vec<Recoverable> {
    scan(cx).good
}

/// 복구 대상이 있으면 시트로 묻는다(`run`이 첫 창을 연 뒤 부른다). 흐름은 스스로 끝까지 돈다(호출자가
/// 작업을 놓아 취소하는 일이 없게).
pub fn offer_recovery(cx: &mut App) {
    let Scan { good, corrupt } = scan(cx);
    if good.is_empty() {
        return;
    }
    let host = crate::files::active_project_window(cx)
        .or_else(|| project_windows(cx).into_iter().next())
        .or_else(|| {
            crate::open_project_window(Project::new(), cx)
                .ok()
                .map(|(_, v)| v)
        });
    let Some(host) = host else {
        return;
    };
    let message = format!(
        "KeyScore가 예기치 않게 끝났다. 저장하지 않은 프로젝트 {}개를 복구하겠는가?",
        good.len()
    );
    let mut detail = good
        .iter()
        .map(|r| {
            r.path.as_deref().map_or_else(
                || crate::document::UNTITLED.to_owned(),
                |p| p.display().to_string(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    if corrupt > 0 {
        let _ = write!(
            detail,
            "\n읽을 수 없는 저장본 {corrupt}개는 {}에 그대로 둔다.",
            autosave_dir(cx).display()
        );
    }
    crate::files::set_busy_now(&host, true, cx);
    cx.spawn(async move |cx| {
        let handle = cx.update(|cx| host.read(cx).window_handle());
        let answer = crate::files::ask(
            handle,
            PromptLevel::Warning,
            &message,
            Some(&detail),
            &[PromptButton::ok(RECOVER), PromptButton::new(DISCARD)],
            cx,
        )
        .await;
        cx.update(|cx| {
            crate::files::set_busy_now(&host, false, cx);
            match answer {
                Some(0) => recover(good, &host, cx),
                Some(1) => discard(&good, cx),
                // 창이 사라졌다: 다음 실행에서 다시 묻는다.
                _ => {}
            }
        });
    })
    .detach();
}

/// 바뀐 창의 저장본을 지금 쓴다(취소할 수 없는 종료 훅).
pub fn flush_all(cx: &mut App) {
    for view in project_windows(cx) {
        let Some(job) = view.update(cx, ProjectWindow::prepare_autosave) else {
            continue;
        };
        let result = job.write();
        view.update(cx, |w, cx| w.finish_autosave(&job, result, cx));
    }
}

// ---------------------------------------------------------------- 창의 자동 저장

/// 쓸 내용 하나(메인 스레드에서 직렬화하고, 쓰기는 백그라운드에서 한다).
pub(crate) struct Job {
    dir: PathBuf,
    key: String,
    content: String,
    meta: String,
    revision: u64,
}

impl Job {
    /// 내용을 먼저, 메타를 나중에 쓴다. 메타가 있으면 내용도 온전하다. 각각 임시 파일 + rename이다.
    fn write(&self) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        write_atomic(&self.dir.join(format!("{}.ksq", self.key)), &self.content)?;
        write_atomic(&self.dir.join(format!("{}.json", self.key)), &self.meta)
    }
}

impl ProjectWindow {
    /// 문서가 바뀌었다(창의 문서 관찰자가 부른다). 깨끗해졌으면 저장본을 지우고, 바뀐 채면 30 s 타이머를 건다.
    /// 타이머가 이미 있으면 그대로 둔다: 첫 변경부터 재므로 계속 고쳐도 30 s 안에 저장본이 생긴다.
    pub(crate) fn autosave_document_changed(&mut self, cx: &mut Context<Self>) {
        if self.autosave.closed || !self.document().read(cx).is_dirty() {
            self.autosave.pending = None;
            self.remove_autosave(cx);
            return;
        }
        if self.autosave.pending.is_none() {
            self.schedule_autosave(cx);
        }
    }

    fn schedule_autosave(&mut self, cx: &mut Context<Self>) {
        let timer = cx.background_executor().timer(AUTOSAVE_DELAY);
        self.autosave.pending = Some(cx.spawn(async move |this, cx| {
            timer.await;
            let Ok(Some(job)) = this.update(cx, ProjectWindow::prepare_autosave) else {
                let _ = this.update(cx, |w, _| w.autosave.pending = None);
                return;
            };
            let (job, result) = cx
                .background_spawn(async move {
                    let result = job.write();
                    (job, result)
                })
                .await;
            let finished = this.update(cx, |w, cx| {
                w.autosave.pending = None;
                w.finish_autosave(&job, result, cx);
            });
            // 쓰는 동안 뷰가 사라졌다(창을 닫았다). 방금 쓴 저장본을 남기지 않는다.
            if finished.is_err() {
                remove_files(&job.dir, &job.key);
            }
        }));
    }

    /// 지금 내용으로 쓸 일을 만든다. 바뀌지 않았으면 없다.
    fn prepare_autosave(&mut self, cx: &mut Context<Self>) -> Option<Job> {
        let doc = self.document().read(cx);
        if self.autosave.closed || !doc.is_dirty() {
            return None;
        }
        let key = doc
            .path()
            .map_or_else(|| self.autosave.untitled_key.clone(), path_key);
        let session = cx.global::<Session>();
        let meta = serde_json::json!({
            "path": doc.path().map(|p| p.display().to_string()),
            "read_only": doc.read_only().map(|ReadOnly::NewerFormat(v)| v),
            "session": session.id,
            "pid": session.pid,
        });
        Some(Job {
            dir: autosave_dir(cx),
            key,
            content: ksq::to_string(doc.project()),
            meta: meta.to_string(),
            revision: doc.revision(),
        })
    }

    fn finish_autosave(&mut self, job: &Job, result: io::Result<()>, cx: &mut Context<Self>) {
        match result {
            Ok(()) => {
                // 다른 이름으로 저장 등으로 키가 바뀌었으면 옛 저장본을 지운다.
                if let Some(old) = self.autosave.written.replace(job.key.clone())
                    && old != job.key
                {
                    remove_files(&job.dir, &old);
                }
                self.autosave.failure_shown = false;
                let doc = self.document().read(cx);
                let (dirty, revision) = (doc.is_dirty(), doc.revision());
                // 쓰는 동안 저장했거나 창을 닫았으면 방금 쓴 것을 지우고, 또 바뀌었으면 다음 타이머를 건다.
                if !dirty || self.autosave.closed {
                    self.remove_autosave(cx);
                } else if revision != job.revision && self.autosave.pending.is_none() {
                    self.schedule_autosave(cx);
                }
            }
            Err(e) => {
                // 다음 변경 때 다시 시도한다. 알림은 성공하기 전까지 한 번.
                if !self.autosave.failure_shown {
                    self.autosave.failure_shown = true;
                    self.say(format!("자동 저장본을 쓰지 못했다: {e}"), cx);
                }
            }
        }
    }

    /// 창을 닫는다: 저장본을 지우고 더 쓰지 않는다. 진행 중인 쓰기는 끝난 뒤 지운다.
    pub(crate) fn close_autosave(&mut self, cx: &App) {
        self.autosave.closed = true;
        self.remove_autosave(cx);
    }

    /// 이 창의 저장본을 지운다(명시 저장, 창 닫기).
    pub(crate) fn remove_autosave(&mut self, cx: &App) {
        if let Some(key) = self.autosave.written.take() {
            remove_files(&autosave_dir(cx), &key);
        }
    }
}

// ---------------------------------------------------------------- 복구

struct Scan {
    good: Vec<Recoverable>,
    /// 끝난 세션의 것인데 메타나 내용을 읽을 수 없는 저장본, 메타 없는 내용 파일.
    corrupt: usize,
}

fn scan(cx: &App) -> Scan {
    let dir = autosave_dir(cx);
    let own = cx.global::<Session>().id.clone();
    let mut scan = Scan {
        good: Vec::new(),
        corrupt: 0,
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return scan;
    };
    let mut names: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    names.sort();
    for content in names
        .iter()
        .filter(|p| p.extension().is_some_and(|x| x == "ksq"))
    {
        let Some(key) = content
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
        else {
            continue;
        };
        let meta_path = dir.join(format!("{key}.json"));
        let Some(meta) = fs::read_to_string(&meta_path)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        else {
            scan.corrupt += 1;
            continue;
        };
        let Some(session) = meta["session"].as_str() else {
            scan.corrupt += 1;
            continue;
        };
        if session == own || session_alive(&dir, session) {
            continue;
        }
        let Some(project) = fs::read_to_string(content)
            .ok()
            .and_then(|t| ksq::from_str(&t).ok())
            .map(|(p, _)| p)
        else {
            scan.corrupt += 1;
            continue;
        };
        scan.good.push(Recoverable {
            key,
            path: meta["path"].as_str().map(PathBuf::from),
            read_only: meta["read_only"].as_u64().map(ReadOnly::NewerFormat),
            pid: meta["pid"].as_u64().and_then(|p| u32::try_from(p).ok()),
            project,
        });
    }
    scan
}

/// 세션 잠금을 다른 프로세스가 쥐고 있는가. 잠금 파일이 없거나 잡을 수 있으면 끝난 세션이다. 판정할 수
/// 없으면(권한 등) 살아 있다고 본다: 남의 저장본을 가져가지 않는다.
fn session_alive(dir: &Path, session: &str) -> bool {
    let path = dir.join(format!("session-{session}.lock"));
    match fs::File::open(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(_) => true,
        Ok(file) => match file.try_lock() {
            Ok(()) => false,
            // 다른 프로세스가 쥐고 있거나(WouldBlock) 판정할 수 없다.
            Err(_) => true,
        },
    }
}

fn recover(good: Vec<Recoverable>, host: &Entity<ProjectWindow>, cx: &mut App) {
    let mut restored = false;
    for item in good {
        // 같은 파일을 이미 연 창이 있으면 경로 없이 연다(같은 파일 두 창 금지).
        let path = item
            .path
            .clone()
            .filter(|p| !crate::files::is_open_anywhere(p, cx));
        let Ok((handle, view)) = crate::open_project_window(item.project, cx) else {
            continue;
        };
        restored = true;
        view.update(cx, |w, cx| {
            w.document()
                .update(cx, |d, _| d.mark_recovered(path.clone(), item.read_only));
            if path.is_none() {
                w.autosave.untitled_key.clone_from(&item.key);
            }
            // 저장본을 이 세션 것으로 가져온다(다시 복구 대상이 되지 않게). 창이 다시 쓸 때 키가 바뀌면 지운다.
            w.autosave.written = Some(item.key.clone());
            if let Some(job) = w.prepare_autosave(cx) {
                let result = job.write();
                w.finish_autosave(&job, result, cx);
            }
        });
        let _ = handle.update(cx, |_, window, cx| {
            view.read(cx).refresh_file_state(window, cx);
        });
        remove_stale_temps(item.path.as_deref(), item.pid);
    }
    if restored && host.read(cx).document().read(cx).is_pristine() {
        crate::files::close_now_sync(host, cx);
    }
}

fn discard(good: &[Recoverable], cx: &mut App) {
    let dir = autosave_dir(cx);
    for item in good {
        remove_files(&dir, &item.key);
        remove_stale_temps(item.path.as_deref(), item.pid);
    }
}

/// 끝난 세션이 원래 파일 옆에 남긴 임시 파일(`.<이름>.tmp-<pid>-<n>`)을 지운다. 그 pid의 것만 지우므로 다른
/// 프로그램의 쓰기와 겹치지 않는다.
fn remove_stale_temps(path: Option<&Path>, pid: Option<u32>) {
    let (Some(path), Some(pid)) = (path, pid) else {
        return;
    };
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return;
    };
    let prefix = format!(".{}.tmp-{pid}-", name.to_string_lossy());
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

// ---------------------------------------------------------------- 도움 함수

fn remove_files(dir: &Path, key: &str) {
    // 메타를 먼저 지운다. 중간에 멈춰도 메타 없는 내용만 남아 복구 대상이 되지 않는다.
    let _ = fs::remove_file(dir.join(format!("{key}.json")));
    let _ = fs::remove_file(dir.join(format!("{key}.ksq")));
}

fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let temp = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .map_or_else(String::new, |e| e.to_string_lossy().into_owned())
    ));
    let result = (|| {
        let mut file = fs::File::create(&temp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// 원래 경로의 키: FNV-1a 64비트 16진. 실행마다 같아야 해서 표준 해셔(무작위 씨앗)를 쓰지 않는다.
fn path_key(path: &Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.as_os_str().as_encoded_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// 새 프로젝트 창의 키: 프로세스, 시각, 순번.
fn untitled_key() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("untitled-{}-{nanos}-{n}", std::process::id())
}
