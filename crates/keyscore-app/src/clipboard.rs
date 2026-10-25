//! 노트 클립보드(S3-T3a, REQ 7.5 「클립보드 규칙」, KM-15).
//!
//! 복사한 노트는 앱 전역 내부 클립보드에 구조 그대로 두고, 시스템 클립보드에는 같은 내용을 SMF Type 0 바이트와
//! 계이름 텍스트로 함께 올린다. 붙여넣기는 시스템 클립보드의 `changeCount`가 우리가 쓴 값 그대로면 내부 구조를,
//! 다르면(다른 앱이 바꿈) 시스템 클립보드의 SMF, 없으면 텍스트를 읽는다.
//!
//! GPUI 클립보드는 문자열·이미지만 쓸 수 있어 `NSPasteboard`를 직접 쓴다. 쓴 문자열은 GPUI의
//! `read_from_clipboard`에도 그대로 보인다.

use gpui_kit::{App, Global};
use keyscore_core::{Clip, ModelError, Note, Project, Tick, Track, TrackColor};
use keyscore_midi::{SmfFile, SmfFormat, smf_export, smf_import};
use keyscore_text::Mode;
use keyscore_text::project::{Loss, project_to_text, text_to_project};
use objc2::rc::Retained;
use objc2_app_kit::NSPasteboard;
use objc2_foundation::{NSData, NSString};

/// 계이름 텍스트를 담는 형식(UTI). `NSPasteboardTypeString`과 같은 값이다(정적 상수는 `unsafe`로만 읽혀 문자열을
/// 쓴다).
pub const TEXT_TYPE: &str = "public.utf8-plain-text";
/// SMF 바이트를 담는 형식(UTI, `kUTTypeMIDIAudio`).
pub const MIDI_TYPE: &str = "public.midi-audio";

/// 클립보드 내용: 노트 값(길이·음높이·벨로시티·피치 오프셋·가사·모르는 키)은 그대로, 시작만 가장 이른 노트 기준.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipNotes {
    /// 시작 순으로 정렬, 비지 않음.
    notes: Vec<Note>,
    /// 가장 늦은 끝(첫 노트 시작 기준).
    span: u64,
}

impl ClipNotes {
    /// 시작을 가장 이른 노트 기준으로 옮겨 담는다. 비면 `None`.
    #[must_use]
    pub fn new(notes: Vec<Note>) -> Option<Self> {
        let first = notes.iter().map(|n| n.start().get()).min()?;
        let mut moved: Vec<Note> = notes
            .into_iter()
            .filter_map(|n| n.with_start(Tick::new(n.start().get() - first)).ok())
            .collect();
        moved.sort();
        let span = moved.iter().map(Note::end).max().unwrap_or(0);
        Some(ClipNotes { notes: moved, span })
    }

    #[must_use]
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

    /// 가장 늦은 끝(첫 노트 시작 기준).
    #[must_use]
    pub fn span(&self) -> u64 {
        self.span
    }

    /// 시작을 `by`만큼 민 노트들.
    ///
    /// # Errors
    ///
    /// 틱이 넘치면 [`ModelError::TickOverflow`].
    pub fn shifted(&self, by: u64) -> Result<Vec<Note>, ModelError> {
        self.notes
            .iter()
            .map(|n| {
                let start = n.start().checked_add(by)?;
                n.with_start(start)
            })
            .collect()
    }

    /// 다른 앱에 건넬 임시 프로젝트: 원래 트랙의 이름·채널·프로그램·뱅크로 트랙 하나, 틱 0의 클립 하나(마디 단위).
    /// 템포·박자표는 원래 곡의 `at`(복사 시작) 자리 값이다.
    fn export_project(&self, source: &Project, track: &Track, at: Tick) -> Option<Project> {
        let mut project = Project::new();
        project
            .tempo_map_mut()
            .set(Tick::ZERO, source.tempo_map().tempo_at(at));
        project
            .meter_map_mut()
            .set(Tick::ZERO, source.meter_map().signature_at(at));
        project.set_key(source.key());
        let bar = source.meter_map().signature_at(at).bar_ticks().max(1);
        let mut clip = Clip::new(Tick::ZERO, self.span.div_ceil(bar).max(1) * bar).ok()?;
        for n in &self.notes {
            clip.insert_note(n.clone()).ok()?;
        }
        let mut out = Track::new(track.name(), track.channel(), TrackColor::new(0).ok()?);
        out.set_program(track.program());
        out.set_bank(track.bank());
        out.insert_clip(clip).ok()?;
        project.push_track(out).ok()?;
        Some(project)
    }

    /// SMF Type 0 바이트. 만들지 못하면 `None`.
    #[must_use]
    pub fn to_smf(&self, source: &Project, track: &Track, at: Tick) -> Option<Vec<u8>> {
        let project = self.export_project(source, track, at)?;
        let (file, _) = smf_export(&project, SmfFormat::Single).ok()?;
        Some(file.write())
    }

    /// 계이름 텍스트. 내보내기가 실패하거나 다시 읽으면 달라지는 경우(`Loss::Mismatch`)는 `None`.
    #[must_use]
    pub fn to_text(&self, source: &Project, track: &Track, at: Tick) -> Option<String> {
        let project = self.export_project(source, track, at)?;
        let (text, losses) = project_to_text(&project).ok()?;
        if losses.iter().any(|l| matches!(l, Loss::Mismatch { .. })) {
            return None;
        }
        Some(text)
    }

