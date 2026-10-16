# Task 계획: S1-T5 메트로놈과 미리듣기

- 브랜치: `s1/t5-metronome-preview`
- 관련 REQ: 6.3 MET-1, MET-2, MET-3, PRE-1, 9.2(실시간 스레드 규칙)
- 선행 Task: S1-T4(스케줄러)
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

- 스케줄러가 재생 중 박자표(`MeterMap`)의 박마다 클릭을 보낸다.
  - 강박(마디 첫 박)은 강박 노트, 나머지는 약박 노트다.
  - 포트·채널·노트·벨로시티·길이는 `MetronomeConfig`로 정한다. 기본값은 채널 10, 강박 37, 약박 42다.
  - 포트는 스냅샷의 포트 번호라 트랙 출력과 다른 포트를 쓸 수 있다(MET-3).
- 클릭은 「버튼」(`set_metronome_enabled`)과 「강제」(`set_metronome_forced`, 녹음·카운트인용) 가운데 하나라도 켜져 있으면 나간다.
- 클릭 Note On은 박 틱의 정확한 호스트 시각에 나간다. 이벤트·Clock과 같은 정수 계산을 쓴다. Note Off는 길이만큼 뒤에 나간다.
- 재생 중에는 박마다 `Beat { bar, beat, at }`을 UI 쪽 큐로 낸다(MET-2 시각 메트로놈). 클릭이 꺼져 있어도 낸다. 큐가 가득 차면 버린다(화면 표시는 하나 빠져도 된다).
- `Scheduler::preview(port, channel, pitch, velocity, length)`는 Note On을 지금 보내고 `length` 뒤에 Note Off를 보낸다(PRE-1, 기본 200 ms). 정지 중에도 동작하고, 패닉 없이도 노트가 남지 않는다.
- 같은 노트를 끄기 전에 다시 미리들으면 앞 노트를 먼저 끄고 새 길이로 다시 켠다(뒤 노트가 앞 노트의 Note Off로 일찍 끊기지 않게).

단계 파일과 다른 점은 없다. 클릭 길이 기본값(50 ms)과 벨로시티 기본값(강박 127, 약박 100)은 REQ에 없어 흔한 값으로 정했다. 둘 다 설정으로 바꿀 수 있다(MET-1 「길이는 설정 > 메트로놈」).

## 공개 인터페이스 초안

~~~rust
pub const DEFAULT_PREVIEW_LENGTH: Duration; // 200 ms
pub const MAX_NOTE_LENGTH: Duration; // 10 s, 미리듣기·클릭 길이 상한(넘으면 줄인다)

pub struct MetronomeConfig { /* 비공개 필드 */ }
impl MetronomeConfig {
    pub fn new(port: u16, channel: Channel, accent: Pitch, beat: Pitch) -> Self;
    pub fn with_velocities(self, accent: Velocity, beat: Velocity) -> Self;
    pub fn with_length(self, length: Duration) -> Self;
    // getter: port, channel, accent, beat, accent_velocity, beat_velocity, length
}
impl Default for MetronomeConfig; // 포트 0, 채널 10, 37/42, 127/100, 50 ms

pub struct Beat { /* bar, beat, at */ }
impl Beat { pub fn bar(&self) -> u64; pub fn beat(&self) -> u32; pub fn at(&self) -> HostTime; }

impl Snapshot {
    pub fn with_meter(self, meter: MeterMap) -> Self; // 기본은 4/4
    pub fn meter(&self) -> &MeterMap;
}

