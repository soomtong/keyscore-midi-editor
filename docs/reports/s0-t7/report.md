# Task 보고: S0-T7 퀀타이즈·조옮김·길이 스케일 연산

- PR: 로컬 브랜치 `s0/t7-edit-ops` (remote 없음)
- 계획 파일: docs/reports/s0-t7/plan.md
- 계획 대비 달라진 점: `Pitch::MIN`·`Pitch::MAX` 공개 상수를 더했다(전체 음역 표현용). 그 밖에는 계획대로다.

## 구현 요약

- `src/transform.rs`
  - `NoteRef`, `Grid`, `QuantizeSettings`, `PitchRange`와 `quantize`·`transpose`·`shift`·`scale`.
  - 공통 `plan`이 노트마다 새 노트를 계산한다. 끝이 클립을 넘으면 마디 경계까지 늘릴 길이를 정하고, 다음 클립과의 겹침을 검사한다. 결과 묶음은 `ExtendClip…`, `RemoveNote…`, `InsertNote…` 순서다.
  - `snap`은 마디 기준 그리드 점(스윙 적용)과 다음 마디 시작 가운데 가장 가까운 점을 고른다. 거리가 같으면 앞쪽이다.
- `src/edit.rs`: `Edit::ExtendClip`(늘리기만)과 내부 `Op::ShrinkClip`(그 undo).
- `src/track.rs`·`src/clip.rs`(crate 내부): `resize_clip`(겹침 검사), `set_length`(줄일 때도 노트 시작은 클립 안).
- `src/note.rs`(crate 내부): `with_length`, `with_pitch`(절대 피치 범위 재검사). 공개 상수 `Pitch::MIN`·`MAX`.
- `src/error.rs`: `ClipWouldShrink`, `NoteBeforeClipStart`, `PitchRangeTooNarrow`, `QuantizeSettingOutOfRange`, `ScaleRatioInvalid`.

## 테스트 결과

~~~
cargo test --workspace: 187 passed
transform 속성 테스트(각 256사례) 임시 계측: 퀀타이즈 511회 중 508회 성공, 일괄 연산 248회 중 240회 성공(실패 8회는 시간 이동의 「클립 시작 앞」 거부)
~~~

테스트를 먼저 썼다(`dc6afc0`). `todo!()` 뼈대에서 단위 12개와 속성 4개가 실패하는 것을 확인했다.

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `quantize_full_strength_is_idempotent` | 통과 |
| 2 | `quantize_swing_zero_equals_plain_grid`(테스트 안의 독립 계산과 비교) | 통과 |
| 3 | `quantize_snaps_to_bar_relative_grid`, `quantize_bar_end_uses_next_bar_start` | 통과 |
| 4 | `quantize_strength_and_swing_values` | 통과 |
| 5 | `quantize_triplet_grid`, `quantize_settings_validate_and_grid_ticks` | 통과 |
| 6 | `transpose_by_octave_inside_range_is_exact` | 통과 |
| 7 | `transpose_folds_into_range`(좁은 음역, 오프셋 노트 포함) | 통과 |
| 8 | `shift_past_end_extends_clip_by_bars`, `shift_before_start_is_rejected` | 통과 |
| 9 | `extend_into_next_clip_is_rejected` | 통과 |
| 10 | `scale_doubles_spacing_and_length` | 통과 |
| 11 | `transform_edits_apply_as_one_undo_unit` | 통과 |
| 12 | `extend_clip_edit_round_trip_and_rejects_shrink` | 통과 |
| 추가 | `stale_reference_is_rejected_on_apply` | 통과 |

## 자동 게이트

- [x] fmt, clippy -D warnings, test, deny
- [x] `check-deps.sh`, `check-deps.test.sh` 11/11, `bench-check.test.sh` 8/8

## 알려진 한계

- 연산은 선택 노트를 「지우고 다시 넣기」로 표현한다. 같은 값의 노트가 여럿이면 어느 것을 지웠는지 구분하지 않는다(노트는 값으로 식별된다). 결과 상태는 같다.
- 숨김 꼬리 노트를 옮기면 결과 끝까지 클립을 늘려 꼬리가 드러난다(계획의 해석).
- 트랙 음역 설정은 아직 없다. 호출자가 `PitchRange`로 넘긴다.
- 퀀타이즈 대상 노트가 한 마디보다 멀리 떨어진 그리드 점으로 갈 일은 없다. 후보가 같은 마디 안 세 칸과 다음 마디 시작뿐이다.

## 리뷰어에게

- `snap`의 후보 선택(스윙 점, 마디 끝, 거리 동점)과 강도 계산의 반올림 방향이 결정 문서와 계획의 해석에 맞는지 봐 달라.
- `plan`이 만든 묶음이 원자적으로 적용되는지 봐 달라. `ExtendClip`이 먼저 오고, 그 뒤 노트 삭제·삽입이다. 클립 경계 거부(앞쪽, 다음 클립과 겹침)도 함께 봐 달라.
- 여러 클립·여러 트랙에 걸친 선택과, 같은 노트를 두 번 고른 선택을 봐 달라.
