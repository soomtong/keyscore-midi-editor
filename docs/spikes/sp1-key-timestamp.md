# SP-1 GPUI 키 이벤트 타임스탬프

- 수행일: 2026-10-11
- 코드: `spikes/sp1-key-timestamp/` (독립 cargo 프로젝트, 빈 `[workspace]`)
- GPUI: crates.io `gpui-kit = "=0.7.1"`(`default-features = false`)을 썼다. gpui-kit은 `pub use ::gpui::*`로 GPUI를 re-export하고, 그 GPUI는 gpui-kit이 고정한 `gpui-pre =0.3.8`이다(`Cargo.lock`에서 확인). path 의존은 쓰지 않았다.
- 추가 의존: `objc2 0.6`, `block2 0.6`(MIT), `objc2-app-kit 0.3`, `objc2-foundation 0.3`(Zlib OR Apache-2.0 OR MIT). 모두 gpui-pre-macos가 이미 끌어오는 버전이라 의존 그래프에 새 버전이 늘지 않는다.

## 질문

gpui-kit이 고정한 GPUI 스냅샷(gpui-pre 0.3.8)에서 `NSEvent.timestamp`가 키 이벤트·Action 핸들러까지 전달되는가? 전달되지 않으면 `NSEvent.addLocalMonitorForEvents`로 받은 타임스탬프를 GPUI 핸들러와 정확히 짝지을 수 있는가?

## 방법

### 1. 소스 분석 (`~/.cargo/registry/src/index.crates.io-*/`)

- `gpui-pre-0.3.8/src/interactive.rs:25-35` `KeyDownEvent`의 필드는 `keystroke`, `is_held`, `prefer_character_input` 셋뿐이다. `KeyUpEvent`도 `keystroke`뿐.
- `gpui-pre-0.3.8/src/platform/keystroke.rs:18-33` `Keystroke`는 `modifiers`, `key`, `key_char`뿐이다.
- `gpui-pre-macos-0.3.8/src/events.rs:131-137` `NSKeyDown` → `KeyDownEvent` 변환에서 `native_event.timestamp`를 읽지 않는다. macOS 플랫폼 crate 전체에서 `timestamp`는 `display_link.rs`(CVTimeStamp)에만 나온다.
- Action 핸들러 시그니처는 `Fn(&A, &mut Window, &mut App)`이고 Action 값은 키맵에 등록된 값 그대로라 이벤트 정보가 실리지 않는다.
- `gpui-pre-0.3.8/src/window.rs:1263-1290` `InputRateTracker`는 입력 시각을 `Instant::now()`로 찍는다(OS 타임스탬프 미사용).
- 디스패치 경로는 동기적이다: `-[NSView keyDown:]`/`performKeyEquivalent:` → `gpui-pre-macos-0.3.8/src/window.rs:2637` `handle_key_event` → `run_callback`(같은 파일 2650 근처) → `gpui-pre-0.3.8/src/window.rs:1982` `on_input` → `Window::dispatch_event`(5455) → `dispatch_key_event`(5871) → `dispatch_action_on_node`(6015). 모두 AppKit의 `-[NSApplication sendEvent:]` 호출 스택 안에서 끝난다.
- 예외 두 가지:
  - `window.rs:5872-5874` 키 디스패치 직전, 뷰가 dirty면 `self.draw(cx)`로 프레임을 먼저 그린다. 핸들러 진입 시각에 그리기 시간이 섞이는 직접 경로다.
  - 다중 키 바인딩(예: `g g`)의 대기 입력은 `PENDING_INPUT_TIMEOUT`(1 s, `window.rs:1316`) 뒤 타이머 태스크에서 `replay_pending_input`(6290-6299)으로 재생된다. 이때는 원래 `sendEvent:` 밖이다.

### 2. 실험 (`spikes/sp1-key-timestamp`)

GPUI 창 하나에 `a` → Action(`KeyA`) 경로, `s` → `on_key_down`(raw `KeyDownEvent`) 경로를 걸고, 핸들러 안에서 다음을 동시에 읽어 비교한다.

