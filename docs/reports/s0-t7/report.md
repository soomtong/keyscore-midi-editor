# Task 보고: S0-T7 퀀타이즈·조옮김·길이 스케일 연산

- PR: 로컬 브랜치 `s0/t7-edit-ops` (remote 없음)
- 계획 파일: docs/reports/s0-t7/plan.md
- 계획 대비 달라진 점: `Pitch::MIN`·`Pitch::MAX` 공개 상수를 더했다(전체 음역 표현용). 그 밖에는 계획대로다.

## 구현 요약

- `src/transform.rs`
  - `NoteRef`, `Grid`, `QuantizeSettings`, `PitchRange`와 `quantize`·`transpose`·`shift`·`scale`.
  - 공통 `plan`이 노트마다 새 노트를 계산한다. 끝이 클립을 넘으면 마디 경계까지 늘릴 길이를 정하고, 다음 클립과의 겹침을 검사한다. 결과 묶음은 클립마다 `ReplaceNotes` 하나다(리뷰 2 뒤 개정, 처음에는 `ExtendClip…`, `RemoveNote…`, `InsertNote…` 순서였다).
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

- (리뷰 2 뒤 개정) 연산은 클립마다 `ReplaceNotes` 하나로 표현한다. 처음에는 「지우고 다시 넣기」였다. 같은 값의 노트가 여럿이면 어느 것을 바꿨는지 구분하지 않는다(노트는 값으로 식별된다). 결과 상태는 같다.
- (리뷰 1 뒤 결정 개정) 숨김 꼬리 노트는 꼬리로 둔다. 처음 해석(결과 끝까지 늘려 드러냄)은 폐기했다.
- 트랙 음역 설정은 아직 없다. 호출자가 `PitchRange`로 넘긴다.
- 퀀타이즈 대상 노트가 한 마디보다 멀리 떨어진 그리드 점으로 갈 일은 없다. 후보가 같은 마디 안 세 칸과 다음 마디 시작뿐이다.

## 리뷰어에게

- `snap`의 후보 선택(스윙 점, 마디 끝, 거리 동점)과 강도 계산의 반올림 방향이 결정 문서와 계획의 해석에 맞는지 봐 달라.
- `plan`이 만든 묶음이 원자적으로 적용되는지 봐 달라. (리뷰 2 뒤) 클립마다 `ReplaceNotes` 하나가 길이 늘리기와 노트 바꾸기를 함께 한다. 클립 경계 거부(앞쪽, 다음 클립과 겹침)도 함께 봐 달라.
- 여러 클립·여러 트랙에 걸친 선택과, 같은 노트를 두 번 고른 선택을 봐 달라.

## 리뷰 대응 (review-1, 판정: 수정 후 재리뷰)

| # | 지적 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 숨김 꼬리 때문에 조옮김·`shift(0)`이 클립을 늘리거나 거부됨 (차단) | 사람에게 물어 결정을 개정했다(꼬리는 꼬리로 둔다). 원래 꼬리였던 노트는 시작만 클립 안이면 되고, 시작이 밖으로 나갈 때만 늘린다. 연산은 `Edit::ReplaceNote`로 바꿨다. 새 노트는 원래 노트가 꼬리였을 때만 꼬리일 수 있어, 「꼬리는 분할로만」이 유지된다. 역편집은 내부 `Op::UnreplaceNote`다 | `core: Keep hidden tails as tails in transforms via ReplaceNote` |
| 2 | `transpose`의 i32 오버플로와 반음 수에 비례하는 접기 반복 | i64로 계산하고, 접을 옥타브 수를 한 번에 구한다 | `core: Compute transpose folding in i64 without a loop` |
| 3 | 같은 노트를 두 번 고른 선택이 `Ok` 뒤 적용 실패 | 계산 단계에서 고른 개수와 클립의 개수를 해시로 비교해 거부한다(선택 크기에 선형) | `core: Reject selections that pick a note more often than it exists` |
| 4 | u64 초과를 「클립 시작 앞」으로 보고 | 음수만 `NoteBeforeClipStart`, u64 초과는 `TickOverflow` | `core: Report tick overflow, not before-start, for oversized transforms` |
| 5 | 여러 클립에 걸친 반속이 언제나 거부됨 (제안) | 사람에게 물어 결정을 개정했다(클립마다 기준점). 리뷰어 테스트의 기대값을 뒤집고 이름을 바꿨다(커밋 본문에 이유) | `core: Anchor scale at each clip's first selected note`, `core: Expect per-clip scale anchor in review test` |
| 6 | 구현 해석이 결정 문서에 없음 (제안) | 받음. 결정 문서에 부록으로 옮겼다 | `plan:` |
| 7 | 1/4보다 긴 그리드와 점음표 그리드 (제안) | 결정 요청 `docs/decisions/2026-10-12-quantize-grid-lengths.md`. S5 전까지 정하면 된다 | `plan:` |

