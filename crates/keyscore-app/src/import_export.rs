//! SMF·계이름 텍스트 가져오기·내보내기(S2-T7c, IO-2, IO-3, 결정 `2026-10-22-s2-t7c-import-export`).
//!
//! - SMF 가져오기: 백그라운드에서 읽고 바꿔 새 프로젝트 창에 둔다(손대지 않은 빈 창이면 바꾼다). 가사 붙이기가
//!   큰 파일에서 오래 걸린다(S1-T6 이월).
//! - 계이름 텍스트 가져오기: 지금 문서에 undo 하나로 넣는다. 위치는 재생 위치(S3에서 커서로 바꾼다).
//! - 내보내기 둘: 문서를 바꾸지 않고 쓰고, 잃은 정보를 상태줄에 요약한다.
//!
//! 네 흐름 모두 창의 파일 흐름 하나(`file_flow`)를 지킨다.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use gpui_kit::{
    AppContext as _, AsyncApp, Context, Entity, PathPromptOptions, PromptButton, PromptLevel,
    Window,
};
use keyscore_core::{Clip, Edit, ModelError, Tick, Track, TrackColor};
use keyscore_midi::{
    ExportReport, ImportError, ImportReport, MAX_SONG_TICKS, SmfError, SmfFile, SmfFormat,
    smf_export, smf_import,
};
use keyscore_text::project::{Loss, project_to_text};
use keyscore_text::{Mode, Parsed};

use crate::actions::file;
use crate::document::ProjectDocument;
use crate::files::{self, ask, default_directory, file_name, write_bytes_atomic};
use crate::window::ProjectWindow;

const OK: &str = "확인";

/// 계이름 텍스트 가져오기의 결과.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextImportSummary {
    /// 넣은 블록(클립) 수.
    pub blocks: usize,
    /// 새로 만든 트랙 수.
    pub new_tracks: usize,
    /// 넣은 노트 수.
    pub notes: usize,
}

/// 읽은 텍스트를 문서에 undo 하나로 넣는다. `@track` 이름이 같은 트랙이 있고 그 트랙의 클립과 겹치지 않으면
/// 그 트랙에, 아니면 새 트랙(겹치면 「이름 2」처럼 비어 있는 이름)에 둔다. 클립은 `at`에서 시작한다.
/// 헤더는 반영하지 않는다(결정 2 A).
///
/// # Errors
///
/// 모델 불변식을 어기면(틱 넘침 등) [`ModelError`]. 이때 문서는 바뀌지 않는다.
pub fn import_text_into(
    document: &mut ProjectDocument,
    parsed: &Parsed,
    at: Tick,
) -> Result<TextImportSummary, ModelError> {
    let project = document.project();
    let mut names: Vec<String> = project
        .tracks()
        .iter()
        .map(|t| t.name().to_owned())
        .collect();
    let mut summary = TextImportSummary::default();
    let mut edits = Vec::new();
    for block in parsed.tracks() {
        let source = block.clip();
        let length = source.length();
        let mut clip = Clip::new(at, length)?;
        for note in source.notes() {
            clip.insert_note(note.clone())?;
        }
        summary.blocks += 1;
        summary.notes += source.notes().len();
        let end = at.get().saturating_add(length);
        // 같은 트랙의 클립은 겹칠 수 없다(core 불변식). 겹치면 새 트랙에 둔다.
        let target = project
            .tracks()
            .iter()
            .find(|t| t.name() == block.name())
            .filter(|t| {
                t.clips().all(|(_, c)| {
                    c.start().get().saturating_add(c.length()) <= at.get() || c.start().get() >= end
                })
            });
        let track_id = if let Some(track) = target {
            track.id()
        } else {
            let name = free_name(block.name(), &names);
            names.push(name.clone());
            let index = project.tracks().len() + summary.new_tracks;
            let color = TrackColor::new(
                u8::try_from(index % usize::from(TrackColor::PALETTE_LEN)).unwrap_or(0),
            )?;
            let mut track = Track::new(name, block.channel(), color);
            track.set_program(block.program());
            track.set_bank(block.bank());
            let id = track.id();
            edits.push(Edit::InsertTrack {
                index,
                track: Box::new(track),
            });
            summary.new_tracks += 1;
            id
        };
        edits.push(Edit::InsertClip {
            track: track_id,
            clip: Box::new(clip),
        });
    }
    document.apply(edits)?;
    Ok(summary)
}