- monitor: `NSEvent::addLocalMonitorForEventsMatchingMask_handler(KeyDown|KeyUp)` 블록이 thread-local에 남긴 마지막 이벤트(일련번호, timestamp, keyCode, 블록 호출 시각)
- current: `NSApplication::currentEvent()`(지금 `sendEvent:` 중인 이벤트)
- 핸들러 진입 시각: `NSProcessInfo.systemUptime`과 `mach_absolute_time()`(timebase로 초 환산)

입력 생성 두 가지:

- `synth`: 앱 안에서 `NSEvent keyEventWithType:...`로 down/up을 만들고 `NSApp postEvent:atStart:NO`로 큐에 넣는다. 타임스탬프에 게시 시각을 넣어 핸들러가 받은 값이 원래 이벤트의 값인지 대조한다. 60회.
- `osascript`: 별도 스레드에서 `osascript -e 'tell application "System Events" to keystroke "a"'`. WindowServer를 거쳐 메인 스레드 상태와 무관하게 도착하는 실제 키 이벤트. 30회. 이 기기에서는 접근성 권한이 이미 있어 동작했다.

부하 조건: `SP1_LOAD_MS=12`면 render마다 메인 스레드를 12 ms 붙잡고 매 프레임 다시 그린다(60 Hz에서 프레임의 72%).

실행:

~~~
cd spikes/sp1-key-timestamp
cargo build --release
SP1_LOAD_MS=0  ./target/release/sp1-key-timestamp synth
SP1_LOAD_MS=12 ./target/release/sp1-key-timestamp synth
SP1_LOAD_MS=0  ./target/release/sp1-key-timestamp osascript
SP1_LOAD_MS=12 ./target/release/sp1-key-timestamp osascript
./target/release/sp1-key-timestamp manual   # 사람이 a/s를 20초간 친다
~~~

측정 환경: MacBook Pro(Mac13,1, Apple M1 Max, 32 GB), macOS 27.0.1(26A434), 외장 LG 40" 5K2K(2560x1080 @ 60 Hz, 배율 2x), rustc 1.95.0, release 프로필.

## 결과

GPUI는 `NSEvent.timestamp`를 핸들러까지 전달하지 않는다(소스 분석으로 확정). 대신 핸들러 안에서 `NSApp.currentEvent`와 로컬 모니터 기록 모두로 원래 이벤트의 타임스탬프를 정확히 짝지을 수 있었다.

| 조건 | 핸들러 호출 | monitor ts == current ts | keyCode 일치 | 게시 ts 일치 | 모니터 일련번호가 핸들러 사이 정확히 +2 |
| --- | --- | --- | --- | --- | --- |
| synth, 부하 0 | 60 (Action 30, key_down 30) | 60/60 | 60/60 | 60/60 | 59/59 |
| synth, 부하 12 ms | 60 | 60/60 | 60/60 | 60/60 | 59/59 |
| osascript, 부하 0 | 30 (15/15) | 30/30 | 30/30 | 해당 없음 | 29/29 |
| osascript, 부하 12 ms | 30 (15/15) | 30/30 | 30/30 | 해당 없음 | 29/29 |

지연(ms). 「핸들러 − ts」는 수신 즉시 `mach_absolute_time`을 찍었을 때(REQ 9.3의 2번, S5-T2 분기 C) 섞이는 오차와 같다.

| 조건 | 핸들러 − event.timestamp p50 / p95 / max | 핸들러 − 모니터 블록 p50 / p95 / max |
| --- | --- | --- |
| synth, 부하 0 | 0.52 / 1.39 / 2.71 | 0.18 / 0.29 / 1.50 |
| synth, 부하 12 ms | 0.10 / 12.10 / 17.79 | 0.03 / 0.06 / 17.49 |
| osascript, 부하 0 | 0.95 / 1.07 / 10.30 | 0.16 / 0.18 / 8.78 |
| osascript, 부하 12 ms | 8.35 / 14.65 / 31.24 | 0.14 / 0.15 / 18.90 |

