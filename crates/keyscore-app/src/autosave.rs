//! 자동 저장본과 크래시 후 복구(S2-T7b, IO-5, 8.4).
//!
//! 저장본은 설정 파일 옆 `autosave/`에만 쓴다(앱에서는 `~/Library/Application Support/KeyScore/autosave/`).
//! 프로젝트 폴더와 원래 파일은 건드리지 않는다. 창마다 파일 둘이다. 키는 창마다 고유하다(프로세스, 시각,
//! 순번). 원래 경로는 메타에만 둔다. 경로로 키를 만들면 같은 파일을 연 다른 창(복구 창, 다른 실행)이 서로의
//! 저장본을 덮거나 지운다(S2-T7b 리뷰 1 지적 2·3·4).
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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui_kit::{App, AppContext as _, Context, Entity, PromptButton, PromptLevel, Task};
use keyscore_core::{Project, ksq};

use crate::app_state::AppSettings;
use crate::document::ReadOnly;
use crate::window::{ProjectWindow, project_windows};

/// 문서가 바뀐 뒤 저장본이 생기기까지(IO-5).
pub const AUTOSAVE_DELAY: Duration = Duration::from_secs(30);

/// 이보다 오래된 임시 파일만 끝난 세션의 것으로 본다.
const STALE_TEMP_AGE: Duration = Duration::from_mins(1);

/// 복구 시트 단추. 시험이 이 글자로 답한다.
const RECOVER: &str = "복구";
const DISCARD: &str = "버리기";

/// 복구할 수 있는 저장본 하나.
#[derive(Debug)]
pub struct Recoverable {
    key: String,
    session: String,
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

/// 한 창의 저장본 파일을 바꾸는 차례. 쓰기는 준비할 때 세대 번호를 받고, 바닥(`floor`)보다 낮은 세대는
/// rename하지 않고 버린다. 성공한 쓰기와 지우기는 잠금 안에서 바닥을 올린다. 그래서 늦게 끝난 옛 쓰기가
/// 종료 훅이 쓴 최신 저장본을 덮거나(리뷰 2 지적 1), 지운 뒤에 저장본을 되살리지 않는다(창이 사라진 뒤 끝난
/// 쓰기 포함, 리뷰 2 제안 5).
#[derive(Default)]
struct Slot {
    next: AtomicU64,
    floor: Mutex<u64>,
}

impl Slot {
    fn issue(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }

    /// 지금까지 준비한 모든 쓰기를 무효로 하고 파일을 지운다.
    fn remove(&self, dir: &Path, key: &str) {
        let mut floor = self
            .floor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *floor = self.next.load(Ordering::Relaxed);
        remove_files(dir, key);
    }
}

/// 창 하나의 자동 저장 상태(`ProjectWindow`가 가진다).
pub(crate) struct WindowAutosave {
    /// 이 창의 저장본 키(창마다 다르다). 복구한 창은 저장본의 키를 이어받는다.
    key: String,
    /// 이 창의 저장본 파일을 바꾸는 쓰기·지우기의 차례.
    slot: Arc<Slot>,
    /// 복구한 저장본 내용 파일의 배타 잠금. 이 세션 것으로 다시 쓰기 전까지(쓰기가 실패해도) 다른 실행이 같은
    /// 저장본을 복구 대상으로 보지 않게 한다(리뷰 2 제안 2).
    claim: Option<fs::File>,
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
            key: new_key(),
            slot: Arc::default(),
            claim: None,
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
        // 만들고 잠그는 사이에 다른 실행이 그 파일을 지웠으면(끝난 세션의 잠금으로 오인) 다시 만든다. 잠근 파일이
        // 경로의 파일과 같은지 확인한다. 다른 실행이 끝난 세션 판정(`session_alive`)으로 잠깐 잠갔으면 잠그기가
        // 실패한다. 그 잠금은 곧 풀리므로 조금 기다려 다시 시도한다(S2-T7b 리뷰 4 이월).
        for attempt in 0..3 {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(20));
            }
            let file = fs::File::create(&path).ok()?;
            if file.try_lock().is_ok() && same_inode(&file, &path) {
                return Some((path, file));
            }
        }
        None
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
        // 복구 대상이 없어도 끝난 세션의 찌꺼기(바뀐 창 없이 끝난 세션의 잠금 파일, 두 rename 사이에 끝난 세션의
        // 내용 없는 메타)를 지운다. [복구]·[버리기] 때만 정리하면 시트가 뜨지 않는 세션의 것이 쌓인다(S2-T7b
        // 리뷰 4 이월).
        remove_dead_locks(&autosave_dir(cx), &cx.global::<Session>().id.clone());
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
    slot: Arc<Slot>,
    generation: u64,
}