제가 고친 리뷰어 테스트에서 한 글자 변수 이름 때문에 clippy가 실패해 이름을 바꿨다(별도 커밋). 이번에는 지적마다 커밋을 나눴다.

~~~
cargo test --workspace: 206 passed (리뷰어 테스트 18개 포함)
~~~

## 리뷰 대응 (review-2, 판정: 수정 후 재리뷰)

| # | 지적 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | 꼬리 판정 시점이 `plan`(늘리기 전)과 `ReplaceNote` 적용(늘린 뒤)에서 달라 `Ok` 묶음이 적용에서 실패 (차단) | 클립 단위 편집 `Edit::ReplaceNotes { length, pairs }`를 더했다. 길이 늘리기와 노트 바꾸기를 한 편집으로 하고, 꼬리 여부는 편집 직전 길이로 판정한다. 일괄 연산은 클립마다 이것 하나를 낸다. 처음에는 「늘린 뒤 꼬리가 아니게 되면 끝까지 덮을 때까지 반복」으로 고쳤다가, 리뷰어 테스트가 결정(꼬리는 꼬리로)과 어긋남을 보여 되돌렸다 | `core: Add ReplaceNotes clip-level edit skeleton and test`, `core: Implement ReplaceNotes and emit it from transforms` |
| 2 | 결정 개정 규칙에 구현자 테스트가 없음 | 테스트 3개(`tail_note_keeps_clip_length_when_start_stays_inside`, `tail_start_beyond_extended_length_still_applies`, `scale_anchors_each_clip_at_its_first_selected_note`)를 더했다. 수정 뒤에 썼으므로 대신 확인했다: 셋 모두 최초 구현(`5ad9592`)에서 실패하고, 꼬리 시작 사례는 `ReplaceNotes` 이전에도 실패한다 | `core: Test amended transform rules …` |
| 3 | 낡은 문서(`scale` 문서, 보고서 「알려진 한계」) | 고쳤다 | `core: Fix stale transform docs …` |
| 4 | 꼬리 모양은 바뀔 수 있음을 불변식 문구에 (제안) | 받음. `clip.rs` 문서와 README에 「꼬리 개수는 분할로만 늘고, 모양은 편집으로 바뀔 수 있다」를 적었다 | 같음 |

리뷰어 1회차 테스트 `review_stale_transform_after_extend_is_atomic`는 묶음 모양(첫 편집이 `ExtendClip`)을 단언해서 새 모양(`ReplaceNotes`가 길이를 늘림)으로 단언을 바꿨다. 의도(낡은 묶음의 원자적 실패)는 그대로다(`core: Update stale-batch review test …`). `Edit::ReplaceNote`는 리뷰어 2회차 테스트가 직접 공격하므로 남겼다.

과정 기록: 지적 2의 테스트를 커밋한 뒤에야 clippy를 돌려 한 글자 변수 이름 경고를 발견했다. 별도 커밋으로 고쳤다. 게이트는 커밋 전에 돌려야 한다.

~~~
cargo test --workspace: 227 passed, clippy 경고 0
~~~

## 리뷰 대응 (review-3, 판정: 수정 후 재리뷰)

| # | 지적 | 대응 | 커밋 |
| --- | --- | --- | --- |
| 1 | `ReplaceNotes`의 차례 적용으로 앞 pair의 `to`를 뒤 pair의 `from`으로 써서 분할 없이 꼬리 생성 (차단) | 동시 치환으로 바꿨다. 모든 `from`을 편집 직전 노트에서 찾아 꼬리 여부와 함께 먼저 지운 뒤 `to`를 넣는다. pair 순서와 무관하고, 앞 pair의 결과는 `from`이 될 수 없다(`NoteNotFound`). 리뷰어 테스트 2개가 통과한다 | `core: Replace notes simultaneously in ReplaceNotes` |
| 2 | 보고서의 옛 묶음 모양 | 고쳤다 | `plan:` |
| 3 | 「`Ok`면 적용 성공」 속성 테스트를 구현자 테스트로 (제안) | 기록만 함. 리뷰어 3회차 속성 테스트(각 2000건)가 그 역할을 하며 저장소에 남는다 | |

이번에는 게이트를 모두 돌린 뒤 커밋했다.

~~~
cargo test --workspace: 238 passed, clippy 경고 0
~~~

## 리뷰 대응 (review-4, 판정: 병합 가능)

제안 1(동시 치환 규칙을 직접 겨냥한 구현자 테스트)은 기록만 한다. 리뷰어 3·4회차 테스트(`review_s0_t7_r3.rs`, `review_s0_t7_r4.rs`)가 그 규칙을 겨냥하고 저장소에 남는다. 차례 치환으로 되돌리면 그 테스트들이 실패하는 것을 리뷰어가 확인했다.
