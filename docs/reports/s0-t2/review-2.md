# 적대적 리뷰: S0-T2 핵심 모델 타입 (리뷰 2회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트). 구현 컨텍스트를 물려받지 않았다. 1회차 리뷰어와도 다른 세션이다.
- 대상 커밋: `246215d` (`git diff main...HEAD`, 1회차 이후 변경은 `227af18..246215d`)
- 기준: `plan/stages/10-S0-foundation.md` S0-T2, `docs/reports/s0-t2/plan.md`, `docs/decisions/2026-10-12-clip-split-and-velocity.md`(Q1, Q2 개정, Q3), REQ 5.10 「클립 규칙」, 6.1 SEQ-3·SEQ-7, 6.5 M2-1·M2-3, 1회차 리뷰와 구현자 대응(`report.md` 「리뷰 대응 (review-1)」)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`src/*.rs`의 `#[cfg(test)]` 위쪽만 읽고 적었다.

1. `Velocity::from_midi1`이 MIDI 2.0 명세의 일반형 scaleUp(min-center-max, 비트 반복)과 0–127 전체에서 같은가. 손으로 고른 네 값(0, 64, 65, 100, 127)만으로는 반복 루프의 시프트 방향·마스크 실수를 놓친다.
2. `Velocity::to_midi1`이 명세의 scaleDown(오른쪽 시프트)과 u16 전체에서 같은가. 하위 비트 절삭으로 바꿔도 테스트가 잡는가. 16비트 값 1–0x1FF가 7비트 0(MIDI 1.0 Note On에서 Note Off 의미)이 되는 것을 어디서 다루는가.
3. `PitchOffset`: `SEMITONE = 2^25`, `MAX = 12 × 2^25 = 402,653,184`, 포함 경계(±MAX 허용, ±MAX±1 거부), `i32::MIN`·`i32::MAX` 거부. 음높이 + 오프셋이 MIDI 2.0 7.25 절대 피치(u32, 0 이상 128 미만)로 표현되지 않는 조합(127 + 12반음, 0 − 12반음)을 허용하는가.
4. `ClipId::allocate` 전역 카운터: 한 트랙 안에서 같은 식별자가 둘 생기는 공개 경로가 있는가(꺼낸 클립을 다시 넣기, 복제한 트랙 사이 옮기기, 분할 반복). 병렬 스레드에서 겹치는가.
5. `Track::clone`은 식별자를 그대로 복사한다. 복제 트랙을 같은 `Project`에 넣으면 트랙 0의 식별자가 트랙 1의 클립을 가리키는가(1회차 제안 4의 우회 경로).
6. `PartialEq`가 `ClipId`를 비교한다. 꺼낸 클립을 다시 넣은 트랙은 원본과 다르다고 나오는가(1회차 제안 5, S0-T4 「undo N번 = 초기 상태」). S0-T6 직렬화에서 식별자를 저장하지 않으면 왕복 동일이 깨지고, 저장해 복원하면 새 프로세스의 카운터(0부터)가 복원된 식별자와 같은 값을 다시 발급할 수 있는가.
7. 지적 1의 수정(`extent()` 검사)을 되돌리면 어느 테스트가 잡는가. 특히 「이후 모든 단계의 회귀 테스트」인 `model_invariants.rs`가 잡는가(틱 범위 `0..20_000`).
8. `move_clip`이 `remove` 뒤 `set_start(..)?`를 부른다(`track.rs:343-345`). 사전 검사와 `set_start` 검사가 어긋나면 클립이 사라지는가.
9. `set_clip_name`·`set_clip_color`가 식별자를 유지하고, 없는 식별자에 `UnknownClip`을 주는가.
10. 문서(README, 계획, 보고)가 개정된 결정 Q2·Q3과 맞는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 7→16 전체 값 | 부분 | `velocity_from_midi1_follows_midi2_min_center_max_scaling`은 6개 값. 왕복 테스트는 상위 7비트만 보므로 하위 9비트 오류를 못 본다. 이번에 전체 값 테스트를 더했다 |
| 2. 16→7 전체 값, 1–0x1FF → 0 | 부분 | 경계 3개 값만. 0 문제는 결정 Q2가 S1로 미뤘다 |
| 3. 오프셋 경계 | 있음 | `pitch_offset_beyond_one_octave_is_rejected`. `i32::MAX`는 없었다(이번에 더함). 음높이 + 오프셋 범위는 없음 |
| 4. 한 트랙 안 식별자 중복 | 부분 | `check_invariants`가 중복을 검사하지만 단일 트랙 수열만. 트랙 사이 이동·복제·병렬은 없음 |
| 5. 복제 트랙 식별자 공유 | 없음 | |
| 6. `PartialEq`와 식별자 | 없음 | |
| 7. 지적 1 회귀 | 부분 | `review_s0_t2.rs`만 잡는다. `model_invariants.rs`는 못 잡는다(3절 되돌리기 실험) |
| 8. `move_clip` 중간 실패 | 간접 | `review_s0_t2.rs`의 u64 끝 속성 테스트만 잡는다 |
| 9. 클립 이름·색 | 있음 | `track_set_clip_name_and_color_keep_clip_id` |
| 10. 문서 | 해당 없음 | 읽어서 확인. 7절 지적 1 |