/// 쓰기 결과.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Written,
    /// 더 새 쓰기나 지우기가 먼저 끝나 이 쓰기는 버렸다.
    Superseded,
}

impl Job {
    /// 세대별 임시 파일에 내용과 메타를 쓰고, 차례 잠금 안에서 세대를 확인한 뒤 메타 → 내용 순으로 rename한다.
    /// 각 파일은 언제나 어느 회차의 온전한 내용이다.
    fn write(&self) -> io::Result<Outcome> {
        fs::create_dir_all(&self.dir)?;
        let content = self.dir.join(format!("{}.ksq", self.key));
        let meta = self.dir.join(format!("{}.json", self.key));
        let content_tmp = self
            .dir
            .join(format!("{}.ksq.tmp-{}", self.key, self.generation));
        let meta_tmp = self
            .dir
            .join(format!("{}.json.tmp-{}", self.key, self.generation));
        let staged = write_synced(&content_tmp, &self.content)
            .and_then(|()| write_synced(&meta_tmp, &self.meta));
        let result = staged.and_then(|()| {
            let mut floor = self
                .slot
                .floor
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if self.generation < *floor {
                return Ok(Outcome::Superseded);
            }
            // 메타를 먼저 바꾼다. 메타가 이 세션을 가리키면 내용이 옛 회차여도 다른 실행이 가져가지 않는다. 내용을
            // 먼저 바꾸면, 메타 rename이 실패할 때 복구 잠금(`claim`)은 지워진 옛 내용 inode에 남고 새 내용
            // 파일은 끝난 세션 것으로 보여 다른 실행이 가져간다(리뷰 3 제안 4).
            let had_content = content.exists();
            fs::rename(&meta_tmp, &meta)?;
            if let Err(e) = fs::rename(&content_tmp, &content) {
                // 처음 쓰는 저장본이면 내용 없는 메타를 남기지 않는다(리뷰 2 제안 5).
                if !had_content {
                    let _ = fs::remove_file(&meta);
                }
                return Err(e);
            }
            *floor = self.generation + 1;
            Ok(Outcome::Written)
        });
        let _ = fs::remove_file(&content_tmp);
        let _ = fs::remove_file(&meta_tmp);
        result
    }
}

