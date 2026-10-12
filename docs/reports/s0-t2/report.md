# Task 보고: S0-T2 핵심 모델 타입

- PR: 로컬 브랜치 `s0/t2-core-model` (remote 없음)
- 계획 파일: docs/reports/s0-t2/plan.md
- 계획 대비 달라진 점
  - 트랙 안 클립의 노트 편집은 `ClipNotesMut` 핸들 대신 `Track::insert_note`/`remove_note`로 했다. 계획 파일의 인터페이스도 함께 고쳤다.
  - `Velocity::TRACK_DEFAULT`(crate 내부, 100)를 더했다. 새 트랙 기본 벨로시티의 숫자는 REQ에 없어서 흔한 시퀀서 기본값을 따랐다.
  - S0-T1의 `scripts/check-deps.test.sh`를 고쳤다(`d11e520`). `core`에 `[dev-dependencies]`가 생기자 기존 주입 방식이 의도와 다르게 동작했다. 아래 「리뷰어에게」 참고.

## 구현 요약

- `src/tick.rs`: `Tick`, `PPQ`, 오버플로를 검사하는 `checked_add`.
- `src/note.rs`: `Pitch`, `PitchOffset`(센트), `Velocity`(v1 0–127), `Note`(생성자에서 길이 ≥ 1과 끝 오버플로를 검사하고, 빈 가사는 `None`), `CcPoint`.
- `src/clip.rs`: `Clip`, `ClipId`. `insert_note`는 노트 끝까지 클립 안이어야 받고, `(시작, 음높이)` 정렬을 유지한다. crate 내부의 `split_at`은 실패할 수 있는 계산을 모두 끝낸 뒤에 상태를 바꾼다.
- `src/track.rs`: `Channel`, `TrackColor`, `Program`, `Bank`, `OutputPort`, `Track`. 클립은 시작 순으로 정렬해 둔다. 삽입·이동은 겹침을 검사하고, 분할하면 오른쪽 클립을 바로 뒤에 놓는다. 클립 식별자는 재사용하지 않는다.
- `src/key.rs`: `PitchClass`, `Key`. `src/project.rs`: `Project`(조성과 트랙). `src/error.rs`: `ModelError`.
- `tests/model_invariants.rs`: 공개 API만 쓰는 proptest 4개.
- 의존 추가: `thiserror`(normal), `proptest`(dev). 둘 다 `check-deps.sh`의 `core` 허용 목록에 있다.

## 테스트 결과

~~~
keyscore-core lib: test result: ok. 24 passed; 0 failed
model_invariants:  test result: ok. 4 passed; 0 failed (각 512 사례)
check-deps.test:   11 passed, 0 failed
~~~

테스트를 먼저 썼고, 몸체가 `todo!()`인 뼈대에서 단위 23개와 통합 4개가 모두 실패하는 것을 확인했다(`95699cd`). 실패하지 않은 단위 테스트는 `key_default_is_none` 하나다. `#[derive(Default)]`라서 처음부터 구현되어 있었다.

속성 테스트가 실제 상태 변화를 검사하는지 보려고, 연산별 성공·실패 횟수를 임시로 계측했다(확인 후 되돌림). 클립 삽입 2,877/1,905, 노트 삽입 2,211/3,941, 이동 2,308/827, 분할 644/2,501(성공/실패)로 고르게 섞였다. 분할 보존 속성은 460개 사례 중 242개가 노트가 있는 클립을 분할했다.

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `channel_new_zero_or_seventeen_is_rejected` | 통과 |
| 2 | `channel_ten_is_drum` | 통과 |
| 3 | `pitch_new_above_127_is_rejected` | 통과 |
| 4 | `pitch_offset_beyond_one_octave_is_rejected` | 통과 |
| 5 | `velocity_new_above_127_is_rejected_in_v1` | 통과 |
| 6 | `note_new_zero_length_is_rejected` (+ `note_new_end_overflowing_u64_is_rejected`) | 통과 |
| 7 | `note_with_empty_lyric_has_no_lyric` | 통과 |
| 8 | `clip_new_zero_length_or_overflowing_end_is_rejected` | 통과 |
| 9 | `clip_insert_note_past_end_is_rejected` | 통과 |
| 10 | `clip_insert_note_keeps_notes_sorted` | 통과 |
| 11 | `track_insert_overlapping_clip_is_rejected` | 통과 |
| 12 | `track_move_clip_onto_neighbor_is_rejected_and_state_unchanged` | 통과 |
| 13 | `track_split_clip_keeps_crossing_note_in_left_as_tail` | 통과 |
| 14 | `track_split_clip_at_boundary_is_rejected` | 통과 |
| 15 | `track_ops_random_sequence_preserve_invariants` | 통과 |
| 16 | `track_ops_failed_op_leaves_track_unchanged` | 통과 |
| 17 | `track_split_clip_preserves_absolute_notes` | 통과 |
| 18 | `track_move_clip_moves_notes_with_it` | 통과 |
| 추가 | `tick_checked_add_overflow_is_rejected`, `clip_remove_note_removes_only_one_equal_note`, `clip_set_empty_name_has_no_name`, `small_value_types_reject_out_of_range`, `track_new_has_documented_defaults`, `track_note_ops_on_unknown_clip_are_rejected`, `track_clip_ids_are_not_reused_after_remove`, `pitch_class_new_above_11_is_rejected`, `key_default_is_none` | 통과 |