## 3. 깨뜨리기 시도

### 3.1 1회차 수정의 되돌리기 실험

`/tmp` 아래 `git archive HEAD` 사본에서 제품 코드 일부를 되돌리고 `cargo test -p keyscore-core --no-fail-fast`를 돌렸다. 저장소의 코드는 건드리지 않았다.

| 되돌린 것 | 실패로 바뀐 테스트 | 판단 |
| --- | --- | --- |
| M1a: `move_clip`의 `new_start.checked_add(clip.extent())?` 한 줄 삭제 | `review_s0_t2::edge_tick_ops_preserve_invariants_and_failed_ops_leave_state`만. `set_start`가 `remove` 뒤에 실패해 클립이 사라진다. 리뷰 1의 깨뜨리기 테스트(`move_clip_with_hidden_tail_...`)는 클립이 없어져 통과한다 | 지적 1 해결은 확인. 단 7절 제안 2·3 |
| M1b: M1a + `set_start`도 `length`만 검사 | `review_s0_t2::move_clip_with_hidden_tail_near_u64_max_keeps_absolute_note_end_in_u64` | 지적 1 해결 확인. `model_invariants.rs`는 두 경우 모두 통과(틱 범위가 작아 절대 끝 검사가 발동하지 않음) |
| M2: `set_clip_name` 몸체를 아무것도 안 하게 | `track_set_clip_name_and_color_keep_clip_id` | 지적 2 해결 확인 |
| M3: `PitchOffset::MAX = 1200`(센트로 되돌림) | `pitch_offset_beyond_one_octave_is_rejected`, 이번에 더한 `pitch_offset_bounds_match_decision_q3_exactly` | 지적 3 해결 확인 |
| M4: `from_midi1`의 비트 반복 끄기(단순 시프트) | `velocity_from_midi1_follows_midi2_min_center_max_scaling`, 이번에 더한 전체 값 테스트 | 결정 Q2 반영 확인 |
| M4b: `to_midi1`을 하위 7비트 절삭(`& 0x7F`)으로 | `velocity_midi1_round_trip_is_lossless`, `track_new_has_documented_defaults`, 이번에 더한 u16 전체 테스트 | M2-1 「상위 비트 절삭이 아님」 확인 |
| M5: 트랙별 카운터(1회차 방식)로 되돌림 | `track_clip_id_from_another_track_is_unknown`, `review_s0_t2::clip_id_from_another_track_addresses_unrelated_clip` | 제안 4 해결 확인(독립으로 만든 트랙 사이) |