/// `name`이 없으면 그대로, 있으면 「name 2」, 「name 3」 가운데 비어 있는 첫 이름.
fn free_name(name: &str, taken: &[String]) -> String {
    if !taken.iter().any(|t| t == name) {
        return name.to_owned();
    }
    // 이름이 n개면 2..=n+1 안에 빈 자리가 반드시 있다.
    (2..=taken.len() + 1)
        .map(|n| format!("{name} {n}"))
        .find(|candidate| !taken.contains(candidate))
        .unwrap_or_else(|| name.to_owned())
}

// ---------------------------------------------------------------- 창 처리기

impl ProjectWindow {
    pub(crate) fn file_import_smf(
        &mut self,
        _: &file::ImportSmf,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_file_flow(cx, async |view, cx| import_smf(view, cx).await);
    }

    pub(crate) fn file_export_smf(
        &mut self,
        _: &file::ExportSmf,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_file_flow(cx, async |view, cx| export_smf(view, cx).await);
    }

    pub(crate) fn file_import_text(
        &mut self,
        _: &file::ImportText,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_file_flow(cx, async |view, cx| import_text(view, cx).await);
    }

    pub(crate) fn file_export_text(
        &mut self,
        _: &file::ExportText,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_file_flow(cx, async |view, cx| export_text(view, cx).await);
    }
}

// ---------------------------------------------------------------- 흐름

async fn import_smf(view: &Entity<ProjectWindow>, cx: &mut AsyncApp) {
    let Some(path) = pick_file(cx).await else {
        return;
    };
    let name = file_name(&path);
    say(view, format!("가져오는 중: {name}"), cx);
    let read = path.clone();
    let result = cx
        .background_spawn(async move {
            let bytes = std::fs::read(&read).map_err(|e| e.to_string())?;
            let file = SmfFile::read(&bytes).map_err(|e| smf_error_text(&e))?;
            smf_import(&file).map_err(|e| match e {
                ImportError::Smf(e) => smf_error_text(&e),
                ImportError::Model(e) => e.to_string(),
            })
        })
        .await;
    let (project, report) = match result {
        Ok(ok) => ok,
        Err(detail) => {
            fail(view, &format!("“{name}”을 가져오지 못했다"), &detail, cx).await;
            return;
        }
    };
    let stem = path
        .file_stem()
        .map_or_else(|| name.clone(), |s| s.to_string_lossy().into_owned());
    let opened = cx.update(|cx| {
        let (_, imported) = crate::open_project_window(project, cx).ok()?;
        imported.update(cx, |w, cx| {
            w.document().update(cx, |d, _| d.mark_imported(stem));
            w.say(import_summary(&name, &report), cx);
        });
        files::refresh(&imported, cx);
        Some(())
    });
    if opened.is_none() {
        fail(
            view,
            &format!("“{name}”을 가져오지 못했다"),
            "창을 열 수 없다",
            cx,
        )
        .await;
        return;
    }
    // 이 흐름을 띄운 창이 손대지 않은 빈 창이면 가져온 창으로 바꾼다(열기와 같다).
    cx.update(|cx| {
        if view.read(cx).document().read(cx).is_pristine() {
            files::close_now_sync(view, cx);
        }
    });
}

async fn export_smf(view: &Entity<ProjectWindow>, cx: &mut AsyncApp) {
    let Some(path) = pick_new_path(view, "mid", cx).await else {
        return;
    };
    let name = file_name(&path);
    let result = cx.update(|cx| {
        let (file, report) = smf_export(
            view.read(cx).document().read(cx).project(),
            SmfFormat::Parallel,
        )
        .map_err(|e| smf_error_text(&e))?;
        write_bytes_atomic(&path, &file.write()).map_err(|e| e.to_string())?;
        Ok::<_, String>(report)
    });
    match result {
        Ok(report) => say(view, export_smf_summary(&name, &report), cx),
        Err(detail) => fail(view, &format!("“{name}”으로 내보내지 못했다"), &detail, cx).await,
    }
}