계획에 있었지만 쓰지 않은 테스트: 없음.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings (pedantic 포함)
- [x] test
- [x] deny: `advisories ok, bans ok, licenses ok, sources ok`. 쓰이지 않은 허용 라이선스에 대한 경고만 있다.
- [x] `scripts/check-deps.sh` ok, `scripts/check-deps.test.sh` 11/11
- [x] 전이 그래프 확인(S0-T1 review-3 제안 1): `cargo tree -p keyscore-core --target all -e normal,build,dev`에 UI·MIDI·네트워크 crate가 없다. `windows-sys`, `linux-raw-sys`, `r-efi`는 proptest(tempfile·getrandom)가 다른 OS용으로 끌어오는 것이고, 라이선스는 모두 허용 목록과 OR로 묶인다.

## 알려진 한계

- 분할로 생긴 숨김 꼬리를 재생·내보내기에서 클립 끝에서 자르는 일은 S1·S0-T5·S0-T6이 맡는다. 모델은 꼬리를 보존만 한다.
- 비파괴 트림, 합치기, 복제, 트랙 간 이동은 S4-T2의 범위다. 합치기를 만들면 「분할 → 합치기 = 원본」 속성 테스트를 그 Task에 넣어야 한다(결정 Q1의 약속).
- 클립 검색과 겹침 검사는 클립 수에 선형이다. 트랙당 클립은 수백 개 수준이라 문제없다고 보지만, 성능 게이트는 없다.
- 같은 음높이 노트의 겹침과 벨로시티 0 노트는 허용한다. MIDI 1.0 송신 규칙은 S1에서 정한다.

## 리뷰어에게

- 불변식의 핵심은 `Track::move_clip`·`split_clip`과 `Clip::split_at`이다. 실패할 때 상태가 불변인지(계산을 끝낸 뒤 변경하는지) 봐 달라.
- `Track`은 `&mut Clip`을 내주지 않는다. `Project::track_mut`는 `&mut Track`을 내주는데, `Track`이 스스로 불변식을 지키므로 괜찮다고 판단했다.
- `PitchOffset`을 정수 센트로 둔 해석(계획의 「리스크」)이 SEQ-3의 「소수」와 맞는지 봐 달라.
- `scripts/check-deps.test.sh` 수정(`d11e520`)은 S0-T1 파일을 건드린다. 이 Task가 `core`에 dev 의존을 처음 넣으면서 드러난 결함이라 이 PR에서 고쳤다. 고치기 전에는 「직접 의존」 사례 일부가 dev 의존으로 바뀐 채 통과하고 있었다.

## 리뷰 대응 (review-1, 판정: 수정 후 재리뷰)

| # | 지적 | 대응 |
| --- | --- | --- |
| 1 | 숨김 꼬리 클립을 u64 끝으로 옮기면 노트의 절대 끝이 넘침 | `Clip::extent()`(길이와 가장 늦은 노트 끝 중 큰 값)로 `move_clip`·`set_start`를 검사한다. 검사는 클립을 빼내기 전에 한다. 공용 불변식 검사기(`model_invariants.rs`)에 절대 끝 검사를 추가했다. 리뷰어의 깨뜨리는 테스트가 이제 통과한다 |
| 2 | 트랙 안 클립의 이름·색 API 없음 | `Track::set_clip_name`, `Track::set_clip_color`를 추가했다. 식별자는 유지된다 |
| 3 | 피치 오프셋 단위를 결정 요청 없이 확정 | 받음. 사람에게 물어 MIDI 2.0 해상도(i32, 반음 1/2^25)로 정했다(결정 Q3). 같은 검토에서 REQ M2-1(16비트 벨로시티 보관)이 드러났다. 앞선 벨로시티 질문이 M2-1을 빠뜨렸던 것이라, 다시 물어 16비트 보관으로 바꿨다(결정 Q2 개정) |
| 4 | 트랙마다 0부터 발급한 `ClipId` | 받음. 프로세스 전역 원자 카운터로 발급한다. 리뷰어가 「현재 동작 기록」으로 남긴 테스트는 기대값을 `None`으로 바꿨다(커밋 `256adcb` 본문에 이유) |
| 5 | `next_clip_id`가 `PartialEq`에 들어감 | 4로 해결했다. `Track`에 카운터 필드가 없다 |

테스트를 먼저 바꿨고(`256adcb`), `todo!()` 뼈대에서 새 테스트 15개와 속성 테스트가 실패하는 것을 확인했다. 수정은 파일이 겹쳐서 커밋 하나로 묶었다.

~~~
keyscore-core lib: 28 passed
model_invariants:  4 passed
review_s0_t2:      5 passed
check-deps.test:   11 passed
~~~

벨로시티 변환 값은 MIDI 2.0 명세의 min-center-max 확장으로 손으로 계산해 고정했다(64 → 0x8000, 65 → 0x8208, 100 → 0xC924, 127 → 0xFFFF). 0–127 전체의 왕복 무손실도 테스트한다. 새 트랙 기본 벨로시티는 MIDI 1.0 100에 해당하는 0xC924다.