### 3.2 새 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 명세 부록의 일반형 scaleUp(비트 수를 인자로 받음)을 독립적으로 옮겨 0–127 전체를 `from_midi1`과 비교. 단조 증가, 0·64·127 → 0·0x8000·0xFFFF, 128–255 거부. Python으로 따로 계산해 0x200(1), 0x7E00(63), 0x8208(65), 0xC104(96), 0xC924(100), 0xFDF7(126)도 대조 | 못 깨뜨림 | `velocity_from_midi1_matches_spec_scale_up_for_every_7bit_value` |
| 2 | u16 전체 65,536값에서 `to_midi1` = 명세 scaleDown(`>> 9`) | 못 깨뜨림 | `velocity_to_midi1_matches_spec_scale_down_for_every_16bit_value` |
| 3 | 오프셋 0, ±1, ±MAX, ±(MAX−1) 허용과 ±(MAX+1), `i32::MAX`, `i32::MAX−1`, `i32::MIN`, `i32::MIN+1` 거부. `SEMITONE = 33,554,432`, `MAX = 402,653,184` | 못 깨뜨림 | `pitch_offset_bounds_match_decision_q3_exactly` |
| 4 | 8개 스레드가 각자 트랙에 클립 500개를 넣고 3개 중 1개를 분할. 모든 식별자 유일 | 못 깨뜨림 | `clip_ids_are_unique_across_threads` |
| 5 | 트랙 A·B에 삽입·분할, A↔B·A→A·B→B로 클립을 꺼내 다시 넣기, B를 A의 복제로 바꾸기를 섞은 수열 512사례. 매 연산 뒤 각 트랙 안 식별자 유일, 다시 넣은 클립은 옛 식별자를 받지 않음 | 못 깨뜨림. 한 트랙 안 중복은 공개 API로 만들 수 없다 | `clip_transfer_between_cloned_tracks_never_duplicates_id_within_track` |
| 6 | `Project`에 트랙 `a.clone()`과 `a`를 넣고, 트랙 0에서 얻은 식별자로 트랙 1의 `remove_clip` | 불변식은 못 깨뜨림. 트랙 1의 클립이 지워진다(`Some`). 실험만 하고 테스트로 남기지 않았다(7절 제안 4) | 없음 |
| 7 | 같은 연산으로 만든 두 트랙의 `==`, 꺼낸 클립을 같은 자리에 다시 넣은 트랙과 원본의 `==` | 둘 다 `false`. 실험만 하고 남기지 않았다(7절 제안 5) | 없음 |
| 8 | `Pitch 127` + `PitchOffset::MAX`, `Pitch 0` − `MAX`를 7.25 절대 피치로 계산 | 둘 다 받아들여진다. 4,664,066,048(u32 최대 4,294,967,295 초과), −402,653,184. 실험만(7절 제안 6) | 없음 |
| 9 | `Velocity::new(1)`, `Velocity::new(0x1FF)`의 `to_midi1` | 둘 다 0. 실험만(7절 제안 7) | 없음 |

