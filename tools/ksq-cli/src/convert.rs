use std::collections::HashSet;
use std::fmt;

use keyscore_core::{
    Clip, MeterMap, Note, PitchOffset, Project, TempoMap, Tick, Track, TrackColor, Velocity, ksq,
};
use keyscore_text::{Diagnostic, ExportTrack, Header, Mode, export, parse};
use serde_json::{Map, Value};

use crate::CliError;

/// 계이름 텍스트 파서가 블록을 시작할 때의 벨로시티(MIDI 1.0). `keyscore-text`의 `START_VELOCITY`와
/// 같아야 한다. 어긋나면 손실 없는 프로젝트의 바이트 왕복 테스트가 깨져 드러난다.
const TEXT_VELOCITY: u8 = 100;

/// `ksq2txt`가 텍스트로 옮기지 못한 정보 한 가지.
///
/// 텍스트는 헤더 하나(템포·박자·조)와 트랙마다 시작 0인 클립 하나, 노트의 (시작, 길이, 음높이,
/// 가사)만 나른다(REQ 5.14). 손실이 하나도 없으면 `txt2ksq`로 되읽은 프로젝트의 `.ksq`가 원본과
/// 바이트까지 같다. `track`은 0부터 센 트랙 순번이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loss {
    /// 틱 0 뒤의 템포 변화.
    TempoChanges { dropped: usize },
    /// 틱 0 뒤의 박자표 변화.
    MeterChanges { dropped: usize },
    /// 클립이 여럿이거나 0에서 시작하지 않아, 시작 0 클립 하나로 합쳤다. 빈 곳은 쉼표가 된다.
    ClipLayout {
        track: usize,
        name: String,
        clips: usize,
        first_start: u64,
    },
    /// 클립 이름·색.
    ClipAttributes { track: usize, name: String },
    /// 클립 끝을 넘는 노트 꼬리(분할로 생긴 숨김 꼬리)를 잘랐다.
    HiddenTails {
        track: usize,
        name: String,
        notes: usize,
    },
    /// 벨로시티가 텍스트 기본값(MIDI 1.0 100)이 아닌 노트. 내보내기는 `v`를 쓰지 않는다.
    Velocities {
        track: usize,
        name: String,
        notes: usize,
    },
    /// 피치 오프셋이 있는 노트.
    PitchOffsets {
        track: usize,
        name: String,
        notes: usize,
    },
    /// 텍스트가 나르지 않는 트랙 속성 가운데 `txt2ksq`의 기본값과 다른 것.
    TrackAttributes {
        track: usize,
        name: String,
        attributes: Vec<&'static str>,
    },
    /// 이 도구가 모르는 `.ksq` 키(경로 목록).
    UnknownKeys { paths: Vec<String> },
    /// 클립이 없는 트랙. 텍스트 블록은 비어 있을 수 없어 한 마디 쉼표 클립으로 적었다.
    EmptyTrack { track: usize, name: String },
    /// 같은 이름의 트랙은 텍스트에서 한 트랙의 성부로 합쳐지므로(결정 Q2) 이름을 바꿨다.
    RenamedTrack {
        track: usize,
        from: String,
        to: String,
    },
    /// 클립 길이가 쉼표로 닿지 않아 가장 가까운 길이로 적었다
    /// (`docs/decisions/2026-10-12-ktext-implementation-details.md`).
    ClipLength {
        track: usize,
        name: String,
        from: u64,
        to: u64,
    },
    /// 되읽은 결과가 위 손실로 설명되지 않게 달라졌다. `keyscore-text`가 어긋난 것이다.
    Mismatch { detail: String },
}

/// 사람에게 보이는 트랙 표시. 이름에 줄바꿈·따옴표가 있어도 한 줄로 보이게 `Debug`로 적는다.
struct TrackLabel<'a>(usize, &'a str);

impl fmt::Display for TrackLabel<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "트랙 {} {:?}", self.0 + 1, self.1)
    }
}

impl fmt::Display for Loss {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Loss::TempoChanges { dropped } => write!(
                f,
                "템포 변화 {dropped}개를 잃는다(틱 0의 템포만 헤더로 간다)"
            ),
            Loss::MeterChanges { dropped } => write!(
                f,
                "박자표 변화 {dropped}개를 잃는다(틱 0의 박자표만 헤더로 간다)"
            ),
            Loss::ClipLayout {
                track,
                name,
                clips,
                first_start,
            } => write!(
                f,
                "{}: 클립 {clips}개(첫 시작 {first_start}틱)를 시작 0 클립 하나로 합친다",
                TrackLabel(*track, name)
            ),
            Loss::ClipAttributes { track, name } => {
                write!(f, "{}: 클립 이름·색을 잃는다", TrackLabel(*track, name))
            }
            Loss::HiddenTails { track, name, notes } => write!(
                f,
                "{}: 클립 끝을 넘는 노트 꼬리 {notes}개를 자른다",
                TrackLabel(*track, name)
            ),
            Loss::Velocities { track, name, notes } => write!(
                f,
                "{}: 노트 {notes}개의 벨로시티를 잃는다(텍스트는 {TEXT_VELOCITY}으로 읽힌다)",
                TrackLabel(*track, name)
            ),
            Loss::PitchOffsets { track, name, notes } => write!(
                f,
                "{}: 노트 {notes}개의 피치 오프셋을 잃는다",
                TrackLabel(*track, name)
            ),
            Loss::TrackAttributes {
                track,
                name,
                attributes,
            } => write!(
                f,
                "{}: 트랙 속성을 잃는다: {}",
                TrackLabel(*track, name),
                attributes.join(", ")
            ),
            Loss::UnknownKeys { paths } => write!(
                f,
                "모르는 키 {}개를 잃는다: {}",
                paths.len(),
                paths.join(", ")
            ),
            Loss::EmptyTrack { track, name } => write!(
                f,
                "{}: 클립이 없어 한 마디 쉼표 클립으로 쓴다",
                TrackLabel(*track, name)
            ),
            Loss::RenamedTrack { track, from, to } => write!(
                f,
                "{}: 같은 이름의 트랙이 있어 {to:?}(으)로 바꾼다",
                TrackLabel(*track, from)
            ),
            Loss::ClipLength {
                track,
                name,
                from,
                to,
            } => write!(
                f,
                "{}: 클립 길이 {from}틱을 쉼표로 닿는 {to}틱으로 쓴다",
                TrackLabel(*track, name)
            ),
            Loss::Mismatch { detail } => {
                write!(
                    f,
                    "되읽은 결과가 원본과 다르다(keyscore-text 버그): {detail}"
                )
            }
        }
    }
}

