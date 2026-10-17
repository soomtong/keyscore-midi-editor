# Task 계획: S2-T5a MIDI 엔진 연결과 트랜스포트 Action

- 브랜치: `s2/t5-transport-panels`(S2-T5를 셋으로 나눈 첫 Task, 단계 파일 「나눔」)
- 관련 REQ: TRN-1, TRN-2(메트로놈 토글), TRN-3(S, 재생 중 편집 반영), OUT-1, OUT-4, OUT-6, MET-1, SEQ-7, IO-1
- 선행 Task: S1(스케줄러·렌더러·포트), S2-T4(실행 함수)
- 관련 결정: `2026-10-12-ksq-unknown-keys-on-value-change.md`(B), `2026-10-16-mute-solo-precedence.md`

## 끝의 정의 (내 말로)

1. core: 포트·조성의 모르는 키(결정 B)
   - `SetTrackProp(Port)`와 `SetKey`가 값을 바꾸면 그 객체의 모르는 키(`port_extra`, `key_extra`)를 지운다.
   - 같은 값을 다시 넣는 편집은 모르는 키를 지우지 않는다.
   - 역편집(`Op`)이 옛 값과 옛 모르는 키를 함께 담아, undo가 둘을 되살리고 redo가 다시 지운다.
   - 공개 `Edit`는 바뀌지 않는다.
2. MIDI 엔진(`crates/keyscore-app/src/engine.rs`, 앱 전역 하나)
   - `engine::install(cx, EngineOptions)`: 프로세스의 첫 CoreMIDI 호출로 `MidiClient::new`를 부른다(S1-T2 이월). 이어서 가상 포트 「KeyScore Out」(OUT-1)을 만들고, 빈 스냅샷으로 스케줄러를 띄운다.
   - 바이너리(`run`)는 창을 열기 전에 설치한다. `init`은 CoreMIDI를 건드리지 않는다. 그래서 MIDI가 필요 없는 테스트는 MIDI 서버와 무관하게 돈다.
   - 설치 실패(MIDI 서버 없음 등)는 앱을 멈추지 않는다. 엔진 없이 뜨고, 트랜스포트 Action은 메시지만 남긴다.
   - 트랙 → 포트 라우팅(SEQ-7, OUT-4)
     - `Virtual`은 기본 가상 포트로 간다.
     - `Named`는 OS 목적지가 있으면 연다(한 번 연 포트는 이름으로 재사용).
     - 없으면 그 트랙을 소리 내지 않고 경고를 남긴다. 같은 이름의 포트가 돌아올 때의 재연결은 S2-T5c(OUT-3)다. 가상 포트 추가는 설정(S2-T6)이다.
   - 스냅샷: 프로젝트를 `render`로 바꾸고 템포·박자 맵을 싣는다. 뮤트·솔로는 렌더러 규칙을 따른다.
3. 트랜스포트(창마다 상태, 엔진은 하나)
   - Action: `transport::Play`, `Stop`, `Toggle`, `Pause`, `PlayFromCursor`, `Panic`, `ToggleMetronome`.
     - `PlayFromCursor`는 커서가 생기기 전(S3)까지 Play와 같다.
     - `ToggleLoop`·`LoopSelection`은 구간 선택이 생기는 S3·S4로 미룬다.
   - 재생 시작 때 각 트랙의 뱅크·프로그램을 보낸다(OUT-4). 틱 0이 아닌 곳에서 시작해도 음색이 맞게 하기 위해서다.
   - 일시정지는 멈춘 자리를 기억하고, 다음 재생이 거기서 이어진다. 정지는 처음으로 돌아간다. 자리는 재생 시작 호스트 시각과 템포 맵으로 앱이 계산한다(스케줄러에 위치 조회가 없다).
   - 한 번에 한 창만 재생한다. 다른 창이 재생을 시작하면 앞 창은 정지 상태가 된다.
   - 재생 중 문서가 바뀌면 스냅샷을 다시 만들어 교체한다(TRN-3, `Scheduler::swap`).
   - 메트로놈 토글(TRN-2, MET-1): 기본 설정(기본 가상 포트, 채널 10, 강박 37, 약박 42)으로 켜고 끈다. 포트·채널·노트 설정 화면은 S2-T6이다.
   - 패닉(OUT-6): 스케줄러 패닉(전 포트·채널 All Notes Off·All Sound Off·서스테인 해제).
   - 창이 닫히면 그 창의 재생을 멈춘다.
