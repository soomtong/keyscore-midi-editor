//! S2-T7d 적대적 리뷰 1회차 수용 시험(`docs/reports/s2-t7d/review-1.md`). 가상 포트 개수를 세거나 CoreMIDI
//! 목적지를 만드는 시험은 프로세스 단위 16개 상한·목적지 목록을 다른 시험과 나누지 않게 자식 프로세스에서 돈다.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Action, AnyWindowHandle, AppContext as _, Entity, TestAppContext};
use keyscore_app::actions::transport;
use keyscore_app::engine::{self, EngineOptions};
use keyscore_app::port_mapping::{self, Line, MainReason, Mapping};
use keyscore_app::window::ProjectWindow;
use keyscore_app::{files, init};
use keyscore_core::{OutputPort, Project, ksq};

const MAP: &str = "제안대로 매핑";
const WAIT: Duration = Duration::from_secs(2);

fn unique(tag: &str) -> String {
    format!("KS rv7d {} {tag}", std::process::id())
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ks-rv7d-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

/// 트랙마다 포트 하나. 각 트랙에 박마다 노트가 든 긴 클립(재생 시험용).
fn project_on(ports: &[OutputPort]) -> Project {
    let notes: Vec<String> = (0..64)
        .map(|i| format!("[{}, 240, 60, 100]", if i == 0 { 0 } else { 960 }))
        .collect();
    let tracks: Vec<String> = ports
        .iter()
        .enumerate()
        .map(|(i, port)| {
            let port = match port {
                OutputPort::Virtual => "\"virtual\"".to_owned(),
                OutputPort::Named(n) => {
                    format!("{{\"named\": {}}}", serde_json::to_string(n).unwrap())
                }
            };
            format!(
                r#"{{"name": "T{i}", "port": {port}, "channel": 1,
                    "clips": [{{"start": 0, "length": 61440, "notes": [{}]}}]}}"#,
                notes.join(", ")
            )
        })
        .collect();
    ksq::from_str(&format!(
        r#"{{"tempo": [[0, 500000]], "tracks": [{}]}}"#,
        tracks.join(",")
    ))
    .unwrap()
    .0
}

fn named(n: &str) -> OutputPort {
    OutputPort::Named(n.to_owned())
}

fn start(cx: &mut TestAppContext, tag: &str) -> PathBuf {
    let dir = temp_dir(tag);
    cx.update(|cx| init(cx, dir.join("settings.json")));
    let options = EngineOptions {
        client_name: format!("ks-rv7d-{tag}"),
        port_name: unique(&format!("main {tag}")),
    };
    cx.update(|cx| engine::install(cx, &options)).unwrap();
    dir
}

