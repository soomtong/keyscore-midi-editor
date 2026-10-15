# Task 계획: S1-T3 MIDI 1.0 인코딩과 다운스케일

- 브랜치: `s1/t3-midi1-encoding`
- 관련 REQ: M2-1, SEQ-4, CTL-4, OUT-5, OUT-6, 9.2(송신 경로 할당 금지)
- 선행 Task: S1-T1(`MidiPort::send`가 받는 UMP), S0-T2(`Velocity`, `Channel`, `Pitch`, `Program`, `Bank`)
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

- 모델 값(16비트 벨로시티, 32비트 CC·벤드, 채널 1–16)을 MIDI 1.0 메시지로 바꾸는 인코더가 `keyscore-midi`에 있다.
- 결과는 고정 크기 값 `Midi1Message`다(힙 할당 없음). 두 가지로 꺼낼 수 있다.
  - UMP 1워드: `MidiPort::send`용. 메시지 타입 0x2(MIDI 1.0 채널 보이스)와 0x1(시스템)만 쓴다.
  - MIDI 1.0 바이트(1–3바이트): SMF용(S1-T6).
- 다운스케일은 MIDI 2.0 명세의 변환 규칙을 따른다(M2-1. 상위 비트 절삭이 아니다).
  - 벨로시티 16→7비트: `v >> 9`. Note On에서 결과가 0이면 1로 한다(MIDI 1.0에서 벨로시티 0인 Note On은 Note Off이기 때문이다. 명세의 변환 규칙).
  - CC 32→7비트: `v >> 25`
  - 피치 벤드 32→14비트: `v >> 18`. 가운데는 0x8000_0000 → 0x2000이다.
- 채널 보이스: Note On, Note Off(릴리스 벨로시티 포함), Program Change, Bank Select(CC0 MSB + CC32 LSB 두 메시지), CC, Pitch Bend
- 시스템: Timing Clock(0xF8), Start(0xFA), Continue(0xFB), Stop(0xFC), Song Position Pointer(0xF2, 14비트 16분음표 단위)
- 패닉 묶음: 채널마다 All Sound Off(CC120), All Notes Off(CC123), Sustain 0(CC64)(OUT-6)
- Running status는 쓰지 않는다. 메시지마다 상태 바이트를 쓴다.

## 단계 파일과의 차이

단계 파일 게이트의 「벨로시티 0은 Note Off로」는 MIDI 1.0의 해석(벨로시티 0인 Note On = Note Off)을 가리킨다. 모델 쪽 「16비트 벨로시티가 7비트 0으로 내려가는 Note On」을 Note Off로 보내라는 뜻으로 읽으면, REQ M2-1이 따르라는 MIDI 2.0 변환 규칙(결과 0이면 1)과 어긋난다. 그래서 REQ를 따른다.

- 모델의 Note On은 언제나 벨로시티 1 이상으로 나간다.
- 모델 벨로시티 0으로 Note On을 만드는 것도 1이다. 노트를 끄려면 `note_off`를 쓴다.

테스트 게이트는 이 해석으로 둘 다 확인한다. 하나는 Note On이 0으로 나가지 않는다는 것, 다른 하나는 Note Off가 0x8n이라는 것이다.

## 공개 인터페이스 초안

~~~rust
pub struct Midi1Message { /* bytes: [u8; 3], len: u8 */ }
impl Midi1Message {
    pub fn bytes(&self) -> &[u8];
    pub fn ump(&self, group: u8) -> u32;   // group 0–15, 타입 0x2/0x1 한 워드
}
pub fn note_on(ch: Channel, pitch: Pitch, velocity: Velocity) -> Midi1Message;
pub fn note_off(ch: Channel, pitch: Pitch, release: Velocity) -> Midi1Message;
pub fn program_change(ch: Channel, program: Program) -> Midi1Message;
pub fn bank_select(ch: Channel, bank: Bank) -> [Midi1Message; 2];     // CC0, CC32
pub fn control_change(ch: Channel, controller: u8, value32: u32) -> Result<Midi1Message, MidiError>; // 0–119
pub fn pitch_bend(ch: Channel, value32: u32) -> Midi1Message;
pub fn timing_clock() / start() / continue_() / stop() -> Midi1Message;
pub fn song_position(sixteenths: u16) -> Result<Midi1Message, MidiError>; // < 16384
pub fn panic_messages(ch: Channel) -> [Midi1Message; 3];
pub fn velocity_to_7bit / cc_to_7bit / bend_to_14bit (pub, 테이블 테스트용)
~~~

컨트롤러 번호 120–127은 채널 모드 메시지라 `control_change`에서는 오류다(`InvalidMessage`). 패닉은 전용 함수로만 보낸다.

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `velocity_downscale_matches_midi2_translation_table` | 단위(표) | 0, 1, 0x1FF, 0x200, 0x8000, 0xFFFF 등 → 7비트, Note On의 0 → 1 |
| 2 | `cc_and_bend_downscale_match_midi2_translation_table` | 단위(표) | 32비트 끝값·가운데·경계 |
| 3 | `upscale_then_downscale_round_trips_all_7bit_values` | 속성 | core `Velocity::from_midi1` → 다운스케일 = 원래 값(1–127), 0은 Note On에서 1 |
| 4 | `note_and_controller_messages_encode_status_and_data` | 단위 | 상태 바이트·채널 0–15·데이터 7비트 |
| 5 | `system_realtime_and_spp_encode` | 단위 | F8/FA/FB/FC, F2 LSB·MSB |
| 6 | `ump_uses_only_types_1_and_2_with_matching_bytes` | 속성 | 모든 메시지의 UMP가 타입 0x1·0x2, 그룹, 바이트와 일치(S1-T1 이월) |
| 7 | `panic_messages_cover_all_sound_off_all_notes_off_sustain` | 단위 | 순서와 값 |
| 8 | `invalid_controller_and_spp_are_errors` | 단위 | 120–127, SPP ≥ 16384 |
| 9 | `encoded_note_on_reaches_virtual_port_receiver` | 통합 | 가상 포트로 보낸 결과를 `coremidi` 수신기가 같은 바이트로 받음 |
| 10 | `encoding_does_not_allocate` | 단위 | 할당 세는 전역 할당자로 인코딩 경로의 할당 0 |

## 불변식

- 모든 데이터 바이트는 0–127, 상태 바이트는 0x80 이상이다.
- 인코딩 함수는 할당·락·패닉이 없다. 입력 타입이 범위를 보장하고, 범위 밖 입력(컨트롤러 번호, SPP)은 `Result`로 거부한다.
- `#![deny(unsafe_code)]`인 crate 규칙 그대로 unsafe를 쓰지 않는다.

## 범위 밖

- MIDI 2.0 UMP 송신(M2-2): 이후 버전
- SysEx, MTC, Active Sensing: REQ에 없다
- 송신 타이밍과 스케줄러: S1-T4

## 리스크·결정 요청

- 결정 요청 없음. 위 「단계 파일과의 차이」는 REQ M2-1을 따른 해석이다.
