# 적대적 리뷰: S1-T3 MIDI 1.0 인코딩과 다운스케일 (리뷰 1회차)

- 리뷰어 세션: 구현자·계획자와 다른 새 서브 에이전트 세션. 구현 맥락을 물려받지 않았다.
- 대상 커밋: `09a704a` (브랜치 `s1/t3-midi1-encoding`, `git diff main...HEAD`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

단위 테스트가 `src/encode.rs` 안에 있어 코드를 읽을 때 함께 보였다. 아래 목록은 `tests/*.rs`를 열기 전에 적었다.

1. 다운스케일이 상위 비트를 남기는가(벨로시티 `>>9`, CC `>>25`, 벤드 `>>18`). 하위 비트 절삭(`& 0x7F`)이 섞인 곳이 없는가. 모든 `u16`에서 core `Velocity::to_midi1`과 같은가.
2. Note On은 어떤 16비트 벨로시티에서도 0으로 나가지 않는가. Note Off의 릴리스 벨로시티는 0을 그대로 두는가.
3. 피치 벤드 바이트 순서(`E0 lsb msb`), SPP(`F2 lsb msb`), 뱅크 셀렉트(CC0 MSB, CC32 LSB)가 14비트 전 범위에서 손실 없이 되돌아오는가.
4. 채널 1–16이 상태 바이트 하위 니블 0–15로 가는가(모든 메시지 종류).
5. UMP 워드가 타입 0x1·0x2만 쓰고, 그룹·상태·데이터 배치가 UMP 명세와 맞는가. 2바이트 메시지(Program Change)와 1바이트 리얼타임의 빈 데이터가 0인가.
6. `ump(group)`의 group 범위 밖 입력 처리. 리뷰 초점 「상위 비트 절삭」에 해당하는가.
7. 컨트롤러 120–127, SPP 16384 이상이 거부되는가.
8. 패닉 묶음의 내용과 순서가 OUT-6과 맞는가.
9. 할당 0 테스트가 실제로 할당을 잡는가(일부러 할당을 넣은 사본으로).
10. CoreMIDI를 거친 뒤 모든 메시지 종류가 같은 바이트로 도착하는가(특히 타입 0x1의 SPP, 2바이트 Program Change).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 상위 비트 다운스케일 | 있음(표 8·7·6행) | 전수 비교와 core `to_midi1` 일치는 없었다. 리뷰어 테스트로 보탬 |
| 2 Note On 0 → 1 | 있음(0, 1, 0x1FF) | 전 `u16`과 단조성은 없었다. 보탬 |
| 3 14비트 왕복 | 일부(0x1234, 끝값) | 전 범위 왕복은 없었다. 보탬 |
| 4 채널 니블 | 일부(1, 2, 3, 5, 10, 16) | 1–16 × 모든 종류는 없었다. 보탬 |
| 5 UMP 배치 | 있음(proptest) | 고정 기댓값 비교는 Clock·SPP만. Program Change·Start·Stop 보탬 |
| 6 group 범위 밖 | 없음 | group은 proptest에서 0–15만. 깨짐(3절 #1) |
| 7 범위 밖 입력 | 있음 | |
| 8 패닉 | 있음(채널 5) | 전 채널은 리뷰어 테스트로 보탬 |
| 9 할당 0 | 있음 | 변이 시험으로 확인(3절 #9) |
| 10 CoreMIDI 왕복 | 일부(Note On, 패닉, Clock) | SPP·PC·벤드·뱅크·Start/Stop/Continue 보탬 |

## 3. 깨뜨리기 시도

모두 실제로 실행했다. 테스트는 `crates/keyscore-midi/tests/review_s1_t3.rs`.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `ump(16..=255)`가 0–15의 다른 그룹 워드와 같은지 | 깨짐. `ump(16) == ump(0)`, `ump(17) == ump(1)`. `group & 0x0F`가 범위 밖 그룹을 조용히 다른 그룹으로 접는다 | `ump_group_out_of_range_does_not_alias_valid_group`(실패) |
| 2 | 모든 `u16` 벨로시티에서 `>>9`·core `to_midi1`과 비교 | 못 깨뜨림 | `velocity_to_7bit_every_u16_matches_top_bits_and_core_to_midi1` |
| 3 | 모든 `u16`에서 Note On 데이터가 1–127이고 단조 증가인지 | 못 깨뜨림 | `note_on_every_u16_velocity_never_sends_zero_and_is_monotonic` |
| 4 | 모든 `u16` 릴리스 벨로시티로 Note Off(채널 16) | 못 깨뜨림. 0x8F, 0은 0 그대로 | `note_off_every_u16_release_velocity_keeps_zero_and_uses_status_8n` |
| 5 | CC 하위 비트만 켠 값(0x7F, 0x01FF_FFFF 등)과 32비트 표본 | 못 깨뜨림. 하위 비트가 새지 않는다 | `cc_to_7bit_low_bits_set_do_not_leak_into_result` |
| 6 | 14비트 전 값 × 하위 18비트 0/전부 1로 벤드 | 못 깨뜨림. `E0 lsb msb` | `pitch_bend_every_14bit_value_encodes_lsb_then_msb_ignoring_low_18_bits` |
| 7 | 뱅크 0–16383 MSB/LSB 재조합, CC0·CC32를 `control_change`로 보낸 결과와 비교 | 못 깨뜨림 | `bank_select_every_bank_recombines_to_same_value`, `control_change_bank_controllers_match_bank_select_encoding` |
| 8 | SPP 0–16383 왕복과 UMP 워드, 16384–65535 거부 | 못 깨뜨림 | `song_position_every_14bit_value_round_trips_and_16384_is_error` |
| 9 | `/tmp` 사본의 `pitch_bend`에 `black_box(Box::new(..))`를 넣고 `encode_alloc` 실행 | 못 깨뜨림. 사본에서 `left: 1, right: 0`으로 실패했다. 할당 0 테스트는 할당을 잡는다 | 없음(사본은 지웠다) |
| 10 | 채널 1–16 × 모든 채널 메시지의 상태 바이트·UMP 상위 16비트 | 못 깨뜨림 | `every_channel_maps_to_status_nibble_zero_to_fifteen` |
| 11 | Program Change(2바이트)·Start·Continue·Stop의 UMP 고정값 | 못 깨뜨림. `0x2AC2_5500`, `0x1FFA_0000` 등 | `ump_layout_matches_spec_for_two_byte_and_realtime_messages` |
| 12 | 전 채널 패닉 묶음 내용·순서 | 못 깨뜨림 | `panic_messages_every_channel_sends_cc120_cc123_cc64_zero_in_order` |
| 13 | 가상 포트 → `coremidi` 수신기로 Note On/Off, PC, CC119, 벤드 가운데·최대, SPP 0·16383, Start/Continue/Stop/Clock, 뱅크 최대 | 못 깨뜨림. 모두 같은 바이트로 도착 | `every_message_kind_reaches_receiver_with_same_bytes_through_coremidi` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 데이터 바이트 0–127, 상태 바이트 0x80 이상 | 없음 | 입력 타입(`Channel`, `Pitch`, `Program`, `Bank`)이 범위를 보장하고, 다운스케일은 시프트라 범위 안이다 |
| UMP 타입은 0x1·0x2뿐 | 없음 | `Midi1Message`는 비공개 필드이고 생성자는 모두 비공개 헬퍼를 거친다 |
| 쓰지 않는 데이터 바이트는 0 | 없음 | `short`/`realtime`/`program_change`가 0을 채운다(리뷰어에게 3) |
| UMP 그룹은 호출자가 준 그룹 | 깨짐 | `Midi1Message::ump(16)` = `ump(0)`(`encode.rs:70`) |
| Note On 벨로시티 ≥ 1 | 없음 | 전 `u16` 확인 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| M2-1 | 충족 | `crates/keyscore-midi/src/encode.rs:76-90`, `:94-102`. 벨로시티 `>>9`, CC `>>25`, 벤드 `>>18`이고 Note On 결과 0을 1로 올린다. 인터넷 없이 M2-104-UM 원문은 확인하지 못했다. 알려진 MIDI 2.0 → 1.0 기본 변환 규칙(상위 비트를 남기는 오른쪽 시프트, Note On 벨로시티 0은 1로 바꿈, Note Off는 그대로)으로 판단했다 |
| SEQ-4 | 충족 | `encode.rs:111-150`. PC, CC(32비트 → 7비트), 벤드(32비트 → 14비트) |
| CTL-4 | 충족(송신 14비트 부분) | `encode.rs:141-150`. 가운데 `0x8000_0000` → `E0 00 40`. 레인 모델의 부호 있는 값(가운데 0, ±8192)을 `u32`로 옮기는 변환은 아직 없다(7절 #3) |
| OUT-5 | 충족(인코딩 부분) | `encode.rs:152-185`. F8, FA, FB, FC, F2. 켜고 끄기·24 ppqn 송신은 S1-T4 |
| OUT-6 | 충족(채널 단위 묶음) | `encode.rs:187-191`. CC120, CC123, CC64=0. 전 포트·전 채널로 퍼뜨리는 일은 S1-T4 |
| 9.2(송신 경로 할당 금지) | 충족 | `tests/encode_alloc.rs`. 변이 시험으로 할당을 잡음을 확인 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 인코더는 고정 크기 값만 만든다. 변이 시험으로 할당 검사가 동작함을 확인했다.
- 제품 코드의 `unwrap`/`expect`: 없음. `to_7bit`의 `unwrap_or(0x7F)`(`encode.rs:26`)와 `bend_to_14bit`의 `unwrap_or(0x3FFF)`(`encode.rs:89`)는 시프트·마스크 뒤라 닿지 않는 분기다. `as` 절삭도 diff에 없다(core `Velocity::to_midi1`의 `as u8`은 `>>9` 뒤라 손실이 없고 이 Task 범위 밖이다).
- 에러가 삼켜지는 곳: `Midi1Message::ump`의 `group & 0x0F`(`encode.rs:70`). 범위 밖 그룹이 오류 없이 다른 그룹이 된다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | `ump(group)`이 `group & 0x0F`로 상위 비트를 버려 그룹 16을 0, 17을 1로 보낸다. 이 Task의 리뷰 초점(「상위 비트 절삭을 쓴 곳이 없는가」)에 그대로 걸린다. 호출자의 범위 밖 값이 조용히 엉뚱한 그룹으로 간다. 그룹 newtype(0–15를 타입이 보장)을 받거나 `Result`로 거부한다. 문서 주석 「하위 4비트만 쓴다」도 함께 고친다. 리뷰어 테스트 `ump_group_out_of_range_does_not_alias_valid_group`가 통과해야 한다. 테스트가 고정한 것은 「다른 그룹과 같은 워드가 되지 않음」뿐이므로, 타입으로 막아 `u8`을 받지 않게 바꾸면 이 테스트를 새 시그니처에 맞게 고쳐도 된다(지우지 말고 같은 의도로) | `crates/keyscore-midi/src/encode.rs:55-71` |
| 2 | 제안 | 단계 파일 게이트 「벨로시티 0은 Note Off로」를 계획에서 다르게 해석했다. 해석 자체는 REQ M2-1과 알려진 변환 규칙에 맞는다고 본다. 다만 이 해석은 「모델에서 벨로시티 0인 노트도 벨로시티 1로 소리 난다」는 사용자에게 보이는 결정이고, 기록이 Task 계획 파일에만 있다. 단계 파일 문구를 고치거나 `docs/decisions/`에 한 줄 남겨 S1-T6(SMF 읽기에서 `9n kk 00`을 Note On 벨로시티 0 노트로 만들지 않기)과 이어지게 한다 | `docs/reports/s1-t3/plan.md`(단계 파일과의 차이), `plan/stages/11-S1-midi-engine.md:27` |
| 3 | 제안 | CTL-4는 벤드 레인을 「가운데 0, 범위 ±8192」로 적는데 인코더는 가운데 `0x8000_0000`인 `u32`만 받는다. 부호 있는 레인 값 → `u32` 변환과 +8192(14비트로는 +8191이 최대)의 처리를 레인 모델 Task에서 정해야 한다. 이 Task를 막지는 않는다 | `crates/keyscore-midi/src/encode.rs:86-90`, `docs/REQ.md:747` |

리뷰어에게 1–3에 대한 답:

1. 할당 0 테스트의 범위: 인코딩은 호출 스레드에서만 일어나므로 스레드별 카운터로 충분하다. 기본 `alloc_zeroed`는 `self.alloc`을 부르므로 함께 세어진다. 변이 시험(3절 #9)으로 실제로 잡음을 확인했다.
2. `control_change`의 CC0·CC32: 막을 이유가 없다고 본다. 같은 바이트가 나옴을 확인했다(`control_change_bank_controllers_match_bank_select_encoding`).
3. `ump`의 「쓰지 않는 데이터 바이트 0」 불변식: 공개 생성 경로가 모두 비공개 헬퍼를 거치고 필드가 비공개라 깨뜨리지 못했다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 #1(수정)을 고치고 리뷰어 테스트가 모두 통과하면 재리뷰한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(단위 테스트는 같은 파일이라 코드와 함께 보였다. 1절 머리말 참고)
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(`proptest` dev 의존, 워크스페이스에 이미 있음, MIT/Apache-2.0)
- [x] 스타일 지적을 적지 않았다