fn open_file(cx: &mut TestAppContext, path: &Path) -> (AnyWindowHandle, Entity<ProjectWindow>) {
    let (handle, view) = cx.update(|cx| files::open_path(path, cx)).unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.activate_window();
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    (handle, view)
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, action: impl Action) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_action(Box::new(action), cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn prompt_text(cx: &mut TestAppContext) -> String {
    let (msg, detail) = cx.pending_prompt().expect("시트가 없다");
    format!("{msg}\n{detail}")
}

fn answer(cx: &mut TestAppContext, button: &str) {
    assert!(cx.has_pending_prompt(), "시트가 없다");
    cx.simulate_prompt_answer(button);
    cx.run_until_parked();
}

fn ports_of(cx: &mut TestAppContext, view: &Entity<ProjectWindow>) -> Vec<OutputPort> {
    cx.read(|cx| {
        view.read(cx)
            .document()
            .read(cx)
            .project()
            .tracks()
            .iter()
            .map(|t| t.port().clone())
            .collect()
    })
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

fn run_isolated(test: &str) -> bool {
    const ENV: &str = "KS_APP_ISOLATED_TEST";
    if std::env::var(ENV).as_deref() == Ok(test) {
        return false;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture"])
        .env(ENV, test)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("test result: ok. 1 passed"),
        "{test}: 자식 프로세스에서 실패했다\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    true
}

fn split(data: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let len = match data[i] & 0xF0 {
            0xC0 | 0xD0 => 2,
            0xF0 => 1,
            _ => 3,
        };
        out.push(data[i..(i + len).min(data.len())].to_vec());
        i += len;
    }
    out
}

/// 다른 앱(또는 다시 꽂은 USB 장치)의 입력처럼 보이는 목적지.
struct Destination {
    rx: mpsc::Receiver<Vec<u8>>,
    _dest: coremidi::VirtualDestination,
    _client: coremidi::Client,
}

impl Destination {
    fn new(name: &str) -> Destination {
        let (tx, rx) = mpsc::channel();
        let client = coremidi::Client::new(&format!("ks-rv7d-dest-{name}")).unwrap();
        let dest = client
            .virtual_destination(name, move |list: &coremidi::PacketList| {
                for p in list {
                    for msg in split(p.data()) {
                        let _ = tx.send(msg);
                    }
                }
            })
            .unwrap();
        let deadline = Instant::now() + WAIT;
        while !keyscore_midi::destinations()
            .iter()
            .any(|d| d.name() == name)
        {
            assert!(Instant::now() < deadline, "목적지 {name}이 보이지 않는다");
            std::thread::sleep(Duration::from_millis(10));
        }
        Destination {
            rx,
            _dest: dest,
            _client: client,
        }
    }
}

fn pump_collect(
    cx: &mut TestAppContext,
    rx: &mpsc::Receiver<Vec<u8>>,
    limit: Duration,
) -> Vec<Vec<u8>> {
    let mut got = Vec::new();
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        cx.run_until_parked();
        while let Ok(m) = rx.try_recv() {
            got.push(m);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    got
}

// ---------- 지적 3: 시트 글자 ----------

/// `shown`은 「다른 Mac의 장치 이름이 시트 글자를 깨뜨리지 않게」 한다고 하지만 C0/C1 제어 문자와 줄 구분자만
/// 고친다. 이름 규칙(`normalize_port_name`)이 바로 그 이유로 거부하는 양방향 제어 문자(RLO 등)와 폭 없는 문자는
/// 그대로 시트에 들어가, 줄의 뒷부분(「기본 포트 …으로 바꾼다」)을 뒤집어 보이거나 거부 이유를 숨긴다.
#[test]
fn review1_sheet_line_escapes_bidi_and_invisible_characters() {
    for bad in [
        "Bad\u{202E}Port",
        "Bad\u{200B}Port",
        "Bad\u{2066}Port",
        "Bad\u{FEFF}Port",
    ] {
        let p = project_on(&[named(bad)]);
        let plan = port_mapping::plan(&p, &["KeyScore Out".to_owned()], &[]);
        assert_eq!(
            plan,
            vec![Line {
                saved: bad.to_owned(),
                mapping: Mapping::Main(MainReason::InvalidName),
                tracks: 1,
            }]
        );
        let text = port_mapping::describe(&plan[0], "KeyScore Out");
        let raw = bad.chars().nth(3).unwrap();
        assert!(
            !text.contains(raw),
            "시트 줄에 {:?}가 그대로 들어간다: {text:?}",
            raw.escape_unicode().to_string()
        );
    }
}

// ---------- 제안 1: 바꾸지 않는 「바꾼다」 ----------

/// 정규화 이름이 같은 두 이름이 트랙 순서상 「정규화하지 않은 것」 먼저 나오면, 둘째 줄(이미 정규화된 이름)은
/// `Use(자기 이름)`이 되어 시트에 「「X」: 트랙 1개를 「X」로 바꾼다」가 나오고, 적용은 같은 값으로 다시 쓰는
/// 편집을 넣어 상태줄의 「트랙 n개를 바꿨다」에 센다.
#[gpui_kit::test]
fn review1_already_normalized_second_name_is_not_a_change(cx: &mut TestAppContext) {
    let dir = start(cx, "noop");
    let x = unique("noop synth");
    let spaced = x.replacen(' ', "  ", 1);
    let path = dir.join("song.ksq");
    std::fs::write(
        &path,
        ksq::to_string(&project_on(&[named(&spaced), named(&x)])),
    )
    .unwrap();
    let (_, view) = open_file(cx, &path);
    let text = prompt_text(cx);
    answer(cx, MAP);
    assert_eq!(ports_of(cx, &view), vec![named(&x), named(&x)]);
    let said = messages(cx, &view).join("\n");
    eprintln!("said: {said}");
    assert!(
        !text.contains(&format!("「{x}」: 트랙 1개를 「{x}」로 바꾼다")),
        "시트가 바뀌지 않는 줄을 「바꾼다」고 보인다:\n{text}"
    );
    assert!(
        said.contains("트랙 1개를 바꿨다"),
        "실제로 바뀐 트랙은 1개다: {said}"
    );
}

// ---------- 지적 2: 보이지 않은 제안의 적용 ----------

/// 시트에는 「같은 이름의 가상 포트를 만든다」만 보였는데, 시트가 떠 있는 동안 앱 가상 포트가 16개가 되면(여러
/// 파일을 한꺼번에 열어 다른 창이 먼저 매핑한 경우와 같다) [제안대로 매핑]이 다시 계산한 `Main(TooMany)`로 트랙을
/// 기본 포트로 바꾼다. 상태줄 요약은 「가상 포트 0개를 만들고 트랙 1개를 바꿨다」뿐이라 어느 트랙이 왜 기본
/// 포트로 갔는지 보이지 않는다.
#[gpui_kit::test]
fn review1_stale_sheet_does_not_silently_fall_back_to_main(cx: &mut TestAppContext) {
    if run_isolated("review1_stale_sheet_does_not_silently_fall_back_to_main") {
        return;
    }
    let dir = start(cx, "stale");
    let missing = unique("stale missing");
    let path = dir.join("song.ksq");
    std::fs::write(&path, ksq::to_string(&project_on(&[named(&missing)]))).unwrap();
    let (_, view) = open_file(cx, &path);
    let text = prompt_text(cx);
    assert!(text.contains("같은 이름의 가상 포트를 만든다"), "{text}");
    // 시트가 떠 있는 동안 다른 경로로 앱 가상 포트가 16개가 됐다
    while cx.read(engine::virtual_port_names).len() < keyscore_midi::MAX_VIRTUAL_OUTPUTS {
        cx.update(engine::add_virtual_port).unwrap();
    }
    answer(cx, MAP);
    let ports = ports_of(cx, &view);
    let said = messages(cx, &view).join("\n");
    eprintln!("ports: {ports:?}\nsaid: {said}");
    assert!(
        ports == vec![named(&missing)] || cx.has_pending_prompt() || said.contains("기본 포트"),
        "시트에 보이지 않은 기본 포트 대체가 알림 없이 적용됐다: {ports:?} / {said}"
    );
}

// ---------- 지적 1: 돌아온 장치를 가리는 가상 포트 ----------

/// 같은 Mac에서 USB 신디를 잠시 뺀 채 프로젝트를 열면(OUT-3의 흔한 경우) 시트가 그 장치 이름으로 앱 가상 포트를
/// 만들자고 한다. 받아들인 뒤 장치를 다시 꽂으면 엔진은 같은 이름의 앱 포트를 먼저 열어(`open_port`) 트랙의
/// 노트가 장치로 가지 않는다. 포트 메뉴에도 그 장치가 보이지 않는다(`port_choices`). OUT-3 「같은 이름의 포트가
/// 돌아오면 자동 재연결」이 이 매핑 뒤에는 성립하지 않고, 시트는 이 결과를 알리지 않는다.
#[gpui_kit::test]
fn review1_mapped_port_does_not_shadow_returning_device(cx: &mut TestAppContext) {
    if run_isolated("review1_mapped_port_does_not_shadow_returning_device") {
        return;
    }
    let dir = start(cx, "shadow");
    let device = unique("usb synth");
    let path = dir.join("song.ksq");
    std::fs::write(&path, ksq::to_string(&project_on(&[named(&device)]))).unwrap();
    let (handle, view) = open_file(cx, &path);
    let text = prompt_text(cx);
    answer(cx, MAP);
    assert!(cx.read(engine::virtual_port_names).contains(&device));
    // 장치가 돌아왔다
    let back = Destination::new(&device);
    let choices = cx.read(engine::port_choices);
    run(cx, handle, transport::Play);
    let got = pump_collect(cx, &back.rx, Duration::from_millis(1500));
    run(cx, handle, transport::Stop);
    let notes = got.iter().filter(|m| m.first() == Some(&0x90)).count();
    eprintln!(
        "sheet: {text}\nchoices: {choices:?}\nnotes to device: {notes}\nsaid: {:?}",
        messages(cx, &view)
    );
    assert!(
        notes > 0 || text.contains("가린다") || text.contains("돌아와도"),
        "돌아온 장치로 노트가 가지 않고, 시트도 이를 알리지 않았다(OUT-3)"
    );
}

/// 대조군: 매핑하지 않으면([그대로 두기]) 같은 시험 장치로 노트가 간다(위 시험의 0개가 시험 장치 탓이 아니다).
#[gpui_kit::test]
fn review1_control_unmapped_track_reaches_returning_device(cx: &mut TestAppContext) {
    if run_isolated("review1_control_unmapped_track_reaches_returning_device") {
        return;
    }
    let dir = start(cx, "shadow-control");
    let device = unique("usb synth control");
    let path = dir.join("song.ksq");
    std::fs::write(&path, ksq::to_string(&project_on(&[named(&device)]))).unwrap();
    let (handle, _view) = open_file(cx, &path);
    answer(cx, "그대로 두기");
    let back = Destination::new(&device);
    run(cx, handle, transport::Play);
    let got = pump_collect(cx, &back.rx, Duration::from_millis(1500));
    run(cx, handle, transport::Stop);
    let notes = got.iter().filter(|m| m.first() == Some(&0x90)).count();
    assert!(notes > 0, "대조군: 장치로 노트가 가지 않는다");
}