async fn import_text(view: &Entity<ProjectWindow>, cx: &mut AsyncApp) {
    let Some(path) = pick_file(cx).await else {
        return;
    };
    let name = file_name(&path);
    let parsed = std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|text| keyscore_text::parse(&text, Mode::Lenient).map_err(|e| e.to_string()));
    let parsed = match parsed {
        Ok(p) => p,
        Err(detail) => {
            fail(view, &format!("“{name}”을 가져오지 못했다"), &detail, cx).await;
            return;
        }
    };
    let result = cx.update(|cx| {
        let at = view.read(cx).transport.position();
        let document = view.read(cx).document().clone();
        document.update(cx, |d, cx| {
            let result = import_text_into(d, &parsed, at);
            cx.notify();
            result
        })
    });
    match result {
        Ok(summary) => say(view, text_import_summary(&name, &summary, &parsed), cx),
        Err(e) => {
            fail(
                view,
                &format!("“{name}”을 가져오지 못했다"),
                &e.to_string(),
                cx,
            )
            .await;
        }
    }
}

async fn export_text(view: &Entity<ProjectWindow>, cx: &mut AsyncApp) {
    let Some(path) = pick_new_path(view, "txt", cx).await else {
        return;
    };
    let name = file_name(&path);
    let result = cx.update(|cx| {
        let (text, losses) = project_to_text(view.read(cx).document().read(cx).project())
            .map_err(|e| e.to_string())?;
        write_bytes_atomic(&path, text.as_bytes()).map_err(|e| e.to_string())?;
        Ok::<_, String>(losses)
    });
    match result {
        Ok(losses) => say(view, export_text_summary(&name, &losses), cx),
        Err(detail) => fail(view, &format!("“{name}”으로 내보내지 못했다"), &detail, cx).await,
    }
}

// ---------------------------------------------------------------- 요약 문구

fn import_summary(name: &str, report: &ImportReport) -> String {
    let mut dropped = Vec::new();
    if report.lane_points > 0 {
        dropped.push(format!(
            "컨트롤러·벤드·프레셔 포인트 {}개(레인은 S6에서 옮긴다)",
            report.lane_points
        ));
    }
    if report.unattached_lyrics > 0 {
        dropped.push(format!(
            "노트 시작에 붙지 않은 가사 {}개",
            report.unattached_lyrics
        ));
    }
    if report.orphan_lyrics > 0 {
        dropped.push(format!("붙일 트랙이 없는 가사 {}개", report.orphan_lyrics));
    }
    if report.extra_programs > 0 {
        dropped.push(format!(
            "두 번째 뒤의 프로그램 변경 {}개",
            report.extra_programs
        ));
    }
    if report.skipped_events > 0 {
        dropped.push(format!(
            "SysEx·폴리 프레셔·기타 메타 {}개",
            report.skipped_events
        ));
    }
    let mut text = format!(
        "가져옴: {name} — 트랙 {}개, 노트 {}개",
        report.tracks, report.notes
    );
    if !dropped.is_empty() {
        text.push_str(". 버림: ");
        text.push_str(&dropped.join(", "));
    }
    text
}

fn export_smf_summary(name: &str, report: &ExportReport) -> String {
    let mut lost = Vec::new();
    if report.hidden_tails > 0 {
        lost.push(format!(
            "클립 끝에서 자른 노트 꼬리 {}개",
            report.hidden_tails
        ));
    }
    if report.pitch_offsets > 0 {
        lost.push(format!("피치 오프셋 {}개", report.pitch_offsets));
    }
    if report.velocity_rounded > 0 {
        lost.push(format!(
            "7비트로 반올림한 벨로시티 {}개",
            report.velocity_rounded
        ));
    }
    if report.empty_tracks > 0 {
        lost.push(format!(
            "노트·프로그램이 없어 다른 프로그램에서 사라질 수 있는 트랙 {}개",
            report.empty_tracks
        ));
    }
    let mut text = format!("내보냄: {name}");
    if !lost.is_empty() {
        text.push_str(". 잃은 정보: ");
        text.push_str(&lost.join(", "));
    }
    text
}