/// `txt2ksq`가 `index`번째 트랙에 주는 색. 텍스트는 색을 나르지 않으므로 팔레트를 차례로 돈다.
fn default_color(index: usize) -> Result<TrackColor, CliError> {
    let palette = usize::from(TrackColor::PALETTE_LEN);
    // index % palette < 12라 u8에 들어간다.
    let i = u8::try_from(index % palette).unwrap_or(0);
    Ok(TrackColor::new(i)?)
}

/// 출력 텍스트 크기 상한의 배수. S0-T5가 텍스트 → 텍스트에 지킨 「입력에 선형, 256배 이내」와
/// 맞춘다. 먼 틱의 클립을 시작 0 클립으로 합치면 빈 곳이 쉼표로 풀려 작은 파일이 거대한 텍스트가
/// 되므로(S0-T8 리뷰 1 지적 1) 상한이 필요하다.
///
/// 기준은 「텍스트가 나르는 내용만으로 다시 만든 프로젝트」(`txt2ksq`가 만들 프로젝트)의 정규
/// `.ksq` 바이트다. 원본의 정규 `.ksq`를 기준으로 하면 텍스트로 가지 않는 것(모르는 키, 벨로시티
/// 등)으로 상한을 부풀릴 수 있다(S0-T8 리뷰 2 지적 1).
pub const MAX_EXPANSION: u64 = 256;

/// 어림에 쓰는 바이트 상수. 어림은 keyscore-text의 성부 배정 규칙을 흉내 내지 않고, 그 규칙이
/// 무엇이든 성립하는 위쪽 한계로 잡는다(S0-T8 리뷰 3 지적 2). 논증은 [`check_estimate`]에 있다.
///
/// - 쉼표: 한 성부에서 온음표 하나 길이의 쉼표는 `r1 |`와 줄바꿈으로 많아야 5바이트다. 박자표
///   n/d(n 1–16, d 1–256)와 노트 위치를 바꿔 잰 최댓값이 성부당 온음표 하나에 5.07바이트였다.
///   마디가 온음표보다 짧으면 마디선 없이 `r1`만 줄에 써 3바이트 남짓이다.
/// - 성부의 나머지 쉼표: 온음표보다 짧은 끝 쉼표(점·겹점 64분까지 여러 개)와 그 마디선.
/// - 노트(사건): 타이 사슬 16조각 × (화음 음높이와 옥타브 기호, 길이, `&`), `~` 보정, 사건 앞의
///   온음표보다 짧은 쉼표와 마디선. 사건은 노트보다 많지 않으므로 노트마다 센다.
/// - 성부 블록 헤더 줄: `@track` + 따옴표 이스케이프한 이름(많아야 두 배) + ` ch16 prog127
///   bank16383` + 빈 줄.
const HEADER_BYTES: u128 = 128;
const VOICE_LINE_BYTES: u128 = 48;
const VOICE_REST_TAIL_BYTES: u128 = 64;
const NOTE_BYTES: u128 = 320;
const REST_BYTES_PER_WHOLE: u128 = 6;
const WHOLE_TICKS: u64 = 3840;

/// 프로젝트를 계이름 텍스트로 쓴다. 텍스트가 나르지 않는 정보는 [`Loss`]로 돌려준다.
///
/// 헤더는 늘 셋 다 적는다(틱 0의 템포·박자표와 조). 쓴 텍스트를 다시 읽어 원본과 대조하므로,
/// 클립 길이 반올림처럼 내보내기가 바꾼 것도 손실로 나온다. 텍스트 크기는 텍스트가 나르는
/// 내용만으로 다시 만든 프로젝트의 정규 `.ksq` 바이트의 [`MAX_EXPANSION`]배를 넘지 않는다.
///
/// # Errors
///
/// 텍스트가 상한을 넘을 것이면 [`CliError::OutputTooLarge`](텍스트를 만들기 전에 어림해 거른다),
/// 내보낸 텍스트를 다시 읽지 못하면 [`CliError::ExportUnreadable`], 노트를 합친 클립을 만들 수
/// 없으면 [`CliError::Model`].
pub fn project_to_text(project: &Project) -> Result<(String, Vec<Loss>), CliError> {
    to_text(project, None)
}

