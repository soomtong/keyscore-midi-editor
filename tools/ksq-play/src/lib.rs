//! `.ksq`·`.ksqz`를 `CoreMIDI`로 재생하는 도구(S1-T7, 스케줄러의 첫 실사용).
//!
//! 종료 코드: 0 곡 끝까지 재생했거나 `q`로 멈춤, 1 실패(파일·MIDI 오류), 2 잘못된 사용법,
//! 130 Ctrl-C.

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use keyscore_core::ksq::{self, KsqError};
use keyscore_core::{OutputPort, Tick};
use keyscore_midi::{
    MetronomeConfig, MidiClient, MidiError, MidiPort, OsOutput, Scheduler, SchedulerConfig,
    Snapshot, Suggestion, destinations, render, suggest_mapping,
};

/// Ctrl-C를 받았을 때 바이너리가 명령 채널로 보내는 값.
pub const INTERRUPT: &str = "\u{3}";

/// 가상 포트 이름의 기본값(OUT-1).
const DEFAULT_PORT: &str = keyscore_midi::DEFAULT_PORT_NAME;

/// 정지·패닉 뒤 프로세스를 끝내기 전에 기다리는 시간. 스케줄러는 최대 lookahead(10 ms) 앞의 시각으로
/// 예약하므로, 가상 포트를 없애기 전에 그 메시지가 수신 쪽에 닿게 한다.
const DRAIN: Duration = Duration::from_millis(50);

/// 곡 끝 뒤에 더 기다리는 시간. 마지막 Note Off가 닿은 뒤 정지한다.
const TAIL: Duration = Duration::from_millis(100);

/// `--wait`의 상한. 오타로 몇 시간을 기다리지 않게 한다.
const MAX_WAIT_SECS: f64 = 3600.0;

/// 사용법 문자열.
pub const USAGE: &str = "사용법: ksq-play <파일.ksq|.ksqz> [--port <가상 포트 이름>] [--from <마디>] \
[--metronome] [--metronome-port <OS 목적지 이름>] [--wait <초>]\n재생 중: p + Enter 패닉, q + Enter 정지";

/// 해석한 인자.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    path: PathBuf,
    port_name: String,
    from_bar: u64,
    metronome: bool,
    metronome_port: Option<String>,
    wait: Duration,
}

impl Options {
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// 가상 포트 이름(기본 「KeyScore Out」).
    #[must_use]
    pub fn port_name(&self) -> &str {
        &self.port_name
    }

    /// 시작 마디(1부터).
    #[must_use]
    pub const fn from_bar(&self) -> u64 {
        self.from_bar
    }

    #[must_use]
    pub const fn metronome(&self) -> bool {
        self.metronome
    }

    #[must_use]
    pub fn metronome_port(&self) -> Option<&str> {
        self.metronome_port.as_deref()
    }

    #[must_use]
    pub const fn wait(&self) -> Duration {
        self.wait
    }
}

/// 잘못된 사용법.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct UsageError(pub String);

/// 인자를 해석한다.
///
/// # Errors
///
/// 파일이 없거나 둘 이상, 모르는 옵션, 값이 없거나 잘못된 옵션이면 [`UsageError`].
pub fn parse_args(args: &[OsString]) -> Result<Options, UsageError> {
    let mut path = None;
    let mut options = Options {
        path: PathBuf::new(),
        port_name: DEFAULT_PORT.to_string(),
        from_bar: 1,
        metronome: false,
        metronome_port: None,
        wait: Duration::ZERO,
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = |flag: &str| {
            it.next()
                .map(|v| v.to_string_lossy().into_owned())
                .ok_or_else(|| UsageError(format!("{flag}에 값이 없다")))
        };
        match arg.to_str() {
            Some("--port") => options.port_name = value("--port")?,
            Some("--from") => {
                let v = value("--from")?;
                options.from_bar = v
                    .parse::<u64>()
                    .ok()
                    .filter(|&n| n >= 1)
                    .ok_or_else(|| UsageError(format!("--from은 1 이상의 마디 번호다: {v}")))?;
            }
            Some("--metronome") => options.metronome = true,
            Some("--metronome-port") => {
                options.metronome_port = Some(value("--metronome-port")?);
            }
            Some("--wait") => {
                let v = value("--wait")?;
                let secs = v
                    .parse::<f64>()
                    .ok()
                    .filter(|s| s.is_finite() && (0.0..=MAX_WAIT_SECS).contains(s))
                    .ok_or_else(|| UsageError(format!("--wait는 0–3600초다: {v}")))?;
                options.wait = Duration::from_secs_f64(secs);
            }
            Some(flag) if flag.starts_with("--") => {
                return Err(UsageError(format!("모르는 옵션: {flag}")));
            }
            _ if path.is_some() => return Err(UsageError("파일은 하나만 준다".to_string())),
            _ => path = Some(PathBuf::from(arg)),
        }
    }
    options.path = path.ok_or_else(|| UsageError("파일이 없다".to_string()))?;
    Ok(options)
}