- 메인 스레드가 바쁘면 수신 즉시 시각은 최대 31 ms까지 밀린다. 이벤트 타임스탬프는 영향을 받지 않는다(게시 ts와 60/60 일치).
- 「핸들러 − 모니터 블록」의 최대값(17~19 ms)은 모니터가 이벤트를 본 뒤 GPUI가 핸들러를 부르기 전에 프레임을 그린 경우다(`dispatch_key_event`의 dirty 시 `draw`). 그래서 수신 시각을 핸들러에서 찍는 방식(분기 C)은 모니터를 써도 프레임 지연을 피하지 못한다.
- `mach_absolute_time` 환산 초 − `systemUptime`: 모든 표본에서 0.000 ms. `NSEvent.timestamp`는 mach 클록 기준 초이므로 `ts * 1e9`를 mach timebase로 바꾸면 CoreMIDI `MIDITimeStamp`와 같은 클록에 놓인다(REQ 9.3의 3번 전제 확인).
- 해상도: `NSEvent.timestamp`는 내부적으로 정수 ns로 보관된다(f64 왕복 오차 < 1 µs). NF-5(≤ 1 ms) 분해능 조건을 충족한다.
- `manual` 모드에서 osascript 5회 시험: 5/5 짝지어짐.

### 확인 못 함 / 주의

- 실제 하드웨어 키보드 입력은 측정하지 못했다(osascript 이벤트의 timestamp는 System Events가 CGEvent를 만든 시각이다). 하드웨어 키에서 `NSEvent.timestamp`가 IOHID 시각을 담는지는 사람이 `manual` 모드로 확인해야 한다.
- 다중 키 바인딩 재생(1 s 타임아웃) 경로에서는 `currentEvent`가 원래 이벤트가 아니다. Sequence 모드의 타이밍 키에는 다중 키 바인딩을 쓰지 않거나, 핸들러에서 모니터 기록의 keyCode·일련번호로 짝이 맞는지 검사해야 한다.
- 한국어 등 IME 입력 소스가 켜져 있고 포커스된 요소가 텍스트 입력을 받으면 키가 IME(`inputContext handleEvent:`)를 먼저 거친다(`gpui-pre-macos window.rs` `handle_key_event`). 이 경로도 `sendEvent:` 안이라 `currentEvent`는 유효할 것으로 보이나 실험하지 않았다.

## 계획에 미치는 영향

S5-T2 「결정 분기」 중 (B)를 택한다.

- (A) GPUI가 넘기는 타임스탬프: 불가. 고정 스냅샷의 `KeyDownEvent`·`Keystroke`·Action에 타임스탬프가 없고, 스냅샷 고정 원칙상 GPUI macOS 계층을 패치하지 않는다.
- (B) `addLocalMonitorForEvents` + 핸들러 짝짓기: 동작 확인. 핸들러는 `sendEvent:` 안에서 동기적으로 불리므로 짝짓기 키는 「같은 메인 스레드, 마지막 KeyDown, keyCode 일치」로 충분했다(Action·raw key_down 두 경로 모두 100%). 구현 시 `NSApp.currentEvent`를 1차 값으로, 모니터 기록(keyCode·일련번호)을 교차 검증으로 쓰면 다중 키 재생 같은 예외 경로를 걸러 낼 수 있다.
- (C) 수신 즉시 `mach_absolute_time`: 부하 시 p50 8 ms, 최대 31 ms 오차가 섞여 NF-5·M3 게이트(p90 ≤ 10 ms)를 위협한다. (B)가 실패할 때의 대비책으로만 둔다.

S5-T2 리뷰 초점(「메인 스레드 프레임 지연이 타임스탬프에 섞이는 경로」)에 위 「확인 못 함」 두 항목(다중 키 재생, IME 경로)을 추가할 것을 권한다.

사람이 확인할 항목: 하드웨어 키보드로 `manual` 모드를 실행해 짝짓기 100%와 `핸들러 − event.timestamp`가 osascript와 비슷한 크기인지 확인(가능하면 `SP1_LOAD_MS=12`로도).