fn text_import_summary(name: &str, summary: &TextImportSummary, parsed: &Parsed) -> String {
    let mut text = format!(
        "가져옴: {name} — 클립 {}개(새 트랙 {}개), 노트 {}개",
        summary.blocks, summary.new_tracks, summary.notes
    );
    let header = parsed.header();
    let ignored: Vec<&str> = [
        header.tempo().map(|_| "@tempo"),
        header.time().map(|_| "@time"),
        header.key().map(|_| "@key"),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !ignored.is_empty() {
        let _ = write!(text, ". 헤더 {}는 반영하지 않았다", ignored.join("·"));
    }
    let warnings = parsed.warnings().len();
    if warnings > 0 {
        let _ = write!(text, ". 경고 {warnings}개(건너뛴 토큰 등)");
    }
    text
}

/// 잃은 정보의 줄 수가 많으면 앞의 몇 줄만 보인다(상태줄 한 줄).
const MAX_LOSS_LINES: usize = 3;

fn export_text_summary(name: &str, losses: &[Loss]) -> String {
    let mut text = format!("내보냄: {name}");
    if !losses.is_empty() {
        let shown: Vec<String> = losses
            .iter()
            .take(MAX_LOSS_LINES)
            .map(ToString::to_string)
            .collect();
        let _ = write!(
            text,
            ". 잃은 정보 {}가지: {}",
            losses.len(),
            shown.join("; ")
        );
        if losses.len() > MAX_LOSS_LINES {
            text.push_str(" …");
        }
    }
    text
}

/// SMF 오류 문구. 너무 긴 곡은 상한과 시간으로 알린다(S1-T6 이월).
fn smf_error_text(error: &SmfError) -> String {
    match error {
        SmfError::TooLong { tick } => {
            // 120 BPM(4분음표 0.5 s)에서의 길이. 템포 맵을 읽기 전의 오류라 대표 템포로 보인다.
            let hours = |t: u64| t / 960 / 2 / 3600;
            format!(
                "곡이 너무 길다: 끝이 {tick}틱(PPQ 960)으로 상한 {MAX_SONG_TICKS}틱을 넘는다. 120 BPM이면 약 {}시간으로, 상한은 약 {}시간이다",
                hours(*tick),
                hours(MAX_SONG_TICKS)
            )
        }
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------- 도움 함수

async fn pick_file(cx: &mut AsyncApp) -> Option<PathBuf> {
    let chosen = cx.update(|cx| {
        cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("가져오기".into()),
        })
    });
    chosen.await.ok()?.ok()??.into_iter().next()
}

/// 내보낼 경로를 묻는다. 지금 파일의 폴더와 이름 줄기(없으면 가져온 이름, 「제목 없음」)에 `extension`.
async fn pick_new_path(
    view: &Entity<ProjectWindow>,
    extension: &str,
    cx: &mut AsyncApp,
) -> Option<PathBuf> {
    let (directory, stem) = cx.update(|cx| {
        let d = view.read(cx).document().read(cx);
        (
            d.path()
                .and_then(Path::parent)
                .map_or_else(default_directory, Path::to_path_buf),
            d.suggested_stem(),
        )
    });
    let chosen =
        cx.update(|cx| cx.prompt_for_new_path(&directory, Some(&format!("{stem}.{extension}"))));
    chosen.await.ok()?.ok()?
}

fn say(view: &Entity<ProjectWindow>, message: String, cx: &mut AsyncApp) {
    cx.update(|cx| view.update(cx, |w, cx| w.say(message, cx)));
}

async fn fail(view: &Entity<ProjectWindow>, message: &str, detail: &str, cx: &mut AsyncApp) {
    let handle = cx.update(|cx| view.read(cx).window_handle());
    ask(
        handle,
        PromptLevel::Critical,
        message,
        Some(detail),
        &[PromptButton::ok(OK)],
        cx,
    )
    .await;
}
