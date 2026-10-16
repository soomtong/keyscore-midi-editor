# Task 계획: S1-T4 스케줄러 스레드

- 브랜치: `s1/t4-scheduler`
- 관련 REQ: 9.2(스레드 모델, 실시간 스레드 금지 사항), 9.3, OUT-5, OUT-6, OUT-7, CTL-6(재생 시작 chase), TRN-3(재생 중 편집), NF-1, NF-3, NF-4
- 선행 Task: S1-T1(포트, `MidiPort`), S1-T3(`Midi1Message`, `Group`), S0-T3(`TempoMap`)
- 관련 스파이크: `docs/spikes/sp3-coremidi.md`(CoreMIDI가 미래 타임스탬프를 지킴, 0 타임스탬프 금지)

## 끝의 정의 (내 말로)

- `Scheduler`가 실시간 우선순위 스레드 하나를 띄운다. 그 스레드는 미리 만든 `Snapshot`(불변, `Arc`)의 이벤트를 템포 맵으로 호스트 시각에 맞춰, 수 ms 앞서 `MidiPort::send`로 예약 송신한다(OUT-7).
- 제어 스레드와 스케줄러 스레드 사이 통신
  - 둘 다 락 없는 SPSC 링버퍼(`rtrb`)다.
  - 명령(제어 → 스케줄러): 재생(위치), 정지, 위치 이동, 루프 범위, 스냅샷 교체, 패닉, MIDI Clock 켬/끔, 즉시 송신(미리듣기·NF-1 측정)
  - 돌려받기(스케줄러 → 제어): 교체돼 쓰지 않게 된 스냅샷. 그래서 해제(할당 해제)가 스케줄러 스레드에서 일어나지 않는다.
- 스냅샷 교체는 원자적이다(TRN-3). 스케줄러는 명령 하나로 새 스냅샷으로 넘어가고, 그 뒤에는 옛 스냅샷의 이벤트를 보내지 않는다. 울리고 있는 노트는 교체 때 Note Off를 보낸다.
- 재생 시작과 위치 이동 때 그 위치까지의 프로그램 체인지·뱅크·CC 마지막 값을 먼저 보낸다(chase, CTL-6).
- 정지·위치 이동·루프 되감기·패닉 때 울리고 있는 노트를 끈다.
  - 패닉(OUT-6)은 모든 포트·채널에 All Sound Off, All Notes Off, Sustain 0을 보낸다.
- MIDI Clock(OUT-5, 기본 끔): 켜면 24 ppqn Clock, 재생 시작 때 Start(위치 0) 또는 SPP 뒤 Continue, 정지 때 Stop을 보낸다.
- 스케줄러 루프는 할당·락·로그가 없다. 디버그 빌드에서 `assert_no_alloc`으로 검사한다.
  - 시스템 콜은 MIDI 송신과 시간 대기(`mach_wait_until`)뿐이다.
  - 대기는 9.2의 「시스템 콜 금지」의 예외로 본다. 스케줄러가 시각을 기다리는 유일한 방법이다.

## 공개 인터페이스 초안

~~~rust
pub struct ScheduledEvent { /* tick: Tick, port: u16, msg: Midi1Message */ }
pub struct Snapshot { /* tempo: TempoMap, events: Vec<ScheduledEvent>(정렬), ports: Vec<Arc<dyn MidiPort>> */ }
impl Snapshot {
    pub fn new(tempo: TempoMap, ports: Vec<Arc<dyn MidiPort>>, events: Vec<ScheduledEvent>) -> Result<Self, MidiError>;
}
pub struct Scheduler { /* 스레드, 명령 Producer, 돌려받기 Consumer */ }
impl Scheduler {
    pub fn start(snapshot: Arc<Snapshot>, config: SchedulerConfig) -> Result<Self, MidiError>;
    pub fn play(&mut self, from: Tick) -> Result<(), MidiError>;       // 큐가 가득이면 Err(QueueFull)
    pub fn stop / locate(Tick) / set_loop(Option<(Tick, Tick)>) / swap(Arc<Snapshot>) / panic / set_clock(bool)
    pub fn send_now(&mut self, port: u16, msg: Midi1Message) -> Result<(), MidiError>;
    pub fn collect_garbage(&mut self);                                // 돌려받은 스냅샷 해제
}
pub struct SchedulerConfig { pub lookahead: Duration /* 기본 10 ms */, pub wake: Duration /* 기본 1 ms */ }
~~~

정렬 규칙: 같은 틱에서는 Note Off가 Note On보다 먼저다. 같은 음을 끊고 다시 칠 때 순서가 뒤집히지 않게 하려는 것이다. 그다음 프로그램·뱅크·CC, 마지막이 Note On이다.

## 세부 결정 (위임, 베스트 프랙티스)