4. 창이 남기는 메시지(엔진 없음, 없는 포트, 스케줄러 오류)는 `ProjectWindow::messages()`로 보인다. 상태줄 표시는 S2-T5c다.

## 공개 인터페이스 초안

~~~rust
pub mod engine {
    pub struct EngineOptions { pub client_name: String, pub port_name: String }
    pub fn install(cx: &mut App, options: EngineOptions) -> Result<(), MidiError>;
    pub fn is_installed(cx: &App) -> bool;
    pub enum TransportState { Stopped, Playing, Paused(Tick) }
}
impl ProjectWindow {
    pub fn transport_state(&self) -> TransportState;
    pub fn metronome_on(&self) -> bool;
    pub fn messages(&self) -> &[SharedString];
}
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `port_change_clears_unknown_keys_and_undo_restores_them` | 단위(core) | 포트를 바꾸면 모르는 키가 사라지고, undo는 값과 키를 되살리고, redo는 다시 지운다. 같은 값이면 지우지 않는다. `.ksq` 왕복으로 확인 |
| 2 | `key_change_clears_unknown_keys_and_undo_restores_them` | 단위(core) | 조성도 같다 |
| 3 | `unknown_key_edits_keep_history_invariants` | 속성(core, proptest) | 포트·조성 편집을 무작위로 섞은 뒤 전부 undo하면 처음 `.ksq`와 같다 |
| 4 | `play_action_sends_notes_to_virtual_port` | UI + 루프백(단계 게이트) | 노트 있는 프로젝트에서 `transport::Play` → 루프백 수신기가 Note On을 받는다. 시작 때 프로그램 체인지를 먼저 받는다 |
| 5 | `stop_and_panic_silence_output` | UI + 루프백 | 재생 중 Stop은 울리는 노트의 Note Off를, Panic은 CC 120·123·64를 보낸다 |
| 6 | `pause_resumes_from_paused_position` | UI + 루프백 | 일시정지 뒤 재생은 멈춘 자리 근처에서 이어진다. 정지 뒤 재생은 처음부터다 |
| 7 | `metronome_toggle_sends_clicks` | UI + 루프백 | 메트로놈을 켜고 재생하면 채널 10 노트 37·42를 받는다. 끄면 받지 않는다 |
| 8 | `edit_while_playing_is_heard` | UI + 루프백 | 재생 중 노트를 더하면(문서 편집) 그 노트가 들린다(TRN-3) |
| 9 | `missing_named_port_is_reported_and_silent` | UI | 없는 이름 포트의 트랙은 소리 내지 않고 메시지를 남긴다. 다른 트랙은 울린다 |
| 10 | `transport_without_engine_reports_message` | UI | 엔진 없이 Play → 메시지, 상태 Stopped, 패닉 없음 |
| 11 | `only_one_window_plays` | UI + 루프백 | 둘째 창 Play → 첫 창은 Stopped |

루프백 테스트는 같은 프로세스의 `coremidi` 수신기로 받는다(keyscore-midi 테스트와 같은 방식). 앱 테스트 dev-dependency로 `coremidi`(MIT)를 더한다. 포트 이름은 테스트마다 고유하다.

## 불변식

- 앱이 부르는 첫 CoreMIDI 호출은 `MidiClient::new`다.
- 실시간 스레드는 S1 스케줄러 그대로다. 앱은 제어 쪽 API(`play`, `stop`, `swap`, `panic`, `send_now`, `set_metronome*`)만 부른다.
- 포트·조성 편집은 히스토리를 거치고, undo가 모르는 키까지 되살린다.

## 범위 밖

- 트랜스포트·트랙 목록·인스펙터·상태줄 화면(S2-T5b·T5c), 포트 감시·재연결·재등록(S2-T5c)
- 루프 구간, 커서 위치 재생(S3·S4), MIDI Clock(OUT-5, S2-T6 설정과 함께)
- 가상 포트 추가·이름 변경, 메트로놈 설정 화면(S2-T6)

## 리스크·결정 요청

- 없는 이름 포트를 소리 내지 않는 것은 ksq-play(같은 이름 가상 포트를 만듦)와 다르다. 앱에서는 포트를 사용자 몰래 만들지 않고 알린다. OUT-2(가상 포트는 사용자가 정한다)와 OUT-3(돌아오면 재연결)에 맞춘 세부 판단이다.