/// [`project_to_text`]와 같되 상한을 `max_bytes`로 바꾼다(기본 상한보다 크거나 작아도 된다).
/// `ksq2txt --max-output`이 쓴다. 기본 상한은 위쪽 한계 어림이라 아주 긴 곡을 거부할 수 있다.
///
/// # Errors
///
/// [`project_to_text`]와 같다.
pub fn project_to_text_limited(
    project: &Project,
    max_bytes: u64,
) -> Result<(String, Vec<Loss>), CliError> {
    to_text(project, Some(max_bytes))
}

fn to_text(project: &Project, max_bytes: Option<u64>) -> Result<(String, Vec<Loss>), CliError> {
    let mut losses = Vec::new();
    let tempo_changes = project.tempo_map().changes().count().saturating_sub(1);
    if tempo_changes > 0 {
        losses.push(Loss::TempoChanges {
            dropped: tempo_changes,
        });
    }
    let meter_changes = project.meter_map().changes().count().saturating_sub(1);
    if meter_changes > 0 {
        losses.push(Loss::MeterChanges {
            dropped: meter_changes,
        });
    }
    let unknown = unknown_keys(&ksq::to_string(project));
    if !unknown.is_empty() {
        losses.push(Loss::UnknownKeys { paths: unknown });
    }

    let time = project.meter_map().signature_at(Tick::ZERO);
    let header = Header::new(
        Some(project.tempo_map().tempo_at(Tick::ZERO)),
        Some(time),
        Some(project.key()),
    );
    // 합친 클립은 노트 수에 비례하는 메모리만 쓴다(빈 곳은 길이 숫자 하나다). 쉼표로 풀리는 것은
    // 내보낼 때뿐이라, 여기까지 만든 뒤 어림해도 「만들기 전에 거른다」가 지켜진다.
    let names = text_names(project, &mut losses);
    let mut clips = Vec::with_capacity(project.tracks().len());
    for (i, (track, name)) in project.tracks().iter().zip(&names).enumerate() {
        track_attribute_losses(i, track, name, &mut losses)?;
        clips.push(merged_clip(i, track, name, time.bar_ticks(), &mut losses)?);
    }
    let tracks: Vec<ExportTrack<'_>> = project
        .tracks()
        .iter()
        .zip(&names)
        .zip(&clips)
        .map(|((t, name), clip)| ExportTrack {
            name,
            channel: t.channel(),
            program: t.program(),
            bank: t.bank(),
            clip,
        })
        .collect();
    let limit = if let Some(m) = max_bytes {
        m
    } else {
        let base = ksq::to_string(&projection(&header, &tracks)?).len();
        MAX_EXPANSION.saturating_mul(u64::try_from(base).unwrap_or(u64::MAX))
    };
    check_estimate(&tracks, limit)?;
    let text = export(&header, &tracks);
    // 어림이 모자란 경우의 안전망. 되읽기(verify) 전에 거른다.
    let actual = u64::try_from(text.len()).unwrap_or(u64::MAX);
    if actual > limit {
        return Err(CliError::OutputTooLarge {
            estimated: actual,
            limit,
            detail: "어림보다 실제 출력이 컸다".to_owned(),
        });
    }
    verify(&text, &header, &tracks, &mut losses)?;
    Ok((text, losses))
}