impl ProjectWindow {
    /// 문서가 바뀌었다(창의 문서 관찰자가 부른다). 깨끗해졌으면 저장본을 지우고, 바뀐 채면 30 s 타이머를 건다.
    /// 타이머가 이미 있으면 그대로 둔다: 첫 변경부터 재므로 계속 고쳐도 30 s 안에 저장본이 생긴다.
    pub(crate) fn autosave_document_changed(&mut self, cx: &mut Context<Self>) {
        if self.autosave.closed || !self.document().read(cx).is_dirty() {
            // 진행 중인 작업은 놓지 않는다. 쓰기 도중이면 끝난 뒤 `finish_autosave`가 방금 쓴 것을 지운다. 놓으면
            // 그 뒷정리가 사라져 저장한 문서의 저장본이 남는다(리뷰 1 지적 1).
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
            // 쓰는 동안 뷰가 사라졌으면(창을 닫았다) 닫기가 이미 차례를 올려 이 쓰기는 버려졌다.
            let _ = this.update(cx, |w, cx| {
                w.autosave.pending = None;
                w.finish_autosave(&job, result, cx);
            });
        }));
    }

    /// 지금 내용으로 쓸 일을 만든다. 바뀌지 않았으면 없다.
    fn prepare_autosave(&mut self, cx: &mut Context<Self>) -> Option<Job> {
        let doc = self.document().read(cx);
        if self.autosave.closed || !doc.is_dirty() {
            return None;
        }
        let key = self.autosave.key.clone();
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
            slot: self.autosave.slot.clone(),
            generation: self.autosave.slot.issue(),
        })
    }

    fn finish_autosave(&mut self, job: &Job, result: io::Result<Outcome>, cx: &mut Context<Self>) {
        match result {
            Ok(outcome) => {
                if outcome == Outcome::Written {
                    // 이 세션 것으로 다시 썼으니 복구 때의 잠금은 더 필요 없다.
                    self.autosave.claim = None;
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
                // 더 고치지 않아도 30 s 뒤 다시 시도한다(리뷰 1 지적 5). 알림은 성공하기 전까지 한 번.
                if !self.autosave.failure_shown {
                    self.autosave.failure_shown = true;
                    self.say(format!("자동 저장본을 쓰지 못했다: {e}"), cx);
                }
                if !self.autosave.closed
                    && self.document().read(cx).is_dirty()
                    && self.autosave.pending.is_none()
                {
                    self.schedule_autosave(cx);
                }
            }
        }
    }

    /// 창을 닫는다: 저장본을 지우고 더 쓰지 않는다. 진행 중인 쓰기는 끝난 뒤 지운다.
    pub(crate) fn close_autosave(&mut self, cx: &App) {
        self.autosave.closed = true;
        self.remove_autosave(cx);
    }

    /// 이 창의 저장본을 지운다(명시 저장, 창 닫기). 진행 중인 쓰기도 무효가 된다.
    pub(crate) fn remove_autosave(&mut self, cx: &App) {
        self.autosave
            .slot
            .remove(&autosave_dir(cx), &self.autosave.key);
        self.autosave.claim = None;
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
        if session == own || session_alive(&dir, session) || claimed(content) {
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
            session: session.to_owned(),
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

/// 다른 실행(또는 이 실행의 다른 창)이 복구하며 내용 파일을 잠갔는가.
fn claimed(content: &Path) -> bool {
    fs::File::open(content).is_ok_and(|f| f.try_lock().is_err())
}

/// 내용 파일을 복구 잠금으로 잠근다. 파일이 없거나 다른 쪽이 먼저 잠갔으면 없음. 검사와 잠금이 한 번이라 두
/// 실행이 같은 저장본을 동시에 복구하지 않는다(S2-T7b 리뷰 4 지적 3).
fn claim(content: &Path) -> Option<fs::File> {
    let file = fs::File::open(content).ok()?;
    file.try_lock().ok()?;
    Some(file)
}

/// 끝난 세션의 찌꺼기를 지운다: 내용 파일이 없는 메타(첫 쓰기의 메타 rename 뒤 내용 rename 전에 끝난 세션, 리뷰
/// 4 지적 1)와 저장본이 하나도 남지 않은 끝난 세션의 잠금 파일(크래시마다 쌓이지 않게, 리뷰 2 제안 3).
fn remove_dead_locks(dir: &Path, own: &str) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let names: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    let session_of = |meta: &Path| {
        fs::read_to_string(meta)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|m| m["session"].as_str().map(str::to_owned))
    };
    let mut sessions_in_use: Vec<String> = Vec::new();
    for meta in names
        .iter()
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
    {
        let Some(session) = session_of(meta) else {
            continue;
        };
        // 내용 없는 메타는 살아 있는 세션(두 rename 사이)의 것만 둔다. 끝난 세션의 것은 복구 대상도 아니면서 그
        // 세션을 「쓰는 중」으로 붙잡아 잠금 파일까지 남긴다.
        let orphan = !meta.with_extension("ksq").exists();
        if orphan && session != own && !session_alive(dir, &session) {
            let _ = fs::remove_file(meta);
            continue;
        }
        sessions_in_use.push(session);
    }
    for path in &names {
        let Some(session) = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .and_then(|n| {
                n.strip_prefix("session-")?
                    .strip_suffix(".lock")
                    .map(str::to_owned)
            })
        else {
            continue;
        };
        // 막 만든 잠금 파일은 아직 잠그기 전일 수 있다(`init`이 만들고 잠그는 사이). 오래된 것만 지운다(리뷰 3
        // 지적 1).
        let old = fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age >= STALE_TEMP_AGE);
        if old
            && session != own
            && !sessions_in_use.contains(&session)
            && !session_alive(dir, &session)
        {
            let _ = fs::remove_file(path);
        }
    }
}

