# 적대적 리뷰: S1-T3 MIDI 1.0 인코딩과 다운스케일 (리뷰 2회차)

- 리뷰어 세션: 구현자·계획자·1회차 리뷰어와 다른 새 서브 에이전트 세션. 구현 맥락을 물려받지 않았다.
- 대상 커밋: `2fadc8b`(리뷰 1 지적 1 수정, `Group` newtype), `fbf7811`(리뷰 1 제안 2·3 기록). 범위는 이 두 커밋과 `f5a98ff`의 대응 기록뿐이다. 1회차에서 본 인코더 본체는 다시 보지 않았다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`git diff 2fadc8b~1 2fadc8b`는 제품 코드와 테스트 변경이 한 diff에 섞여 있어, 제품 코드(`src/encode.rs`, `src/lib.rs`) 부분을 먼저 읽고 아래를 적은 뒤 테스트 부분을 읽었다.

1. `Group::try_from`이 0–15를 모두 받고 16–255를 모두 거부하는가. `get`이 넣은 값을 그대로 돌려주는가. `ZERO`가 `try_from(0)`과 같은가.
2. 0–15 밖의 `Group`을 만드는 다른 길이 있는가: 공개 필드, `Default`, `From<u8>`, `FromStr`, serde, crate 안의 다른 `Group(..)` 생성, unsafe.
3. `ump`에서 마스킹을 지운 뒤 그룹이 비트 24–27만 바꾸는가. 그룹 값이 메시지 타입 니블(28–31)로 새지 않는가(16이 들어오면 타입 0x2가 0x3이 된다).
4. 16개 그룹이 서로 다른 워드를 만드는가. 그룹 0 워드의 상태·데이터가 `bytes()`와 맞는가.
5. 할당 0 테스트가 `Group::try_from(..).unwrap_or(Group::ZERO)`를 센 구간 안에서 부르는데, 여전히 `ump`의 할당을 잡는가. `unwrap_or`가 무엇을 가리는가.
6. 리뷰어 테스트 변경이 호출 방식만 바꿨는가. 실패 테스트의 의도(16–255가 유효 그룹으로 접히지 않음)가 남았는가.
7. `fbf7811`의 기록이 REQ M2-1과 맞고, 이월 대상 Task가 그 결정이 처음 필요한 Task인가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 `try_from` 경계 | 일부 | 리뷰어 테스트가 16–255 거부만 본다. 0–15 수용과 `get` 왕복은 없었다. 보탬 |
| 2 다른 생성 경로 | 없음(코드 검토) | `Group(u8)` 필드 비공개, derive는 `Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash`뿐, crate는 `#![deny(unsafe_code)]`. 생성은 `encode.rs:15`, `:31` 두 곳뿐 |
| 3 그룹 비트 위치 | 있음(proptest, 0–15) | 모든 메시지 종류 × 16그룹에서 `ump(g)`가 `ump(ZERO)`에 `g << 24`를 OR한 값인지는 없었다. 보탬 |
| 4 그룹 단사·바이트 일치 | 일부 | 보탬 |
| 5 할당 0 테스트 | 있음 | 변이 시험으로 확인(3절 #5, #6) |
| 6 리뷰어 테스트 변경 | 해당 없음 | diff로 확인. 3절 #1 |
| 7 계획 기록 | 해당 없음 | 7절 #1, #2 |

## 3. 깨뜨리기 시도

모두 실제로 실행했다. 테스트는 `crates/keyscore-midi/tests/review_s1_t3_r2.rs`. 변이 시험은 `/tmp` 사본에서 했고 사본은 지웠다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `review_s1_t3.rs` 변경 대조. `ump(0)` → `ump(Group::ZERO)`, `ump(0xA)` → `ump(Group::try_from(0xA).unwrap())` 등 호출만 바뀌었고 기대 워드(`0x2AC2_5500`, `0x1FFA_0000`, `0x10FB_0000`, `0x11FC_0000`, SPP·상태 니블 식)는 그대로다. 실패 테스트는 같은 이름으로 「16–255에서 `try_from`이 `InvalidMessage`」를 확인한다. `u8`을 받는 `ump`가 없어졌으니 「접힘」을 일으킬 입력 자체가 없고, 1회차 리뷰가 허락한 바꿈이다. 약해지지 않았다 | 못 깨뜨림 | 없음 |
| 2 | 0–255 전부 `try_from`: 0–15만 `Ok`이고 `get` 왕복, 16–255는 `InvalidMessage` | 못 깨뜨림 | `group_try_from_every_u8_accepts_only_zero_to_fifteen_and_get_round_trips` |
| 3 | `ZERO == try_from(0)`, `ZERO`가 가장 작음(`Ord`가 수 순서) | 못 깨뜨림 | `group_zero_constant_equals_try_from_zero_and_is_smallest` |
| 4 | 메시지 15종(Note On 0·최대, Note Off, PC, CC119, 벤드 0·최대, SPP 0·16383, F8/FA/FB/FC, 뱅크 2, 패닉 3) × 그룹 16: 그룹 0 워드가 바이트와 맞고, 다른 그룹은 비트 24–27만 다르며, 16개 워드가 모두 다름 | 못 깨뜨림 | `ump_every_group_sets_only_group_nibble_and_rest_matches_bytes`, `ump_sixteen_groups_give_sixteen_distinct_words` |
| 5 | 변이: `ump` 안에 `black_box(Box::new(0u8))` | 못 깨뜨림. `encode_alloc`이 실패했다. 할당 0 테스트는 여전히 의미 있다 | 없음 |
| 6 | 변이: `try_from`을 `value < 1`로(1–15 거부) | 부분. `encode_alloc`은 통과했다(`unwrap_or(Group::ZERO)`가 그룹 1 실패를 그룹 0으로 가림). `review_s1_t3`의 `ump_layout..`과 r2 테스트 4개는 실패해 잡았다. 할당 검사의 목적에는 영향이 없다(7절 #4) | r2 테스트 |
| 7 | 변이: `try_from`을 `value < 17`로 | 못 깨뜨림. `review_s1_t3`의 `ump_group_out_of_range..`와 r2 경계 테스트가 실패했다 | r2 테스트 |
| 8 | 변이: `ump`가 그룹을 무시(`0 << 24`) | 못 깨뜨림. r2 테스트 2개가 실패했다 | r2 테스트 |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| `Group`은 0–15 | 없음 | 필드 비공개, 생성은 `ZERO`와 `TryFrom<u8>`뿐. `Default`·`From`·serde 없음. crate `#![deny(unsafe_code)]`(`lib.rs:5`) |
| UMP 그룹 = 호출자가 준 그룹 | 없음 | `ump`가 `group.get() << 24`만 쓴다(`encode.rs:97`). 3절 #4 |
| UMP 타입 니블은 0x1·0x2 | 없음 | 그룹이 4비트를 넘지 않아 타입 니블로 새지 않는다 |
| `MidiPort::send`로 가는 워드의 그룹 | 있음(이 Task 범위 밖) | `send`는 날 `u32`를 받아 `Group`을 거치지 않은 워드도 보낸다. S1-T1 API다(7절 #5) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| M2-1 | 충족 | 이번 수정은 다운스케일을 건드리지 않았다. 리뷰 초점 「상위 비트 절삭」에 걸렸던 `group & 0x0F`가 사라졌다(`crates/keyscore-midi/src/encode.rs:84-98`). `fbf7811`의 해석(Note On 7비트 결과 0 → 1, 끄기는 0x8n)은 M2-1이 따르라는 MIDI 2.0 → 1.0 변환 규칙과 맞다(`plan/stages/11-S1-midi-engine.md:28`) |
| CTL-4 | 해당 없음(이월 기록만) | 이월 위치는 7절 #1 |
| 9.2(송신 경로 할당 금지) | 충족 | `crates/keyscore-midi/tests/encode_alloc.rs:54`. 변이 시험 3절 #5 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. `Group`은 `Copy` 1바이트이고 `try_from`의 오류는 데이터 없는 변형이다.
- 제품 코드의 `unwrap`/`expect`: 없음(테스트 모듈의 `unwrap`만).
- 에러가 삼켜지는 곳: 제품 코드에는 없다. 테스트 `encode_alloc.rs:54`의 `unwrap_or(Group::ZERO)`가 그룹 생성 실패를 가린다(7절 #4).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 벤드 레인 값(가운데 0, ±8192) → 모델 32비트 값 변환과 +8192 처리를 S6-T3로 이월했지만, 그 변환이 처음 필요한 곳은 S6-T3보다 앞이다. 벤드 레인 자체(「벤드 레인(중앙 0, ±8192)」, CTL-4)는 S6-T1 범위이고, S6-T2는 「그리는 동안 즉시 송신(CTL-5)」으로 벤드 값을 이미 인코더에 넘긴다. S6-T3에만 적으면 S6-T1·T2가 이 결정을 모른 채 임의로 정하게 된다. 이월을 S6-T1(값 영역 결정)에 두고 S6-T2·T3가 그것을 따른다고 적는다 | `plan/stages/16-S6-expression.md:12`, `:17`, `:25` |
| 2 | 제안 | 「S1-T6은 가져올 때 벨로시티 0인 Note On을 Note Off로 읽는다」는 S1-T3 절에만 있다. S1-T6 절(범위·테스트 게이트)에는 없어 S1-T6 계획자가 놓칠 수 있다. 놓치면 `9n kk 00`이 끝나지 않는 노트가 되거나 벨로시티 1로 소리 난다. S1-T6 절에 이월 한 줄과 테스트 게이트(벨로시티 0 Note On이 노트 끝으로 읽힘)를 더한다 | `plan/stages/11-S1-midi-engine.md:28`, `:43-47` |
| 3 | 제안 | `Group::try_from`의 오류 `InvalidMessage`는 문서와 표시 문자열이 「UMP 메시지 길이가 메시지 타입과 맞지 않는다」라 그룹 범위 오류를 잘못 설명한다. `control_change`·`song_position`도 같은 변형을 쓰므로 이번 수정 전부터 있던 패턴이다. 범위 오류용 변형을 따로 두거나 문서를 넓힌다 | `crates/keyscore-midi/src/error.rs:18-20`, `crates/keyscore-midi/src/encode.rs:23-36` |
| 4 | 제안 | 할당 0 테스트의 `Group::try_from(1).unwrap_or(Group::ZERO)`는 그룹 1 생성이 실패해도 조용히 그룹 0으로 지나간다(3절 #6). 할당 검사의 목적에는 지장이 없고, 그룹 정확성은 다른 테스트가 잡는다. 그룹을 센 구간 밖에서 `unwrap()`으로 한 번 만들어 쓰면 의도가 더 분명하다 | `crates/keyscore-midi/tests/encode_alloc.rs:54` |
| 5 | 제안(범위 밖) | `MidiPort::send`는 날 `u32` 워드를 받아 `Group`을 거치지 않은 워드도 보낼 수 있다. 스케줄러(S1-T4)가 `Midi1Message::ump`로 만든 워드만 넘기도록 하면 `Group` 보장이 송신 경로 끝까지 이어진다 | `crates/keyscore-midi/src/port.rs:17` 부근 |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 1 지적 1(`Group`)은 해결됐다. 깨뜨리지 못했고, 리뷰어 테스트는 약해지지 않았다. 남은 것은 `fbf7811`의 이월 위치(지적 #1, 수정)다. 단계 파일 한두 줄을 고치면 되고, 재리뷰는 그 diff만 본다. 제안 #2–#5는 판정을 막지 않는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다(제품 코드 부분을 먼저 읽었다. 1절 머리말 참고)
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`2fadc8b`는 `keyscore-midi`의 encode·lib·테스트, `fbf7811`은 단계 파일 두 개)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(실패 테스트는 같은 이름·같은 의도로 바뀜)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