- 우선순위: macOS time-constraint 정책(`thread_policy_set`, `THREAD_TIME_CONSTRAINT_POLICY`)이다. `audio_thread_priority` crate는 쓰지 않는다. Linux 의존(`libdbus-sys`)을 끌고 오기 때문이다. FFI는 함수 단위 `#[allow(unsafe_code)]`로 쓴다.
- 시각: 호스트 시각은 `mach_absolute_time` 틱이다. 템포 맵의 `MusicTime`(1/960 ns 정수)을 ns로 바꾼 뒤 `mach_timebase_info` 비율로 틱을 만든다. 정수 산술만 쓴다(드리프트 없음, NF-4).
- 재생 기준점: 재생 시작 때 `(시작 틱, 시작 호스트 시각)`을 잡는다. 이벤트 호스트 시각은 기준 호스트 시각 + (이벤트 틱 시각 - 시작 틱 시각)이다. 누적 덧셈이 아니라 매번 기준점에서 계산한다.
- 미리 보낼 창: 지금부터 `lookahead`(기본 10 ms) 안에 오는 이벤트를 예약 송신한다. 깨는 주기(1 ms)보다 길어야 한다.
- 송신 실패(`DestinationGone` 등)는 스케줄러를 멈추지 않는다. 그 이벤트를 버리고 계속한다.
- 상태 표(울리는 노트, chase 값)는 스케줄러가 시작할 때 미리 할당한 고정 크기 배열이다. 포트는 최대 `MAX_SCHEDULER_PORTS = 32`개다.

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `snapshot_orders_note_off_before_note_on_at_same_tick` | 단위 | 정렬 규칙 |
| 2 | `tick_to_host_is_integer_exact_across_tempo_changes` | 단위 | 템포 변화가 있어도 기준점 계산이 정확 |
| 3 | `play_sends_events_at_scheduled_host_times` | 통합(루프백) | 수신 타임스탬프 = 계산한 호스트 시각 |
| 4 | `send_now_latency_p99_within_3ms` | 통합 | NF-1(명령 투입 → 수신 도착) p99 ≤ 3 ms, 수치를 출력 |
| 5 | `playback_jitter_p99_within_half_ms_short` | 통합 | NF-3 축약판(20초). 10분판은 `KS_LONG_TESTS=1`, 1시간판은 `KS_LONG_TESTS=hour` |
| 6 | `playback_drift_within_1ms_short` | 통합 | NF-4 축약판(같은 조건) |
| 7 | `swap_during_play_stops_old_events_and_releases_notes` | 통합 | TRN-3 원자 교체 |
| 8 | `stop_locate_and_loop_release_sounding_notes` | 통합 | Note Off 송신 |
| 9 | `play_from_middle_chases_program_bank_and_cc` | 통합 | CTL-6 chase 순서와 값 |
| 10 | `panic_sends_all_sound_off_notes_off_sustain_to_every_port_channel` | 통합 | OUT-6 |
| 11 | `clock_sends_24_ppqn_with_start_spp_continue_stop` | 통합 | OUT-5 |
| 12 | `scheduler_loop_does_not_allocate` | 통합(별도 바이너리) | `assert_no_alloc` 전역 할당자로 재생·교체·위치 이동·루프·패닉 중 할당 0 |
| 13 | `full_command_queue_returns_queue_full` | 단위 | 큐 포화 |
| 14 | `garbage_snapshots_are_returned_not_dropped_on_scheduler_thread` | 통합 | 교체된 스냅샷이 돌려받기 큐로 옴 |

수치(4, 5, 6)는 테스트가 표준 출력에 적고 보고서에 옮긴다.

## 불변식

- 스케줄러 루프 안의 모든 호출에는 할당·락·로그가 없다.
  - 루프 안에서 하는 일: 명령 꺼내기(`rtrb`), 이벤트 읽기, 정수 시각 계산, `MidiPort::send`, 돌려받기 큐에 넣기, `mach_wait_until`
  - 테스트 12가 검사한다.
- 스냅샷은 스케줄러 스레드에서 해제되지 않는다. 돌려받기 큐가 가득하면 그 스냅샷을 다음 루프까지 들고 있는다.
- 우리 포트로 보내는 워드는 `Midi1Message::ump(Group)`로만 만든다(S1-T3 이월).

## 범위 밖

- 메트로놈(S1-T5), SMF(S1-T6), CLI 재생(S1-T7)
- CC 샘플링 보간(CTL-6의 5 ms 보간): S6-T3. 여기서는 이벤트로 주어진 CC를 그대로 보내고 chase만 한다.
- 모델 → 스냅샷 변환(트랙·클립·뮤트·솔로를 이벤트로 펼치기): 단순한 변환 함수 하나만 두고, 라우팅 UI는 S2·S4

## 리스크·결정 요청

- 결정 요청 없음.
- 측정은 이 기기(M1 Max)에서 한다. CI 러너 수치는 remote가 생긴 뒤 본다.