실험 7을 처음 병렬로 돌렸을 때 한 번 「같은 연산으로 만든 두 트랙」이 `true`로 찍혔다. 같은 바이너리를 병렬·단독으로 430번 다시 돌렸지만 재현되지 않았고 모두 `false`였다. `fetch_add`는 원자적이라 중복 발급의 근거가 없어 판정에 쓰지 않았다. 출력을 잘못 읽었을 가능성이 크다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 아니오 | 1회차와 같음. 기존 속성 테스트 통과 |
| 노트 시작은 클립 안 | 아니오 | 1회차와 같음 |
| `insert_note`는 끝까지 클립 안 | 아니오 | `clip.rs:105-121` |
| 노트 절대 끝(클립 시작 + 노트 끝) u64 안 | 아니오 | `move_clip`이 `extent()`로 먼저 검사(`track.rs:336`), `set_start`도 검사(`clip.rs:142-146`). 되돌리기 M1a·M1b가 리뷰 테스트에서 실패 |
| 한 트랙 안 `ClipId` 유일 | 아니오 | `insert_clip`·`split_clip`만 식별자를 만들고 늘 새로 발급(`track.rs:290, 369`). `Clip`은 식별자를 갖지 않아 꺼냈다 넣으면 새 식별자. 시도 4·5 |
| 채널 1–16, 음높이 0–127, 오프셋 ±12반음, 벨로시티 u16 | 아니오 | 생성자 검증, 필드 비공개. 시도 1·2·3 |
| 실패한 연산은 상태 불변 | 아니오 | 현재 코드에서는 성립. 단 `move_clip`의 `remove` 뒤 `?`는 사전 검사에 기대는 구조(제안 2) |
| `pub` 필드 없음 | 지켜짐 | `ClipId(pub(crate) u64)`만 crate 내부 공개 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| SEQ-3 | 충족 | 시작 `Tick`(u64) `note.rs:121`, 길이 u32 `note.rs:122`, 음높이 0–127 `note.rs:11-17`, 피치 오프셋 ±1옥타브(결정 Q3 해상도) `note.rs:31-49`, 벨로시티 0–65535 `note.rs:64-74`, 채널은 트랙 `track.rs:124` |
| SEQ-7 | 충족(이 Task 범위) | 속성 `track.rs:120-133`, 10번 드럼 `track.rs:9, 28-30`. 채널 자동 배정은 계획 「범위 밖」(S4-T3) |
| M2-1 | 충족(이 Task 범위) | 16비트 벨로시티 보관 `note.rs:64`, 7→16 명세 scaleUp과 0–127 전부 일치 `note.rs:86-102`(시도 1), 16→7 명세 scaleDown(상위 7비트)과 u16 전부 일치 `note.rs:110-112`(시도 2). CC 32비트 `note.rs:216-220`. 벤드는 S6 |
| M2-3 (W) | 해당 없음(v1 제외) | 모델 해상도만 결정 Q3로 맞췄다 `note.rs:25-38`. 음높이 + 오프셋의 범위는 제안 6 |
| 5.10 클립 규칙 | 충족(이 Task 범위) | (트랙, 시작, 길이)와 겹침 없음 `track.rs:266-293`, 노트는 클립 기준 `note.rs:117`, 이름·색 `track.rs:300-319`, 색 `None` = 트랙 색 `clip.rs:90-98`. 분할 숨김 꼬리(결정 Q1) `clip.rs:148-176`. 자동 생성·연장은 S0-T7, 비파괴 트림은 S4-T2 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 해당 없음(`core`만 변경, 스케줄러 코드 없음). `ClipId::allocate`의 원자 연산은 편집 경로에서만 불린다.
- 제품 코드의 `unwrap`/`expect`: 없음(`#[cfg(test)]` 위쪽을 `rg`로 확인).
- 패닉 가능 지점: 1회차 목록과 같고 모두 안전. 새로 생긴 `clip.rs:137` `extent()`는 `fold`라 패닉 없음. `to_midi1`의 `as u8`은 `>> 9` 뒤라 안전.
- 에러가 삼켜지는 곳: 없음. 다만 `track.rs:343-345`의 `remove` 뒤 `set_start(..)?`는 실패하면 `Err`를 돌려주면서 클립을 잃는다. 지금은 `track.rs:336`의 같은 검사 때문에 도달하지 않는다(되돌리기 M1a로 실제로 클립이 사라지는 것을 확인).
- `ClipId` 카운터: `fetch_add`는 u64 끝에서 조용히 0으로 돌아간다. 2^64번 발급 뒤에만 일어나 현실적으로 도달하지 않는다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 개정된 결정 Q2·Q3과 어긋나는 문서가 남아 있다. crate README의 공개 API 설명이 `PitchOffset`(센트, ±1200), `Velocity`(v1 0–127)다. 계획 파일은 관련 결정 요약이 「벨로시티 0–127」, 테스트 4의 설명이 「±1200 센트」다. 보고서 「구현 요약」은 `PitchOffset`(센트)·`Velocity`(v1 0–127), 「리뷰어에게」는 「정수 센트 해석을 봐 달라」로 남아 있다. README는 S0-T5(계이름 `v<0–127>`)·S0-T6(`.ksq` 값 범위) 구현자가 읽는 공개 API 문서라 틀린 단위가 그대로 옮겨질 수 있다. 결정 Q2(16비트, 변환 함수)·Q3(반음 1/2^25, ±402,653,184)에 맞게 고칠 것. 보고서의 1회차 이전 서술은 지우지 말고 「리뷰 1 뒤 바뀜」으로 표시해도 된다 | `crates/keyscore-core/README.md:7`, `docs/reports/s0-t2/plan.md:6`, `docs/reports/s0-t2/plan.md:102`, `docs/reports/s0-t2/report.md:13`, `docs/reports/s0-t2/report.md:76` |
| 2 | 제안 | `move_clip`은 클립을 `remove`한 뒤 `set_start(..)?`를 부른다. 지금은 바로 위 사전 검사가 같은 조건을 막지만, 두 검사가 어긋나는 순간(되돌리기 M1a) `Err`를 돌려주면서 클립이 사라진다(데이터 손실). `set_start`를 `remove` 전에 제자리에서 부르거나(겹침 검사는 길이만 쓰므로 순서를 바꿔도 된다), 실패 시 원위치에 되돌리는 구조로 바꾸면 사전 검사에 기대지 않는다 | `crates/keyscore-core/src/track.rs:331-348` |
| 3 | 제안 | 지적 1(1회차) 회귀를 「이후 모든 단계의 회귀 테스트」인 `model_invariants.rs`가 잡지 못한다. 보고서는 공용 검사기에 절대 끝 검사를 더했다고 했지만, 이 파일의 틱 범위가 `0..20_000`이라 그 검사가 발동하지 않는다(되돌리기 M1a·M1b 모두 통과). 지금은 `review_s0_t2.rs`만 잡는다. 또 1회차 깨뜨리기 테스트는 `move_clip`의 결과를 단언하지 않아, 클립이 사라지는 퇴화(M1a)에서 통과한다. `model_invariants.rs`의 `Op` 전략에 u64 끝 근처 틱을 섞거나, 리뷰 테스트에 `Err(TickOverflow)`와 상태 불변 단언을 더할 것 | `crates/keyscore-core/tests/model_invariants.rs:12`, `crates/keyscore-core/tests/model_invariants.rs:147-150`, `crates/keyscore-core/tests/review_s0_t2.rs:39` |
| 4 | 제안 | 1회차 제안 4는 독립으로 만든 트랙 사이에서만 해결되었다. `Track::clone`은 식별자를 복사하므로 복제 트랙을 같은 `Project`에 넣으면 트랙 0의 식별자로 트랙 1의 클립이 지워진다(시도 6). REQ 5.10 「v1의 복제는 독립 사본」이라 트랙·클립 복제 기능(S4)을 `clone`으로 만들면 이 별칭이 사용자 데이터에 생긴다. `Clone` 문서에 「식별자를 공유한다」고 적고, 프로젝트 안에 넣는 복제는 식별자를 새로 발급하는 별도 메서드로 만드는 것을 S4-T2·S4-T3 계획에 넣을 것 | `crates/keyscore-core/src/track.rs:119`, `crates/keyscore-core/src/clip.rs:5-18` |
| 5 | 제안 | 1회차 제안 5는 부분 해결이다. 카운터 필드는 없어졌지만 `PartialEq`가 `ClipId`를 비교하므로, 클립을 꺼냈다가 같은 자리에 다시 넣은 트랙은 원본과 다르다(시도 7). S0-T4의 역연산 undo는 「같은 식별자로 복원」하는 crate 내부 API가 필요하다. S0-T6에서는 (가) 식별자를 저장하지 않으면 「왕복 동일」(derived `PartialEq`)이 깨지고, (나) 저장해 복원하면 새 프로세스의 전역 카운터가 0부터 다시 세어 이후 `insert_clip`·`split_clip`이 복원된 식별자와 같은 값을 발급해 한 트랙 안 중복(차단 사유)이 생긴다. (나)를 고르면 복원 때 카운터를 최댓값 뒤로 올리는(`fetch_max`) 처리와 파일 안 중복 식별자 검증이 필요하다. S0-T4·S0-T6 계획의 「같음」 정의와 리스크에 적을 것 | `crates/keyscore-core/src/clip.rs:12-17`, `crates/keyscore-core/src/track.rs:119-133` |
| 6 | 제안 | 음높이 + 피치 오프셋이 MIDI 2.0 7.25 절대 피치(u32, 0 ≤ 값 < 128반음)로 표현되지 않는 조합(127 + 12반음, 0 − 12반음)을 받아들인다(시도 8). REQ는 이 조합의 규칙을 정하지 않는다. M2-3(이후)과 S0-T6 `.ksq`, S0-T7 조옮김(옥타브 접기)이 이 값에 묶이므로, 거부·잘라냄·허용 중 무엇인지 결정 요청을 쓸 것 | `crates/keyscore-core/src/note.rs:154-157` |
| 7 | 제안 | `to_midi1`은 16비트 1–0x1FF를 7비트 0으로 내린다(시도 9). MIDI 1.0에서 Note On 벨로시티 0은 Note Off다. 결정 Q2가 이 처리를 S1로 미뤘으므로 지금 고칠 것은 없지만, 문서 주석이 「MIDI 1.0 송신은 `to_midi1`로 변환한다」고만 해 송신 코드가 이 함수를 그대로 쓸 수 있다. 주석에 「Note On 송신은 결과 0을 그대로 보내면 안 된다(S1)」를 적고, S1 계획에서 MIDI 2.0 명세의 2.0→1.0 변환 절(Note On 벨로시티가 0이 되는 경우의 규칙)을 원문으로 확인할 것 | `crates/keyscore-core/src/note.rs:104-112` |
| 8 | 제안 | `review_s0_t2.rs`의 시도 4 테스트 이름 `clip_id_from_another_track_addresses_unrelated_clip`이 이제 반대를 단언한다(`None`). 이름이 동작을 설명하는 규칙(`동작_조건_기대결과`)에 맞게 바꾸는 것을 권한다. 리뷰어 테스트라 지우지는 말 것 | `crates/keyscore-core/tests/review_s0_t2.rs:97-109` |