/// 재생 오류.
#[derive(Debug, thiserror::Error)]
enum PlayError {
    #[error("{0}")]
    Ksq(#[from] KsqError),
    #[error("{0}")]
    Midi(#[from] MidiError),
    #[error("{0}")]
    Other(String),
}

/// 재생 중 명령의 결과.
enum Ending {
    Finished,
    Quit,
    Interrupted,
}

macro_rules! say {
    ($w:expr, $($arg:tt)*) => {
        let _ = writeln!($w, $($arg)*);
    };
}

/// 재생한다. `commands`의 `p`는 패닉, `q`는 정지 후 끝, [`INTERRUPT`]는 패닉·정지 후 130.
pub fn run(
    args: &[OsString],
    commands: &Receiver<String>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        say!(out, "{USAGE}");
        return 0;
    }
    let options = match parse_args(args) {
        Ok(o) => o,
        Err(e) => {
            say!(err, "{e}\n{USAGE}");
            return 2;
        }
    };
    match play(&options, commands, out, err) {
        Ok(Ending::Finished | Ending::Quit) => 0,
        Ok(Ending::Interrupted) => 130,
        Err(e) => {
            say!(err, "{e}");
            1
        }
    }
}

/// 포트 이름 → 스냅샷 포트 번호.
///
/// OS 목적지가 있으면 연다. 없으면 `suggest_mapping`의 제안대로 그 이름의 가상 포트를 따로 만든다
/// (OUT-8). 기본 가상 포트에 합치면 같은 채널의 다른 트랙과 섞여 프로그램이 덮이고(SEQ-7), 메트로놈
/// 클릭이 멜로디 신디로 간다(MET-3, S1-T7 리뷰 1 지적 1·2).
struct Ports {
    list: Vec<Arc<dyn MidiPort>>,
    by_name: HashMap<String, u16>,
    main_name: String,
}

impl Ports {
    fn index_of(
        &mut self,
        client: &MidiClient,
        name: &str,
        what: &str,
        err: &mut dyn Write,
    ) -> Result<u16, PlayError> {
        if let Some(&i) = self.by_name.get(name) {
            return Ok(i);
        }
        let available = destinations();
        let suggestion = suggest_mapping(&[name.to_string()], &available)
            .pop()
            .map(|(_, s)| s);
        let port: Arc<dyn MidiPort> = match suggestion {
            Some(Suggestion::Present(info)) => Arc::new(OsOutput::open(client, &info)?),
            Some(Suggestion::Virtual { name: virtual_name }) => {
                if virtual_name == self.main_name {
                    // 기본 가상 포트와 같은 이름이면 그 포트가 바로 그 목적지다.
                    self.by_name.insert(name.to_string(), 0);
                    return Ok(0);
                }
                say!(
                    err,
                    "{what}의 출력 「{name}」이 없어 같은 이름의 가상 포트를 만든다"
                );
                Arc::new(client.create_virtual_output(&virtual_name)?)
            }
            // 가상 포트 이름으로 쓸 수 없는 이름(제어 문자 등)이다. 번호를 붙인 이름으로 만든다.
            Some(Suggestion::Unavailable) | None => {
                let fallback = format!("{} {}", self.main_name, self.list.len() + 1);
                say!(
                    err,
                    "{what}의 출력 「{}」이 없고 그 이름으로 가상 포트를 만들 수 없어 「{fallback}」을 만든다",
                    name.escape_debug()
                );
                Arc::new(client.create_virtual_output(&fallback)?)
            }
        };
        self.list.push(port);
        let index = u16::try_from(self.list.len() - 1)
            .map_err(|_| PlayError::Other("포트가 너무 많다".to_string()))?;
        self.by_name.insert(name.to_string(), index);
        Ok(index)
    }
}

fn play(
    options: &Options,
    commands: &Receiver<String>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Ending, PlayError> {
    let (project, warnings) = ksq::read_file(options.path())?;
    for w in warnings {
        say!(err, "{w}");
    }
    let from = project
        .meter_map()
        .bar_start(options.from_bar() - 1)
        .ok_or_else(|| PlayError::Other(format!("마디 {}가 곡 범위 밖이다", options.from_bar())))?;
    // 프로세스의 첫 CoreMIDI 클라이언트다(알림 조건, S1-T2).
    let client = MidiClient::new("KeyScore Play")?;
    let virtual_out = client.create_virtual_output(options.port_name())?;
    let mut ports = Ports {
        list: vec![Arc::new(virtual_out)],
        by_name: HashMap::new(),
        main_name: options.port_name().to_string(),
    };
    let mut track_ports = Vec::new();
    for track in project.tracks() {
        let index = match track.port() {
            OutputPort::Virtual => 0,
            OutputPort::Named(name) => {
                ports.index_of(&client, name, &format!("트랙 「{}」", track.name()), err)?
            }
        };
        track_ports.push(index);
    }
    let metronome_port = match options.metronome_port() {
        Some(name) if options.metronome() => ports.index_of(&client, name, "메트로놈", err)?,
        Some(_) => {
            say!(
                err,
                "--metronome 없이 --metronome-port를 줬다. 클릭은 나가지 않는다"
            );
            0
        }
        None => 0,
    };
    let rendered = render(&project, |i, _| track_ports.get(i).copied());
    let end = rendered.end();
    if from > end && end > Tick::ZERO {
        return Err(PlayError::Other(format!(
            "마디 {}가 곡 끝 뒤다",
            options.from_bar()
        )));
    }
    let snapshot = Snapshot::new(
        project.tempo_map().clone(),
        ports.list,
        rendered.into_events(),
    )?
    .with_meter(project.meter_map().clone());
    let mut scheduler = Scheduler::start(Arc::new(snapshot), SchedulerConfig::default())?;
    if options.metronome() {
        let config = MetronomeConfig::default();
        let config = MetronomeConfig::new(
            metronome_port,
            config.channel(),
            config.accent(),
            config.beat(),
        );
        scheduler.set_metronome(config)?;
        scheduler.set_metronome_enabled(true)?;
    }
    say!(
        out,
        "재생: {} → 「{}」 (p + Enter 패닉, q + Enter 정지)",
        options.path().display(),
        options.port_name()
    );
    // 신디가 포트를 잡을 시간. 그동안에도 q·Ctrl-C는 받는다.
    if let Some(ending) = wait_for_commands(commands, options.wait(), &mut scheduler, err)? {
        return Ok(finish(scheduler, ending));
    }
    scheduler.play(from)?;
    let tempo = project.tempo_map();
    let span = tempo
        .tick_to_time(end.max(from))
        .as_nanos()
        .saturating_sub(tempo.tick_to_time(from).as_nanos());
    let length = Duration::from_nanos(u64::try_from(span).unwrap_or(u64::MAX)).saturating_add(TAIL);
    let ending =
        wait_for_commands(commands, length, &mut scheduler, err)?.unwrap_or(Ending::Finished);
    Ok(finish(scheduler, ending))
}

/// `limit` 동안 명령을 처리한다. 끝내는 명령이 오면 그 결과를, 시간이 다 되면 `None`을 돌려준다.
fn wait_for_commands(
    commands: &Receiver<String>,
    limit: Duration,
    scheduler: &mut Scheduler,
    err: &mut dyn Write,
) -> Result<Option<Ending>, PlayError> {
    let deadline = Instant::now() + limit;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(None);
        }
        match commands.recv_timeout(left) {
            Ok(line) => match line.trim() {
                "p" => scheduler.panic()?,
                "q" => return Ok(Some(Ending::Quit)),
                INTERRUPT => {
                    scheduler.panic()?;
                    return Ok(Some(Ending::Interrupted));
                }
                "" => {}
                other => {
                    say!(err, "모르는 명령: {other} (p 패닉, q 정지)");
                }
            },
            Err(RecvTimeoutError::Timeout) => return Ok(None),
            // 명령을 보낼 쪽이 없다(표준 입력이 닫힘). 시간만 기다린다.
            Err(RecvTimeoutError::Disconnected) => {
                std::thread::sleep(left);
                return Ok(None);
            }
        }
    }
}

/// 정지하고, 예약한 끄기 메시지가 닿을 때까지 기다린 뒤 스케줄러를 닫는다. 닫을 때 울리는 노트를
/// 끈다(S1-T4).
fn finish(mut scheduler: Scheduler, ending: Ending) -> Ending {
    let _ = scheduler.stop();
    std::thread::sleep(DRAIN);
    drop(scheduler);
    std::thread::sleep(DRAIN);
    ending
}
