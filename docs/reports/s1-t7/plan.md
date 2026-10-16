# Task 계획: S1-T7 `.ksq` 재생 도구

- 브랜치: `s1/t7-ksq-play`
- 관련 REQ: 4 P4·P5, 6.4 OUT-1·OUT-6, 6.3 MET-1·MET-3, SEQ-7(트랙 = 포트·채널), 5.10(뮤트·솔로), 결정 2026-10-12 Q1(숨김 꼬리는 클립 끝에서 잘림)
- 선행 Task: S1-T1(가상 포트), S1-T2(OS 포트·매핑), S1-T4(스케줄러), S1-T5(메트로놈), S0-T6(`.ksq` 읽기)
- 관련 스파이크: SP-3(Surge XT 수신)

## 끝의 정의 (내 말로)

1. `keyscore-midi`에 모델 → 스케줄러 이벤트 변환 `render`가 생긴다. 앱(S2)도 같은 함수를 쓴다.
   - 뮤트된 트랙은 빼고, 솔로가 하나라도 있으면 솔로 트랙만 넣는다.
   - 클립을 펼치고 숨김 꼬리는 클립 끝에서 자른다.
   - 트랙 뱅크·프로그램은 틱 0에 CC 0·32, 프로그램 체인지 순으로 보낸다.
   - 트랙마다 포트 번호는 호출자가 정한다.
   - 곡 끝 틱(가장 늦은 Note Off)도 알려 준다.
2. 새 바이너리 crate `tools/ksq-play`(바이너리 `ksq-play`)가 `.ksq`·`.ksqz`를 재생한다.
   - 포트
     - 기본은 가상 포트 「KeyScore Out」이다.
     - `OutputPort::Named` 트랙은 같은 이름의 OS 목적지가 있으면 거기로 보낸다(`suggest_mapping`).
     - 그 목적지가 없으면 가상 포트로 보내고 표준 오류에 알린다.
   - 옵션
     - `--port <이름>`: 가상 포트 이름
     - `--from <마디>`: 1부터 센다
     - `--metronome`: 클릭 켬(MET-1 기본 채널 10, 37/42)
     - `--metronome-port <OS 목적지 이름>`: MET-3
     - `--wait <초>`: 재생 전 대기. 신디가 포트를 잡을 시간이다(SP-3 수동 확인용)
   - 재생 중 표준 입력
     - `p` + Enter: 패닉(OUT-6)
     - `q` + Enter: 정지하고 끝낸다
   - Ctrl-C를 받으면 패닉과 정지를 보낸 뒤 끝낸다. 걸린 노트를 남기지 않는다.
   - 곡 끝에 닿으면 정지하고 종료 코드 0으로 끝낸다.

단계 파일과 다른 점: 단계 파일은 「`ksq-cli play`」라고 적었다. 그러나 00-conventions와 `scripts/check-deps.sh`는 `ksq-cli`가 core와 text만 쓴다고 정한다. 픽스처·변환 도구가 CoreMIDI 없이 돌아야 하기 때문이다. 그래서 재생은 별도 crate `tools/ksq-play`(core + midi)에 둔다. 00-conventions 저장소 구조와 `check-deps.sh`의 내부 의존 표에 `ksq-play`를 더한다. REQ와는 무관한 구조 결정이다.

## 공개 인터페이스 초안

~~~rust
// keyscore-midi/src/render.rs
pub struct Rendered { /* events: Vec<ScheduledEvent>, end: Tick */ }
impl Rendered { pub fn events(&self) -> &[ScheduledEvent]; pub fn into_events(self) -> Vec<ScheduledEvent>; pub fn end(&self) -> Tick; }
/// `port_of`가 None을 돌려준 트랙은 넣지 않는다.
pub fn render(project: &Project, port_of: impl FnMut(usize, &Track) -> Option<u16>) -> Rendered;

// tools/ksq-play/src/lib.rs
pub struct Options { /* path, port_name, from_bar, metronome, metronome_port, wait */ }
pub fn parse_args(args: &[OsString]) -> Result<Options, UsageError>;
pub fn run(args: &[OsString], input: impl BufRead + Send + 'static, out: &mut dyn Write, err: &mut dyn Write) -> u8;
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `render_skips_muted_tracks_and_honours_solo` | 단위 | 뮤트·솔로 규칙 |
| 2 | `render_cuts_hidden_tail_at_clip_end` | 단위 | 분할로 생긴 꼬리 노트의 Note Off가 클립 끝 |
| 3 | `render_sends_bank_then_program_at_tick_zero` | 단위 | CC 0, CC 32, 프로그램 순서와 채널 |
| 4 | `render_uses_track_channel_and_port_mapping` | 단위 | `port_of`의 번호, None이면 트랙 제외 |
| 5 | `render_end_is_last_note_off` | 단위 | 곡 끝 틱 |
| 6 | `parse_args_accepts_options_and_rejects_bad_usage` | 단위 | 옵션 해석, 종료 코드 2 |
| 7 | `play_fixture_sends_every_note_on_and_off_to_virtual_port` | 통합(루프백) | 픽스처 1곡을 빠른 템포로 바꿔 재생: Note On 수 = 노트 수, 끝에 걸린 노트 없음, 종료 코드 0 |
| 8 | `panic_command_from_stdin_silences_all_channels` | 통합(루프백) | `p` 입력에 CC 120·123·64가 16채널로 오고 그 뒤 걸린 노트 없음 |
| 9 | `quit_command_stops_and_releases_notes` | 통합(루프백) | `q` 입력에 정지, 울리던 노트의 Note Off, 종료 코드 0 |
| 10 | `named_port_missing_falls_back_to_virtual_with_warning` | 통합 | 없는 OS 목적지 이름 트랙은 가상 포트로 가고 표준 오류에 경고 |
| 11 | `metronome_option_clicks_on_channel_ten` | 통합(루프백) | `--metronome`이면 채널 10 클릭 |

수동(단계 게이트, 사람이 듣는다): Surge XT가 「KeyScore Out」을 입력으로 잡고 픽스처 1곡을 끝까지 재생, 패닉 뒤 걸린 노트 없음. 출구 보고서에 절차를 적는다.

## 불변식

- `render`의 이벤트는 같은 트랙에서 Note On 수 = Note Off 수다. 이벤트 열은 `Snapshot::new`가 정렬한다.
- 재생 도구는 끝날 때(곡 끝, `q`, Ctrl-C, 오류) 반드시 정지를 보내고, `Scheduler`가 drop 때 노트를 끈다(S1-T4).
- `ksq-cli`의 의존은 바뀌지 않는다.

## 범위 밖

- 앱의 트랜스포트·포트 설정 UI(S2), 루프 재생 옵션, 템포 바꾸기 옵션, SMF 재생(`.mid` 입력)은 하지 않는다.
- Ctrl-C 처리를 위해 `ctrlc` crate(MIT/Apache-2.0)를 `ksq-play`에만 더한다.

## 리스크·결정 요청

- 결정 요청 없음. `ksq-play` 분리는 위 「단계 파일과 다른 점」에 적은 구조 결정이다.
- 루프백 테스트는 CoreMIDI 서버에 기대므로 S1-T4처럼 흔들릴 수 있다. `scripts/midi-soak.sh`로 반복해 본다.