1회차 지적 확인 요약:

- 지적 1: 해결. `extent()` 검사(`track.rs:336`, `clip.rs:142-146`). 깨뜨리기 테스트 통과, 되돌리면 실패(M1b).
- 지적 2: 해결. `set_clip_name`·`set_clip_color`, 식별자 유지, 없는 식별자는 `UnknownClip`. 되돌리면 실패(M2).
- 지적 3: 해결. 결정 Q3 기록, `PitchOffset(i32)` 반음 1/2^25, ±402,653,184 포함 경계. 되돌리면 실패(M3). 같은 검토에서 결정 Q2가 M2-1에 맞게 개정되고 구현이 명세 변환과 일치한다(시도 1·2).
- 제안 4: 독립 트랙 사이에서 해결(M5로 확인). 복제 트랙 사이는 남음(제안 4).
- 제안 5: 부분 해결(제안 5).

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

공개 API로 불변식을 깨뜨리지 못했다. 한 트랙 안에 같은 `ClipId` 두 개를 만드는 경로도 찾지 못했다(시도 4·5). 1회차 지적 1~3은 모두 해결되었고, 되돌리면 테스트가 실패로 바뀐다. 벨로시티 변환은 0–127과 u16 전체에서 MIDI 2.0 명세의 scaleUp·scaleDown과 일치한다.