    /// SMF 바이트를 읽는다(IO-2와 같은 변환). 모든 트랙·클립의 노트를 절대 틱으로 모은다.
    ///
    /// # Errors
    ///
    /// 읽기 실패나 노트 0개는 이유 문구.
    pub fn from_smf(bytes: &[u8]) -> Result<Self, String> {
        let file = SmfFile::read(bytes).map_err(|e| e.to_string())?;
        let (project, _) = smf_import(&file).map_err(|e| e.to_string())?;
        Self::from_project(&project).ok_or_else(|| "SMF에 노트가 없다".to_owned())
    }

    /// 계이름 텍스트를 읽는다(IO-3과 같은 변환, 느슨 모드). 헤더는 버린다.
    ///
    /// # Errors
    ///
    /// 문법 오류나 노트 0개는 이유 문구.
    pub fn from_text(text: &str) -> Result<Self, String> {
        let parsed = text_to_project(text, Mode::Lenient).or_else(|e| {
            // 다른 앱에서 복사한 MML 조각(`c d e`)에는 트랙 머리가 없다. 머리가 없을 때만 트랙 블록 하나로 다시 읽는다.
            if text.contains("@track") {
                Err(e)
            } else {
                text_to_project(&format!("@track Clip ch1\n{text}\n"), Mode::Lenient).map_err(|_| e)
            }
        });
        let (project, _) = parsed.map_err(|e| e.to_string())?;
        Self::from_project(&project).ok_or_else(|| "텍스트에 노트가 없다".to_owned())
    }

    fn from_project(project: &Project) -> Option<Self> {
        let mut notes = Vec::new();
        for track in project.tracks() {
            for (_, clip) in track.clips() {
                for n in clip.notes() {
                    notes.push(
                        n.with_start(n.start().checked_add(clip.start().get()).ok()?)
                            .ok()?,
                    );
                }
            }
        }
        Self::new(notes)
    }
}

/// 시스템 클립보드에서 읽은 것.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SystemContents {
    pub change_count: isize,
    pub midi: Option<Vec<u8>>,
    pub text: Option<String>,
}

/// 앱 전역 클립보드 상태.
#[derive(Default)]
struct ClipboardState {
    /// 마지막으로 복사한 노트.
    internal: Option<ClipNotes>,
    /// 그때 쓴 시스템 클립보드의 `changeCount`.
    written: Option<isize>,
    /// 시험이 쓰는 이름 붙은 pasteboard. 없으면 일반 클립보드.
    name: Option<String>,
}

impl Global for ClipboardState {}

fn pasteboard(cx: &App) -> Retained<NSPasteboard> {
    match cx
        .try_global::<ClipboardState>()
        .and_then(|s| s.name.as_deref())
    {
        Some(name) => NSPasteboard::pasteboardWithName(&NSString::from_str(name)),
        None => NSPasteboard::generalPasteboard(),
    }
}

/// 시험용: 시스템 클립보드 대신 `name`의 pasteboard를 쓴다. 사람의 클립보드를 덮지 않기 위해서다.
pub fn use_named_pasteboard(cx: &mut App, name: &str) {
    cx.default_global::<ClipboardState>().name = Some(name.to_owned());
}

/// 시스템 클립보드를 비우고 `text`·`midi`를 함께 쓴다. 쓴 뒤의 `changeCount`.
pub fn write_system(cx: &App, text: Option<&str>, midi: Option<&[u8]>) -> isize {
    let board = pasteboard(cx);
    board.clearContents();
    if let Some(text) = text {
        board.setString_forType(&NSString::from_str(text), &NSString::from_str(TEXT_TYPE));
    }
    if let Some(midi) = midi {
        board.setData_forType(
            Some(&NSData::with_bytes(midi)),
            &NSString::from_str(MIDI_TYPE),
        );
    }
    board.changeCount()
}

/// 시스템 클립보드를 읽는다.
#[must_use]
pub fn read_system(cx: &App) -> SystemContents {
    let board = pasteboard(cx);
    SystemContents {
        change_count: board.changeCount(),
        midi: board
            .dataForType(&NSString::from_str(MIDI_TYPE))
            .map(|d| d.to_vec()),
        text: board
            .stringForType(&NSString::from_str(TEXT_TYPE))
            .map(|s| s.to_string()),
    }
}

/// 복사: 내부 클립보드에 `notes`를 두고 시스템 클립보드에 `text`(계이름 텍스트)와 `midi`(SMF)를 함께 올린다.
pub fn store(cx: &mut App, notes: ClipNotes, text: Option<&str>, midi: Option<&[u8]>) {
    let count = write_system(cx, text, midi);
    let state = cx.default_global::<ClipboardState>();
    state.internal = Some(notes);
    state.written = Some(count);
}

/// 붙여넣을 내용. 시스템 클립보드가 우리가 쓴 그대로면 내부 구조, 아니면 SMF, 없으면 텍스트.
///
/// # Errors
///
/// 붙여넣을 노트가 없으면 이유 문구.
pub fn paste_contents(cx: &App) -> Result<ClipNotes, String> {
    let system = read_system(cx);
    if let Some(state) = cx.try_global::<ClipboardState>()
        && state.written == Some(system.change_count)
        && let Some(notes) = &state.internal
    {
        return Ok(notes.clone());
    }
    if let Some(midi) = &system.midi {
        return ClipNotes::from_smf(midi);
    }
    if let Some(text) = &system.text {
        return ClipNotes::from_text(text);
    }
    Err("클립보드가 비었다".to_owned())
}

/// 시험용: 내부 클립보드.
#[must_use]
pub fn internal(cx: &App) -> Option<ClipNotes> {
    cx.try_global::<ClipboardState>()
        .and_then(|s| s.internal.clone())
}