impl Scheduler {
    pub fn set_metronome(&mut self, config: MetronomeConfig) -> Result<(), MidiError>;
    pub fn set_metronome_enabled(&mut self, on: bool) -> Result<(), MidiError>;
    pub fn set_metronome_forced(&mut self, on: bool) -> Result<(), MidiError>;
    pub fn poll_beat(&mut self) -> Option<Beat>;
    pub fn preview(&mut self, port: u16, channel: Channel, pitch: Pitch, velocity: Velocity,
                   length: Duration) -> Result<(), MidiError>;
}
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `metronome_clicks_on_beat_grid_with_accent_on_bar_start` | 통합(루프백) | 120 BPM 4/4에서 클릭 Note On 타임스탬프가 박 틱의 호스트 시각과 1 µs 안, 강박 37·약박 42, 채널 10 |
| 2 | `metronome_follows_meter_change_and_short_bar` | 통합 | 3/4 → 마디 중간 6/8 변화에서 박 격자와 강박 위치 |
| 3 | `metronome_click_note_off_after_configured_length` | 통합 | Note Off 타임스탬프 = Note On + 길이(±1 µs) |
| 4 | `metronome_off_sends_no_click_and_forced_overrides_button` | 통합 | 버튼 끔이면 클릭 없음, 강제 켬이면 버튼과 무관하게 클릭 |
| 5 | `metronome_uses_its_own_port_and_channel` | 통합 | 포트 둘 중 지정 포트로만 클릭(MET-3) |
| 6 | `metronome_restarts_grid_after_locate_and_loop_wrap` | 통합 | 위치 이동·루프 되감기 뒤 첫 클릭이 다음 박(박 위면 그 자리) |
| 7 | `beats_are_reported_for_every_beat_while_playing` | 통합 | `poll_beat`이 (마디, 박, 시각)을 클릭과 같은 시각으로 낸다. 클릭이 꺼져도 낸다 |
| 8 | `preview_sends_note_off_after_default_length` | 통합 | 정지 중 미리듣기: Note On 뒤 200 ms(±1 ms 도착)에 Note Off, 패닉 없이 |
| 9 | `preview_retrigger_same_note_restarts_length` | 통합 | 같은 노트 두 번: 앞 노트를 먼저 끄고, 마지막 Note Off는 두 번째 + 200 ms |
| 10 | `many_previews_never_leave_a_note_on` | 통합 | 노트 100개를 빠르게 미리들어도 Note On 수 = Note Off 수 |
| 11 | `metronome_and_preview_do_not_allocate` | 통합(할당) | `assert_no_alloc` 전역 할당자 아래 클릭·박 보고·미리듣기·되감기 |
| 12 | `metronome_config_default_matches_req` | 단위 | 기본값 채널 10, 37, 42, 길이 50 ms, 미리듣기 200 ms |

## 불변식

- 실시간 루프에서 할당·락·로그 없음. 끄기 대기 표(64칸)와 박 큐(`rtrb`, 256칸)는 시작할 때 만든다.
- 끄기 대기 표가 가득 차면 가장 이른 대기를 지금 보내고 칸을 다시 쓴다. Note On을 보내고 Note Off를 잃는 경로가 없다.
- 클릭과 미리듣기의 Note On·Off도 울리는 노트 표(`active`)에 들어가 정지·패닉·교체 규칙을 그대로 따른다.
- 같은 틱이면 이벤트, 클릭, Clock 순이다. 루프 끝과 같은 틱의 클릭은 다음 바퀴의 것이다.
- `MetronomeConfig`, `Beat`는 `pub` 필드가 없다.

## 범위 밖

- 카운트인(재생 전 마디 수)과 녹음 상태: S5. 이 Task는 「강제」 플래그만 준다.
- 설정 화면과 저장, 트랜스포트 버튼, 상태줄·테두리 그리기: S2·S4(UI).
- 미리듣기를 언제 부를지(커서 이동, Step 입력): S3·S5.
- 복합 박자(6/8을 두 박으로 세기) 강박 규칙: 박자표의 분모 박마다 클릭하고 마디 첫 박만 강박이다.

## 리스크·결정 요청

- 클릭 길이·벨로시티 기본값은 REQ에 없다. 설정 가능한 세부값이라 결정 요청 없이 흔한 값을 쓰고 보고서에 적는다.
- 결정 요청 없음.