남은 「수정」은 지적 1(개정 결정과 어긋나는 README·계획·보고 문서) 하나다. 재리뷰 범위는 그 문서 수정과 제안에 대한 구현자의 수용·거절 기록으로 한정한다. 제품 코드가 바뀌지 않으면 깨뜨리기를 다시 할 필요는 없다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(새 테스트 5개 모두 통과 = 못 깨뜨림, 실험 4개, 되돌리기 7개)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`scripts/check-deps.test.sh`는 1회차가 정당하다고 판단, 이후 변경 없음)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`review_s0_t2.rs` 시도 4의 기대값 변경은 1회차가 「현재 동작 기록」으로 둔 것이고 커밋 `256adcb` 본문에 이유가 있다)
- [x] 새 의존성이 있으면 라이선스를 확인했다(1회차 이후 새 의존성 없음, `cargo deny check` 통과)
- [x] 스타일 지적을 적지 않았다

## 부록: 자동 게이트 실행 결과

대상 커밋 `246215d`에 `tests/review_s0_t2_r2.rs`를 더한 작업 트리 기준:

~~~
cargo fmt --all -- --check                             통과
cargo clippy --workspace --all-targets -- -D warnings  통과
cargo test --workspace                                 통과(core lib 28, model_invariants 4, review_s0_t2 5, review_s0_t2_r2 5)
cargo deny check                                       advisories ok, bans ok, licenses ok, sources ok
scripts/check-deps.sh                                  check-deps: ok
scripts/check-deps.test.sh                             11 passed, 0 failed
~~~

리뷰 테스트를 더하기 전 대상 커밋에서도 위 게이트가 모두 통과했다(core lib 28, model_invariants 4, review_s0_t2 5).