/// 텍스트가 나르는 내용만으로 만든 프로젝트. [`text_to_project`]가 이 텍스트를 읽어 만들 것과
/// 같다(클립 길이 반올림만 빼고).
fn projection(header: &Header, tracks: &[ExportTrack<'_>]) -> Result<Project, CliError> {
    let mut p = Project::new();
    if let Some(tempo) = header.tempo() {
        *p.tempo_map_mut() = TempoMap::new(tempo);
    }
    if let Some(time) = header.time() {
        *p.meter_map_mut() = MeterMap::new(time);
    }
    if let Some(key) = header.key() {
        p.set_key(key);
    }
    for (i, t) in tracks.iter().enumerate() {
        let mut track = Track::new(t.name, t.channel, default_color(i)?);
        track.set_program(t.program);
        track.set_bank(t.bank);
        track.insert_clip(t.clip.clone())?;
        p.push_track(track)?;
    }
    Ok(p)
}

/// 텍스트를 만들기 전에 크기의 위쪽 한계를 구해 상한을 넘으면 거부한다. 틱은 u64라 u128로 센다.
///
/// 위쪽 한계인 까닭(트랙 하나, 사건 수 E ≤ 노트 수 N, 합친 클립 길이 L):
///
/// 1. keyscore-text는 트랙을 성부 블록들로 쓴다. 성부는 사건을 하나 이상 담을 때만 생기고, 그 밖에는
///    클립 끝을 채우는 쉼표 성부가 하나 더 생길 수 있다(사건이 없으면 빈 성부 하나). 그래서 성부 수
///    V ≤ max(N, 1) + 1이다. 몇 개를 예산 안에서 열고 몇 개를 접는지는 보지 않는다.
/// 2. 성부마다 커서는 클립 시작에서 앞으로만 가고 클립 끝(커서 앞지름 한도 480틱 포함)을 넘지
///    않는다. 그래서 한 성부의 온음표 쉼표는 L / 3840 + 1개 이하이고, 온음표보다 짧은 쉼표는
///    사건 앞과 성부 끝에서만 나온다.
/// 3. 각 성부는 헤더 줄 하나를 쓴다. 각 사건은 정확히 한 성부에서 한 번 쓰인다.
///
/// 따라서 출력은 「헤더 + V × (헤더 줄 + 나머지 쉼표 + (L / 3840 + 1) × 온음표 쉼표) + N × 노트 +
/// 2 × 가사 바이트」를 넘지 않는다. 한 자리에 사건이 몰려 성부가 많이 열리는 클립(리뷰 3의 crammed)도 V가
/// 사건 수로 묶이므로 이 식 안이다. 대가로, 노트가 많고 아주 긴 곡(대략 1,400마디 넘게 촘촘한
/// 트랙)은 실제로는 상한 안이어도 거부될 수 있다. 그때는 `ksq2txt --max-output`으로 상한을 올린다.
fn check_estimate(tracks: &[ExportTrack<'_>], limit: u64) -> Result<(), CliError> {
    let mut total = HEADER_BYTES;
    let mut biggest: Option<(u128, usize, &str, u64, u128)> = None;
    for (i, t) in tracks.iter().enumerate() {
        let end = t.clip.length();
        let wholes = u128::from(end / WHOLE_TICKS) + 1;
        let notes = t.clip.notes().len() as u128;
        let lyric_bytes: u128 = t
            .clip
            .notes()
            .iter()
            .map(|n| n.lyric().map_or(0, |l| l.len() as u128))
            .sum();
        let voices = notes.max(1) + 1;
        let per_voice = 2 * t.name.len() as u128
            + VOICE_LINE_BYTES
            + VOICE_REST_TAIL_BYTES
            + wholes * REST_BYTES_PER_WHOLE;
        let bytes = voices * per_voice + notes * NOTE_BYTES + 2 * lyric_bytes;
        total += bytes;
        if biggest.is_none_or(|(b, ..)| bytes > b) {
            biggest = Some((bytes, i, t.name, end, voices));
        }
    }
    if total <= u128::from(limit) {
        return Ok(());
    }
    let detail = biggest.map_or_else(String::new, |(bytes, i, name, end, voices)| {
        format!(
            "가장 큰 곳은 {}: 시작 0으로 합친 클립이 {end}틱(온음표 {}개 분량의 쉼표), 성부 많아야 {voices}개, 많아야 {bytes}바이트",
            TrackLabel(i, name),
            end / WHOLE_TICKS
        )
    });
    // 어림이 u64를 넘으면 어떤 `--max-output`도 통과하지 않는다. u64::MAX로 잘라 「그 값 이상을
    // 주면 변환한다」고 안내하면 지킬 수 없는 약속이 된다(S0-T8 리뷰 5 지적 2).
    match u64::try_from(total) {
        Ok(estimated) => Err(CliError::OutputTooLarge {
            estimated,
            limit,
            detail,
        }),
        Err(_) => Err(CliError::OutputBeyondAnyLimit { detail }),
    }
}

/// 텍스트에 쓸 트랙 이름. 텍스트는 같은 이름의 블록을 한 트랙으로 합치므로(결정 Q2), 앞서 나온
/// 이름과 같으면 `이름 (2)`처럼 번호를 붙인다. 원래 있는 다른 트랙 이름과도 겹치지 않게 고른다.
fn text_names(project: &Project, losses: &mut Vec<Loss>) -> Vec<String> {
    let originals: HashSet<&str> = project.tracks().iter().map(Track::name).collect();
    let mut used: HashSet<String> = HashSet::new();
    let mut out = Vec::with_capacity(project.tracks().len());
    for (i, t) in project.tracks().iter().enumerate() {
        let name = t.name();
        if used.insert(name.to_owned()) {
            out.push(name.to_owned());
            continue;
        }
        // 피해야 할 이름은 원래 이름 n개와 앞서 쓴 이름 n개뿐이라 2..=2n+2 안에 빈 번호가 있다.
        let limit = 2 * project.tracks().len() + 2;
        let renamed = (2..=limit)
            .map(|k| format!("{name} ({k})"))
            .find(|c| !originals.contains(c.as_str()) && !used.contains(c))
            .unwrap_or_default();
        used.insert(renamed.clone());
        losses.push(Loss::RenamedTrack {
            track: i,
            from: name.to_owned(),
            to: renamed.clone(),
        });
        out.push(renamed);
    }
    out
}

/// 텍스트가 나르지 않는 트랙 속성 가운데 `txt2ksq`가 만들 값과 다른 것.
fn track_attribute_losses(
    index: usize,
    track: &Track,
    name: &str,
    losses: &mut Vec<Loss>,
) -> Result<(), CliError> {
    let color = default_color(index)?;
    let fresh = Track::new("", track.channel(), color);
    let mut attributes = Vec::new();
    if track.color() != color {
        attributes.push("color");
    }
    if track.port() != fresh.port() {
        attributes.push("port");
    }
    if track.default_velocity() != fresh.default_velocity() {
        attributes.push("default_velocity");
    }
    if track.mute() != fresh.mute() {
        attributes.push("mute");
    }
    if track.solo() != fresh.solo() {
        attributes.push("solo");
    }
    if track.lyric_lang() != fresh.lyric_lang() {
        attributes.push("lyric_lang");
    }
    if !attributes.is_empty() {
        losses.push(Loss::TrackAttributes {
            track: index,
            name: name.to_owned(),
            attributes,
        });
    }
    Ok(())
}

/// 트랙의 클립들을 시작 0 클립 하나로 합친다. 노트는 트랙 기준 자리로 옮기고, 클립 끝을 넘는
/// 꼬리는 자른다(내보내기도 클립 끝에서 자르지만, 합친 클립에서는 꼬리가 다음 클립 자리로
/// 들어가 보이게 되므로 여기서 먼저 자른다).
fn merged_clip(
    index: usize,
    track: &Track,
    name: &str,
    bar: u64,
    losses: &mut Vec<Loss>,
) -> Result<Clip, CliError> {
    let clips: Vec<&Clip> = track.clips().map(|(_, c)| c).collect();
    let (Some(first), Some(last)) = (clips.first(), clips.last()) else {
        losses.push(Loss::EmptyTrack {
            track: index,
            name: name.to_owned(),
        });
        return Ok(Clip::new(Tick::ZERO, bar)?);
    };
    if clips.len() > 1 || first.start() != Tick::ZERO {
        losses.push(Loss::ClipLayout {
            track: index,
            name: name.to_owned(),
            clips: clips.len(),
            first_start: first.start().get(),
        });
    }
    if clips
        .iter()
        .any(|c| c.name().is_some() || c.color().is_some())
    {
        losses.push(Loss::ClipAttributes {
            track: index,
            name: name.to_owned(),
        });
    }
    let text_velocity = Velocity::from_midi1(TEXT_VELOCITY)?;
    let (mut tails, mut velocities, mut offsets) = (0, 0, 0);
    let mut merged = Clip::new(Tick::ZERO, last.end())?;
    for clip in &clips {
        for n in clip.notes() {
            let mut length = n.length();
            if n.end() > clip.length() {
                tails += 1;
                // 노트 시작은 클립 안이라 남는 길이는 1 이상이고 원래 길이(u32)보다 짧다.
                length = u32::try_from(clip.length() - n.start().get()).unwrap_or(length);
            }
            if n.velocity() != text_velocity {
                velocities += 1;
            }
            if n.offset() != PitchOffset::ZERO {
                offsets += 1;
            }
            let start = clip.start().checked_add(n.start().get())?;
            let mut note = Note::new(start, length, n.pitch(), text_velocity)?;
            if let Some(lyric) = n.lyric() {
                note = note.with_lyric(lyric);
            }
            merged.insert_note(note)?;
        }
    }
    let name = name.to_owned();
    if tails > 0 {
        losses.push(Loss::HiddenTails {
            track: index,
            name: name.clone(),
            notes: tails,
        });
    }
    if velocities > 0 {
        losses.push(Loss::Velocities {
            track: index,
            name: name.clone(),
            notes: velocities,
        });
    }
    if offsets > 0 {
        losses.push(Loss::PitchOffsets {
            track: index,
            name,
            notes: offsets,
        });
    }
    Ok(merged)
}

/// 쓴 텍스트를 다시 읽어 넘긴 헤더·트랙과 대조한다. 클립 길이 차이는 내보내기 규칙(반올림)이라
/// [`Loss::ClipLength`]이고, 그 밖의 차이는 [`Loss::Mismatch`]다.
fn verify(
    text: &str,
    header: &Header,
    tracks: &[ExportTrack<'_>],
    losses: &mut Vec<Loss>,
) -> Result<(), CliError> {
    let parsed = parse(text, Mode::Lenient).map_err(CliError::ExportUnreadable)?;
    if parsed.header() != header {
        losses.push(Loss::Mismatch {
            detail: format!("헤더: {header:?} → {:?}", parsed.header()),
        });
    }
    if parsed.tracks().len() != tracks.len() {
        losses.push(Loss::Mismatch {
            detail: format!("트랙 수: {} → {}", tracks.len(), parsed.tracks().len()),
        });
        return Ok(());
    }
    for (i, (back, sent)) in parsed.tracks().iter().zip(tracks).enumerate() {
        let label = TrackLabel(i, sent.name);
        if back.name() != sent.name
            || back.channel() != sent.channel
            || back.program() != sent.program
            || back.bank() != sent.bank
        {
            losses.push(Loss::Mismatch {
                detail: format!("{label}: 트랙 속성"),
            });
        }
        if note_views(back.clip()) != note_views(sent.clip) {
            losses.push(Loss::Mismatch {
                detail: format!("{label}: 노트"),
            });
        }
        if back.clip().length() != sent.clip.length() {
            losses.push(Loss::ClipLength {
                track: i,
                name: sent.name.to_owned(),
                from: sent.clip.length(),
                to: back.clip().length(),
            });
        }
    }
    Ok(())
}

/// 텍스트가 나르는 노트 정보(시작, 길이, 음높이, 가사), 정렬한 것.
fn note_views(clip: &Clip) -> Vec<(u64, u32, u8, Option<&str>)> {
    let mut v: Vec<_> = clip
        .notes()
        .iter()
        .map(|n| (n.start().get(), n.length(), n.pitch().get(), n.lyric()))
        .collect();
    v.sort_unstable();
    v
}

/// `.ksq`에서 이 도구가 아는 키. `keyscore-core`는 모르는 키를 보존하지만 내보내지 않으므로,
/// 직렬화 결과를 훑어 이 목록 밖의 키를 찾는다. core가 새 키를 알게 되면 여기도 더해야 한다
/// (빠뜨리면 손실이 아닌 것을 손실로 보고할 뿐, 놓치지는 않는다).
const ROOT_KEYS: &[&str] = &["format_version", "key", "tempo", "meter", "tracks"];
const KEY_KEYS: &[&str] = &["tonic", "mode"];
const TRACK_KEYS: &[&str] = &[
    "name",
    "color",
    "port",
    "channel",
    "bank",
    "program",
    "default_velocity",
    "mute",
    "solo",
    "lyric_lang",
    "clips",
];
const PORT_KEYS: &[&str] = &["named"];
const CLIP_KEYS: &[&str] = &["start", "length", "name", "color", "notes"];
const NOTE_KEYS: &[&str] = &["offset", "lyric"];

fn unknown_keys(canonical: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(Value::Object(root)) = serde_json::from_str::<Value>(canonical) else {
        return out;
    };
    extra_keys(&root, ROOT_KEYS, "", &mut out);
    if let Some(Value::Object(key)) = root.get("key") {
        extra_keys(key, KEY_KEYS, "key", &mut out);
    }
    for (i, track) in objects(root.get("tracks")) {
        let path = format!("tracks[{i}]");
        extra_keys(track, TRACK_KEYS, &path, &mut out);
        if let Some(Value::Object(port)) = track.get("port") {
            extra_keys(port, PORT_KEYS, &format!("{path}.port"), &mut out);
        }
        for (j, clip) in objects(track.get("clips")) {
            let path = format!("{path}.clips[{j}]");
            extra_keys(clip, CLIP_KEYS, &path, &mut out);
            let notes = clip.get("notes").and_then(Value::as_array);
            for (k, note) in notes.into_iter().flatten().enumerate() {
                if let Some(Value::Object(attrs)) = note.get(4) {
                    extra_keys(attrs, NOTE_KEYS, &format!("{path}.notes[{k}]"), &mut out);
                }
            }
        }
    }
    out
}

fn objects(value: Option<&Value>) -> impl Iterator<Item = (usize, &Map<String, Value>)> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(i, v)| v.as_object().map(|o| (i, o)))
}

fn extra_keys(object: &Map<String, Value>, known: &[&str], path: &str, out: &mut Vec<String>) {
    for key in object.keys().filter(|k| !known.contains(&k.as_str())) {
        out.push(if path.is_empty() {
            key.clone()
        } else {
            format!("{path}.{key}")
        });
    }
}

/// 계이름 텍스트를 프로젝트로 읽는다. 헤더는 템포·박자 맵의 틱 0 값과 조가 되고, 트랙 블록마다
/// 트랙 하나와 시작 0 클립 하나가 된다. 텍스트에 없는 헤더는 [`Project::new`]의 기본값이다.
/// 트랙 색은 순번대로 팔레트를 돈다(텍스트가 색을 나르지 않는다).
///
/// # Errors
///
/// 문법 오류면 [`CliError::Parse`].
pub fn text_to_project(text: &str, mode: Mode) -> Result<(Project, Vec<Diagnostic>), CliError> {
    let parsed = parse(text, mode)?;
    let mut project = Project::new();
    let header = parsed.header();
    if let Some(tempo) = header.tempo() {
        *project.tempo_map_mut() = TempoMap::new(tempo);
    }
    if let Some(time) = header.time() {
        *project.meter_map_mut() = MeterMap::new(time);
    }
    if let Some(key) = header.key() {
        project.set_key(key);
    }
    for (i, t) in parsed.tracks().iter().enumerate() {
        let mut track = Track::new(t.name(), t.channel(), default_color(i)?);
        track.set_program(t.program());
        track.set_bank(t.bank());
        track.insert_clip(t.clip().clone())?;
        project.push_track(track)?;
    }
    Ok((project, parsed.warnings().to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyscore_core::{
        Bank, Channel, Clip, MeterMap, Note, Pitch, PitchOffset, Program, Tempo, TempoMap, Tick,
        TimeSignature, Track, TrackColor, Velocity, ksq,
    };

    fn note(start: u64, length: u32, pitch: u8) -> Note {
        Note::new(
            Tick::new(start),
            length,
            Pitch::new(pitch).unwrap(),
            Velocity::from_midi1(100).unwrap(),
        )
        .unwrap()
    }

    fn clip(start: u64, length: u64, notes: Vec<Note>) -> Clip {
        let mut c = Clip::new(Tick::new(start), length).unwrap();
        for n in notes {
            c.insert_note(n).unwrap();
        }
        c
    }

    /// `txt2ksq`가 만드는 것과 같은 속성의 트랙(색 = 순번 % 12).
    fn track(index: usize, name: &str, clips: Vec<Clip>) -> Track {
        let color = TrackColor::new(u8::try_from(index % 12).unwrap()).unwrap();
        let mut t = Track::new(name, Channel::new(1).unwrap(), color);
        for c in clips {
            t.insert_clip(c).unwrap();
        }
        t
    }

    fn project(tracks: Vec<Track>) -> Project {
        let mut p = Project::new();
        for t in tracks {
            p.push_track(t).unwrap();
        }
        p
    }

    #[test]
    fn ksq2txt_reports_losses_for_multi_clip_tempo_change_velocity() {
        let loud = Note::new(
            Tick::new(0),
            480,
            Pitch::new(64).unwrap(),
            Velocity::from_midi1(80).unwrap(),
        )
        .unwrap();
        let mut p = project(vec![track(
            0,
            "Lead",
            vec![
                clip(0, 960, vec![note(0, 480, 60)]),
                clip(1920, 960, vec![loud]),
            ],
        )]);
        p.tempo_map_mut()
            .set(Tick::new(3840), Tempo::from_bpm(90).unwrap());
        p.meter_map_mut()
            .set(Tick::new(3840), TimeSignature::new(3, 4).unwrap());

        let (text, losses) = project_to_text(&p).unwrap();

        assert!(
            losses.contains(&Loss::TempoChanges { dropped: 1 }),
            "{losses:?}"
        );
        assert!(
            losses.contains(&Loss::MeterChanges { dropped: 1 }),
            "{losses:?}"
        );
        assert!(
            losses.contains(&Loss::ClipLayout {
                track: 0,
                name: "Lead".to_owned(),
                clips: 2,
                first_start: 0,
            }),
            "{losses:?}"
        );
        assert!(
            losses.contains(&Loss::Velocities {
                track: 0,
                name: "Lead".to_owned(),
                notes: 1,
            }),
            "{losses:?}"
        );
        assert_eq!(losses.len(), 4, "{losses:?}");
        // 합친 클립은 0에서 두 번째 클립 끝까지이고, 노트는 트랙 기준 자리에 있다.
        let (back, _) = text_to_project(&text, Mode::Lenient).unwrap();
        let clips: Vec<_> = back.tracks()[0].clips().map(|(_, c)| c.clone()).collect();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].length(), 2880);
        let starts: Vec<_> = clips[0].notes().iter().map(|n| n.start().get()).collect();
        assert_eq!(starts, vec![0, 1920]);
    }

    #[test]
    fn ksq2txt_renames_duplicate_track_names_and_round_trips() {
        let p = project(vec![
            track(0, "A", vec![clip(0, 960, vec![note(0, 960, 60)])]),
            track(1, "A", vec![clip(0, 960, vec![note(0, 960, 62)])]),
            track(2, "A (2)", vec![clip(0, 960, vec![note(0, 960, 64)])]),
        ]);

        let (text, losses) = project_to_text(&p).unwrap();

        // 「A (2)」는 이미 있으므로 두 번째 「A」는 「A (3)」이 된다.
        assert_eq!(
            losses,
            vec![Loss::RenamedTrack {
                track: 1,
                from: "A".to_owned(),
                to: "A (3)".to_owned(),
            }]
        );
        let (back, warnings) = text_to_project(&text, Mode::Strict).unwrap();
        assert!(warnings.is_empty());
        let names: Vec<_> = back.tracks().iter().map(Track::name).collect();
        assert_eq!(names, vec!["A", "A (3)", "A (2)"]);
        let pitches: Vec<_> = back
            .tracks()
            .iter()
            .map(|t| t.clips().next().unwrap().1.notes()[0].pitch().get())
            .collect();
        assert_eq!(pitches, vec![60, 62, 64]);
    }

    #[test]
    fn ksq2txt_without_losses_round_trips_ksq_bytes() {
        let mut p = project(vec![
            track(
                0,
                "Lead",
                vec![clip(0, 1920, vec![note(0, 480, 60), note(480, 1440, 67)])],
            ),
            track(1, "Pad \"x\"", vec![clip(0, 960, vec![note(0, 960, 48)])]),
        ]);
        *p.tempo_map_mut() = TempoMap::new(Tempo::from_bpm(132).unwrap());
        *p.meter_map_mut() = MeterMap::new(TimeSignature::new(6, 8).unwrap());

        let (text, losses) = project_to_text(&p).unwrap();

        assert_eq!(losses, vec![]);
        let (back, _) = text_to_project(&text, Mode::Strict).unwrap();
        assert_eq!(ksq::to_string(&back), ksq::to_string(&p));
    }

    #[test]
    fn ksq2txt_empty_track_writes_one_bar_rest_and_reports_loss() {
        let p = project(vec![track(0, "Empty", vec![])]);

        let (text, losses) = project_to_text(&p).unwrap();

        assert_eq!(
            losses,
            vec![Loss::EmptyTrack {
                track: 0,
                name: "Empty".to_owned(),
            }]
        );
        let (back, _) = text_to_project(&text, Mode::Strict).unwrap();
        let clip = back.tracks()[0].clips().next().unwrap().1.clone();
        assert_eq!(clip.length(), 3840);
        assert!(clip.notes().is_empty());
    }

    #[test]
    fn ksq2txt_hidden_tail_offset_and_track_attributes_are_reported() {
        let mut c = Clip::new(Tick::ZERO, 1920).unwrap();
        c.insert_note(
            note(0, 960, 60)
                .with_offset(PitchOffset::new(PitchOffset::SEMITONE / 2).unwrap())
                .unwrap(),
        )
        .unwrap();
        c.set_name("verse");
        let mut t = track(0, "Lead", vec![c]);
        t.set_mute(true);
        t.set_lyric_lang(Some("ko".to_owned()));
        // 숨김 꼬리는 분할로만 생긴다.
        let id = t
            .insert_clip(clip(1920, 1920, vec![note(0, 1920, 62)]))
            .unwrap();
        t.split_clip(id, Tick::new(2880)).unwrap();
        let p = project(vec![t]);

        let (_, losses) = project_to_text(&p).unwrap();

        let name = "Lead".to_owned();
        for expected in [
            Loss::PitchOffsets {
                track: 0,
                name: name.clone(),
                notes: 1,
            },
            Loss::ClipAttributes {
                track: 0,
                name: name.clone(),
            },
            Loss::HiddenTails {
                track: 0,
                name: name.clone(),
                notes: 1,
            },
            Loss::TrackAttributes {
                track: 0,
                name: name.clone(),
                attributes: vec!["mute", "lyric_lang"],
            },
        ] {
            assert!(losses.contains(&expected), "{expected:?} 없음: {losses:?}");
        }
    }

    #[test]
    fn ksq2txt_unknown_keys_are_reported_with_paths() {
        let base = project(vec![track(
            0,
            "A",
            vec![clip(0, 960, vec![note(0, 960, 60)])],
        )]);
        let text = ksq::to_string(&base)
            .replacen(
                "\"format_version\": 1,",
                "\"format_version\": 1, \"x_top\": 1,",
                1,
            )
            .replacen("\"mute\": false,", "\"mute\": false, \"x_track\": true,", 1)
            .replacen(
                "[0, 960, 60, 51492]",
                "[0, 960, 60, 51492, {\"x_note\": 2}]",
                1,
            );
        let (p, _) = ksq::from_str(&text).unwrap();

        let (_, losses) = project_to_text(&p).unwrap();

        assert_eq!(
            losses,
            vec![Loss::UnknownKeys {
                paths: vec![
                    "x_top".to_owned(),
                    "tracks[0].x_track".to_owned(),
                    "tracks[0].clips[0].notes[0].x_note".to_owned(),
                ],
            }]
        );
    }

    #[test]
    fn ksq2txt_unreachable_clip_length_reports_rounded_length() {
        // 노트 끝 960 뒤 7틱은 쉼표(15틱 단위)로 닿지 않는다.
        let p = project(vec![track(
            0,
            "A",
            vec![clip(0, 967, vec![note(0, 960, 60)])],
        )]);

        let (_, losses) = project_to_text(&p).unwrap();

        assert_eq!(
            losses,
            vec![Loss::ClipLength {
                track: 0,
                name: "A".to_owned(),
                from: 967,
                to: 960,
            }]
        );
    }

    #[test]
    fn ksq2txt_far_clip_start_errors_before_building_text() {
        let p = project(vec![track(
            0,
            "Far",
            vec![clip(3840 * 10_000_000_000, 960, vec![note(0, 960, 60)])],
        )]);

        let err = project_to_text(&p).unwrap_err();

        let CliError::OutputTooLarge {
            estimated,
            limit,
            detail,
        } = &err
        else {
            panic!("{err}");
        };
        assert!(estimated > limit);
        // 기준은 텍스트가 나르는 내용으로 다시 만든 프로젝트(시작 0으로 합친 클립)다.
        let far = 3840 * 10_000_000_000;
        let projected = project(vec![track(
            0,
            "Far",
            vec![clip(0, far + 960, vec![note(far, 960, 60)])],
        )]);
        assert_eq!(
            *limit,
            MAX_EXPANSION * ksq::to_string(&projected).len() as u64
        );
        assert!(detail.contains("트랙 1 \"Far\""), "{detail}");
    }

    #[test]
    fn ksq2txt_estimate_beyond_u64_is_unconvertible_at_any_limit() {
        // u64 끝 근처의 클립에 노트 1,000개: 성부 1,001개 × 온음표 약 4.8 × 10^15개 × 6바이트가
        // u64를 넘는다. 어림이 u64 안이면 상한 u64::MAX에서 텍스트를 실제로 만들려 하므로(메모리를
        // 다 쓴다) 노트 수를 넉넉히 둔다.
        let length = 2_000_000;
        let notes = (0..1000u64)
            .map(|i| note(i * 1920, 960, 30 + u8::try_from(i % 60).unwrap()))
            .collect();
        let p = project(vec![track(
            0,
            "A",
            vec![clip(u64::MAX - length, length, notes)],
        )]);

        let err = project_to_text_limited(&p, u64::MAX).unwrap_err();

        assert!(
            matches!(err, CliError::OutputBeyondAnyLimit { .. }),
            "{err}"
        );
    }

    #[test]
    fn ksq2txt_limited_rejects_when_estimate_exceeds_limit() {
        let p = project(vec![track(
            0,
            "A",
            vec![clip(0, 3840 * 100, vec![note(0, 960, 60)])],
        )]);

        let small = project_to_text_limited(&p, 100);
        let ample = project_to_text_limited(&p, 1 << 20);

        assert!(
            matches!(small, Err(CliError::OutputTooLarge { .. })),
            "{small:?}"
        );
        assert!(ample.is_ok());
    }

    #[test]
    fn txt2ksq_missing_headers_use_project_defaults() {
        let (p, warnings) =
            text_to_project("@track A ch3 prog5 bank7\nc4\n", Mode::Strict).unwrap();

        assert!(warnings.is_empty());
        let fresh = Project::new();
        assert_eq!(p.tempo_map(), fresh.tempo_map());
        assert_eq!(p.meter_map(), fresh.meter_map());
        assert_eq!(p.key(), fresh.key());
        let t = &p.tracks()[0];
        assert_eq!(
            (
                t.channel().get(),
                t.program().map(Program::get),
                t.bank().map(Bank::get)
            ),
            (3, Some(5), Some(7))
        );
    }

    #[test]
    fn txt2ksq_lenient_returns_warnings() {
        let (_, warnings) = text_to_project("@track A ch1\nc4 q c4\n", Mode::Lenient).unwrap();

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn loss_display_names_track_by_one_based_number() {
        let loss = Loss::Velocities {
            track: 0,
            name: "Lead".to_owned(),
            notes: 3,
        };

        assert!(loss.to_string().starts_with("트랙 1 \"Lead\": "), "{loss}");
    }
}