fn recover(good: Vec<Recoverable>, host: &Entity<ProjectWindow>, cx: &mut App) {
    let dir = autosave_dir(cx);
    let mut restored = false;
    for item in good {
        // 시트가 떠 있는 동안 다른 실행이 가져갔으면 건드리지 않는다. 창을 열기 전에 내용 파일을 잠가, 같은
        // 저장본을 받은 다른 실행이 같은 순간 [복구]해도 한쪽만 연다(리뷰 4 지적 3).
        let content = dir.join(format!("{}.ksq", item.key));
        if !still_dead(&dir, &item) {
            continue;
        }
        let Some(claim) = claim(&content) else {
            continue;
        };
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
            // 저장본을 이 세션 것으로 가져온다(다시 복구 대상이 되지 않게). 다시 쓰기 전에 내용 파일을 잠가,
            // 쓰기가 실패해도 다른 실행이 가져가지 않게 한다.
            w.autosave.key.clone_from(&item.key);
            remove_generation_temps(&dir, &item.key);
            w.autosave.claim = Some(claim);
            if let Some(job) = w.prepare_autosave(cx) {
                let result = job.write();
                w.finish_autosave(&job, result, cx);
            }
        });
        let _ = handle.update(cx, |_, window, cx| {
            view.read(cx).refresh_file_state(window, cx);
        });
        crate::port_mapping::offer(&view, cx);
        remove_stale_temps(item.path.as_deref(), item.pid);
    }
    remove_dead_locks(&dir, &cx.global::<Session>().id.clone());
    if restored && host.read(cx).document().read(cx).is_pristine() {
        crate::files::close_now_sync(host, cx);
    }
}

fn discard(good: &[Recoverable], cx: &mut App) {
    let dir = autosave_dir(cx);
    for item in good {
        // 시트를 띄운 뒤 다른 실행이 가져간 저장본은 지우지 않는다(리뷰 1 지적 3). 복구하며 잠근 저장본도 그
        // 실행의 것이다(리뷰 4 지적 2).
        if !still_dead(&dir, item) || claimed(&dir.join(format!("{}.ksq", item.key))) {
            continue;
        }
        remove_files(&dir, &item.key);
        remove_generation_temps(&dir, &item.key);
        remove_stale_temps(item.path.as_deref(), item.pid);
    }
    remove_dead_locks(&dir, &cx.global::<Session>().id.clone());
}

/// 저장본이 아직 찾았을 때의 끝난 세션 것인가.
fn still_dead(dir: &Path, item: &Recoverable) -> bool {
    let session = fs::read_to_string(dir.join(format!("{}.json", item.key)))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|m| m["session"].as_str().map(str::to_owned));
    session.as_deref() == Some(item.session.as_str()) && !session_alive(dir, &item.session)
}

/// 끝난 세션이 원래 파일 옆에 남긴 임시 파일(`.<이름>.tmp-<pid>-<n>`)을 지운다. 그 pid의 것만, 그리고
/// [`STALE_TEMP_AGE`]보다 오래된 것만 지운다. pid는 다시 쓰일 수 있어, 같은 pid의 다른 프로그램이 지금 쓰는
/// 임시 파일을 지우지 않게 한다(리뷰 1 제안 7). 쓰기 하나가 그만큼 걸리지는 않는다.
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
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age >= STALE_TEMP_AGE);
        if old && entry.file_name().to_string_lossy().starts_with(&prefix) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

// ---------------------------------------------------------------- 도움 함수

/// 끝난 세션이 쓰다 남긴 세대별 임시 파일(`<키>.ksq.tmp-<세대>`, `<키>.json.tmp-<세대>`)을 지운다(리뷰 3
/// 제안 3). 그 키는 이제 이 세션 것이거나 버렸다.
fn remove_generation_temps(dir: &Path, key: &str) {
    let prefixes = [format!("{key}.ksq.tmp-"), format!("{key}.json.tmp-")];
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if prefixes.iter().any(|p| name.starts_with(p.as_str())) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn remove_files(dir: &Path, key: &str) {
    // 메타를 먼저 지운다. 중간에 멈춰도 메타 없는 내용만 남아 복구 대상이 되지 않는다.
    let _ = fs::remove_file(dir.join(format!("{key}.json")));
    let _ = fs::remove_file(dir.join(format!("{key}.ksq")));
}

fn write_synced(path: &Path, text: &str) -> io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

/// 열린 파일이 지금 `path`의 파일인가.
fn same_inode(file: &fs::File, path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        match (file.metadata(), fs::metadata(path)) {
            (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (file, path);
        true
    }
}

/// 창의 키: 프로세스, 시각, 순번.
fn new_key() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("{}-{nanos}-{n}", std::process::id())
}
