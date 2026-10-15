# Task 보고: S1-T3 MIDI 1.0 인코딩과 다운스케일

- PR: 브랜치 `s1/t3-midi1-encoding`
- 계획 파일: `docs/reports/s1-t3/plan.md`
- 작성: 구현자 에이전트(경량 모델)의 보고를 계획자가 옮겼다. 게이트는 계획자가 다시 돌려 확인했다.
- 계획 대비 달라진 점:
  - `Midi1Message::ump(group)`은 group을 `& 0x0F`로 마스킹한다(범위 밖 입력에도 패닉하지 않게).
  - `keyscore-midi`에 dev 의존 `proptest`를 더했다.
  - 통합 테스트에 Timing Clock과 패닉 묶음 수신 검사를 더했다.

## 구현 요약

| 항목 | 내용 |
| --- | --- |
| `Midi1Message` | `bytes: [u8; 3]`와 `len`. `bytes()`는 슬라이스를 돌려준다. `ump(group)`은 0xF0 미만이면 타입 0x2(채널 보이스), 0xF0 이상이면 타입 0x1(시스템)의 1워드를 만든다 |
| 다운스케일 | `velocity_to_7bit` = `v >> 9`, `cc_to_7bit` = `v >> 25`, `bend_to_14bit` = `v >> 18`. Note On은 결과에 `.max(1)`을 적용하고, Note Off(릴리스)는 그대로 둔다 |
| 채널 메시지 | `note_on`, `note_off`, `program_change`, `bank_select`(CC0 MSB, CC32 LSB), `control_change`(120 이상은 `InvalidMessage`), `pitch_bend`(`E0 lsb msb`) |
| 시스템 메시지 | `timing_clock`, `start`, `continue_`, `stop`, `song_position`(16384 이상은 `InvalidMessage`) |
| 패닉 | `panic_messages`: CC120, CC123, CC64=0 순서 |

제품 코드에는 `unwrap`/`expect`와 `as u8` 절삭이 없다. unsafe는 테스트 파일 `tests/encode_alloc.rs`의 할당 세기용 `GlobalAlloc` 하나뿐이고 SAFETY 주석이 있다.

## 테스트 결과

~~~
cargo test --workspace --no-fail-fast: 577 passed, 0 failed
~~~

| 계획 # | 실제 이름 | 위치 | 상태 |
| --- | --- | --- | --- |
| 1 | `velocity_downscale_matches_midi2_translation_table` | `src/encode.rs` | 통과 |
| 2 | `cc_and_bend_downscale_match_midi2_translation_table` | `src/encode.rs` | 통과 |
| 3 | `upscale_then_downscale_round_trips_all_7bit_values` | `src/encode.rs`(proptest) | 통과 |
| 4 | `note_and_controller_messages_encode_status_and_data` | `src/encode.rs` | 통과 |
| 5 | `system_realtime_and_spp_encode` | `src/encode.rs` | 통과 |
| 6 | `ump_uses_only_types_1_and_2_with_matching_bytes` | `src/encode.rs`(proptest) | 통과 |
| 7 | `panic_messages_cover_all_sound_off_all_notes_off_sustain` | `src/encode.rs` | 통과 |
| 8 | `invalid_controller_and_spp_are_errors` | `src/encode.rs` | 통과 |
| 9 | `encoded_note_on_reaches_virtual_port_receiver` | `tests/encode.rs` | 통과 |
| 10 | `encoding_does_not_allocate` | `tests/encode_alloc.rs` | 통과 |

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [x] test (577 passed)
- [x] deny
- [x] 단계별 추가 게이트
  - `check-deps.sh` ok
  - `check-deps.test.sh` ok
  - `bench-check.test.sh` ok

## 알려진 한계

- MIDI 2.0 UMP 송신(M2-2)은 이후 버전이다.
- 단계 파일의 「벨로시티 0은 Note Off로」는 REQ M2-1(MIDI 2.0 변환 규칙)을 따라 해석했다. 계획의 「단계 파일과의 차이」를 보라.

## 리뷰어에게

구현자가 스스로 불안하다고 한 곳:

1. 테스트 10(할당 0)의 범위
   - 스레드별 카운터라, 인코딩을 호출한 스레드의 할당만 센다.
   - `alloc`과 `realloc`만 센다. 기본 `alloc_zeroed`는 `alloc`을 거친다고 본다.
2. `control_change`의 범위
   - 120 이상은 거부하지만 CC0·CC32(뱅크 셀렉트)는 통과한다.
   - 뱅크는 `bank_select`로만 보내야 하는지 판단해 달라.
3. `Midi1Message::ump`의 내부 불변식
   - 쓰지 않는 데이터 바이트가 0이라는 내부 불변식에 기댄다. 생성자는 모두 비공개 헬퍼를 거친다.
