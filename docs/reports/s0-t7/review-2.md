# 적대적 리뷰: S0-T7 퀀타이즈·조옮김·길이 스케일 연산 (리뷰 2회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트). 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: `420aff8` (`s0/t7-edit-ops`). 1회차(`f0731f4`) 이후 제품 코드 커밋 `39db2f6`, `9632e63`, `81c6594`, `82f9fe7`, `653b44c`와 테스트 커밋 `72de9c9`, `a6844b7`
- 기준: `docs/reports/s0-t7/review-1.md`, `docs/decisions/2026-10-12-quantize-and-transform-rules.md`(「개정」, 「부록」 포함), `docs/reports/s0-t7/report.md` 「리뷰 대응 (review-1)」

## 1. 코드만 읽고 적은 「내가 검사할 것」

1회차 이후 `transform.rs`, `edit.rs`의 `ReplaceNote`/`UnreplaceNote`, `clip.rs`·`track.rs`의 `replace_note` 변경분만 읽고 적었다.

1. 꼬리 판정의 시점 불일치: `plan`은 「원래 꼬리」를 계산 시점의 클립 길이로 판정한다(`r.note.end() > clip.length()`). 그런데 적용 시점의 `Clip::replace_note`는 `from.end() > self.length`를 다시 본다. 묶음은 `ExtendClip`이 먼저 오므로, 늘린 뒤의 길이가 원래 꼬리 끝보다 길면 같은 노트가 더 이상 꼬리가 아니다. 그때 옮긴 결과의 끝이 늘린 길이를 넘으면 `plan`이 낸 `Ok` 묶음이 적용에서 거부될 것이다. 꼬리의 시작을 클립 밖으로 멀리 미는 경우, 다른 노트가 클립을 늘려 꼬리가 드러나는 경우 둘 다 볼 것.
2. 꼬리 시작이 정확히 클립 길이에 닿을 때 늘림 길이(`needed_end = start + 1`)가 그 시작을 덮는 마디 경계인가. 마디가 3840이 아닌 박자표(3/4)에서.
3. 공개 `Edit::ReplaceNote`로 분할 없이 꼬리 만들기: 일반 → 꼬리, 한 묶음 안에서 꼬리 → 일반 → 꼬리, 꼬리를 다른 클립으로 옮기기(`RemoveNote` + `InsertNote`, 다른 클립 id로 `ReplaceNote`).
4. `ReplaceNote`로 꼬리를 꼬리인 채 더 길게(분할 때의 원래 끝 너머, u32 최대 길이까지) 늘리기. 결정 Q1 「꼬리는 분할로만 생긴다」와 맞는가. u64 끝을 넘는 꼬리는 거부되는가(`checked_add`).
5. `from`이 없음, 다른 클립 id, 모르는 트랙, `to` 시작이 클립 밖, `from == to`(일반·꼬리) 각각에서 상태 불변과 undo 길이.
6. 같은 값 노트가 여럿일 때 `ReplaceNote`/`UnreplaceNote`의 `position` 첫 일치: 꼬리를 기존 일반 노트와 같은 값으로 바꾼 뒤 undo, 같은 값 둘을 연쇄로 바꾸는 묶음의 undo.
7. 중복 선택 검사: 같은 값이 둘인 클립에서 두 번은 허용·세 번은 거부, 다른 클립의 같은 값은 따로 센다. 모든 연산(`shift`, `transpose`, `quantize`, `scale`)에서 같은 오류.
8. 클립별 기준점: 여러 트랙 × 여러 클립, 클립 안 여러 노트, 선택되지 않은 더 이른 노트는 기준이 아님, 꼬리 노트가 기준인 경우. 「클립 시작 기준」과 구별되는 입력(첫 선택 노트가 0이 아닌 위치).
9. 오버플로: u64 끝 근처 클립의 꼬리 노트에 `shift(i64::MAX/MIN)`, `scale(u32::MAX/1)`. `needed_end = new.start() + 1`이 넘치지 않는가(`with_start`가 시작 + 길이를 검사하므로 시작 ≤ u64::MAX − 1이어야 한다).
10. `transpose`의 새 접기 식(`raw − ceil((raw − high) / 12) × 12`)이 1회차 경계(정확히 12칸 음역, i32 끝)에서 맞는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 꼬리 판정 시점 불일치 | 없음 | 구현자 테스트에 「꼬리 시작이 클립 밖으로 나감」이 하나도 없다 |
| 2 꼬리 시작이 클립 끝에 닿을 때 마디 경계 | 없음 | |
| 3 일반 → 꼬리 거부 | 있음 | `edit.rs` `replace_note_keeps_tails_only_from_tails`(내부 `Op`로). 묶음 연쇄·다른 클립은 없음 |
| 4 꼬리를 더 길게 | 없음 | |
| 5 `from` 없음·다른 클립·`from == to` | 없음 | |
| 6 같은 값 노트와 `ReplaceNote` | 부분 | 1회차 리뷰어 `review_identical_notes_only_selected_one_moves`만 |
| 7 중복 선택 경계 | 부분 | 1회차 리뷰어 `review_duplicate_ref_in_selection`(1개짜리 클립)만 |
| 8 클립별 기준점 | 부분 | 뒤집은 1회차 리뷰어 테스트뿐. 두 클립 모두 노트 하나가 0에 있어 「클립 시작 기준」과 구별하지 못한다 |
| 9 u64 끝 꼬리 | 없음 | |
| 10 접기 식 | 있음 | 1회차 리뷰어 `review_transpose_*` |

1회차 지적에 대한 수정은 모두 1회차 리뷰어 테스트로만 확인된다. 구현자 테스트(`transform.rs` 단위, `tests/transform.rs`)는 결정 개정의 두 규칙을 다루지 않는다.

## 3. 깨뜨리기 시도

### 3.1 1회차 지적 해결 확인 (수정을 되돌린 /tmp 사본에서)

`git archive HEAD`로 `/tmp/ks-r2`에 풀고, 수정 하나씩만 되돌려 `review_s0_t7`을 실행했다. 저장소 작업 트리는 건드리지 않았다.

| 지적 | 되돌린 것 | 결과 |
| --- | --- | --- |
| 1 숨김 꼬리(차단) | `plan`의 `needed_end`를 `new.end()`로 | 3개 실패: `review_transpose_selection_with_hidden_tail_next_to_split_neighbor`, `review_noop_shift_and_zero_strength_quantize_with_hidden_tail`, `review_transpose_does_not_resize_clip_with_hidden_tail` |
| 2 조옮김 오버플로 | i32 덧셈 + 반복 접기 | `review_transpose_huge_semitones` 실패(i32 덧셈 패닉) |
| 3 중복 선택 | 개수 비교를 끔 | `review_duplicate_ref_in_selection` 실패 |
| 4 오류 종류 | `82f9fe7` 역패치 | `review_scale_overflow_reports_tick_overflow` 실패 |
| 5(제안, 결정 개정) 클립별 기준점 | `653b44c` 역패치 | `review_scale_half_across_two_clips_uses_per_clip_anchor` 실패 |

HEAD에서는 `review_s0_t7` 18개 모두 통과한다. 지적 1–4는 해결되었고 결정 개정 두 항목이 코드에 반영되었다. 단 지적 1의 해결은 새 결함을 만들었다(아래 시도 1–3, 지적 1).

뒤집힌 리뷰어 테스트(`review_scale_half_across_two_clips_is_rejected` → `..._uses_per_clip_anchor`)는 정당하다. 원래 테스트는 결함이 아니라 「당시 결정의 결과」를 기록한 것이었고(1회차 지적 5, 제안), 사람이 결정을 개정했으며, 커밋 `72de9c9` 본문에 이유가 있고, 기대값은 개정된 규칙대로다. 다만 이 테스트는 「클립 시작 기준」 구현도 통과시키므로 시도 14로 보강했다.

### 3.2 2회차 시도

모두 `crates/keyscore-core/tests/review_s0_t7_r2.rs`에서 실행했다(공개 API만).

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 분할 뒤 이웃을 지운 왼쪽 클립(길이 3840)의 꼬리 노트(1920, 길이 3840, 끝 5760)를 `shift(+4000)` | 깨짐: `shift`는 `Ok`(ExtendClip 7680 + ReplaceNote), `apply`가 `Err(NoteOutsideClip { start: 5920, end: 9760, clip_length: 7680 })`. 늘린 뒤 길이 7680 ≥ 원래 끝 5760이라 `replace_note`가 꼬리로 보지 않는다 | `review_r2_shift_tail_start_far_out_of_clip_applies` |
| 2 | 꼬리(1920, 길이 5000, 끝 6920) + 일반 노트(3700, 100), `shift(+900)`. 일반 노트가 7680으로 늘리고 꼬리는 시작만 클립 안 | 깨짐: `apply`가 `Err(NoteOutsideClip { start: 2820, end: 7820, clip_length: 7680 })`. 원자성은 지켜짐(문서 그대로) | `review_r2_tail_revealed_by_neighbor_extension_then_overhangs` |
| 3 | 3/4에서 꼬리(3000, 길이 2000)를 `shift(+840)`해 시작을 정확히 3840에 | 깨짐: 늘림 길이는 5760(시작을 덮는 마디 경계, 맞음)이지만 `apply`가 `Err(NoteOutsideClip { start: 3840, end: 5840, clip_length: 5760 })` | `review_r2_tail_start_at_clip_end_extends_to_bar_boundary_in_three_four` |
| 4 | 이웃이 맞닿은 클립에서 꼬리 + 일반 노트 선택: 조옮김, `shift(+40)`(일반 노트 끝 = 3840), `shift(+41)` | 못 깨뜨림(조옮김·+40은 길이 3840 유지, +41은 `ClipOverlap`으로 전체 거부) | `review_r2_mixed_tail_and_plain_selection_with_neighbor` |
| 5 | 시작 `u64::MAX − 8000` 클립의 꼬리에 `shift(i64::MAX, i64::MIN, 1920, 5000, 6079, 6080)`, `scale(u32::MAX/1, 2/1, 1/u32::MAX)` | 못 깨뜨림(패닉 없음) | `review_r2_tail_near_u64_end_does_not_panic` |
| 6 | 공개 `Edit::ReplaceNote`로 일반 노트(100, 100) → 꼬리(3000, 2000) | 못 깨뜨림(`NoteOutsideClip`, 상태·undo 길이 불변) | `review_r2_replace_plain_into_tail_is_rejected` |
| 7 | 한 묶음에서 꼬리 → 일반(10, 10) → 꼬리(10, 9000) | 못 깨뜨림(두 번째에서 `NoteOutsideClip`, 묶음 전체 되돌림) | `review_r2_replace_tail_to_plain_then_back_to_tail_in_one_batch_is_rejected` |
| 8 | 꼬리를 오른쪽 클립으로: `RemoveNote` + `InsertNote`(꼬리 값), 오른쪽 클립 id로 꼬리 값을 `from`으로 `ReplaceNote` | 못 깨뜨림(각각 `Err`, `NoteNotFound`. 상태 불변) | `review_r2_tail_cannot_be_moved_to_other_clip` |
| 9 | 꼬리(끝 5760)를 `ReplaceNote`로 길이 u32::MAX 꼬리로, undo/redo. 같은 것을 u64 끝 근처 클립에서 | 허용됨(undo/redo 정상), u64 초과는 `TickOverflow`로 거부. 결정 개정과 스케일 규칙에서 따라 나오므로 결함으로 보지 않는다(지적 4, 제안) | `review_r2_replace_tail_to_longer_tail_is_allowed_but_bounded_by_u64` |
| 10 | `from` 없음, 다른 클립 id, `to` 시작 = 클립 길이, 모르는 트랙 | 못 깨뜨림(모두 `Err`, 상태·undo 길이 불변) | `review_r2_replace_rejects_missing_from_foreign_clip_and_outside_start` |
| 11 | `from == to`(일반, 꼬리) 적용·undo·redo | 못 깨뜨림 | `review_r2_replace_with_identical_value_round_trips` |
| 12 | 꼬리를 기존 일반 노트와 같은 값으로 바꾼 뒤 undo/redo/undo | 못 깨뜨림(꼬리 하나 + 일반 하나로 정확히 복원) | `review_r2_replace_tail_into_duplicate_of_existing_note_and_undo` |
| 13 | 같은 값 둘: 10→20, 10→20, 20→30 연쇄 묶음, undo 한 번 | 못 깨뜨림 | `review_r2_replace_chain_among_identical_notes_undo` |
| 14 | 같은 값 둘인 클립: 2번 선택 허용(둘 다 이동), 3번 거부, 다른 클립 같은 값은 따로 셈, 네 연산 모두 같은 오류 | 못 깨뜨림 | `review_r2_duplicate_selection_counts_per_clip` |
| 15 | 트랙 2 × 클립 2, 클립 안 3·2노트, 선택 안 된 더 이른 노트(100), 첫 선택 노트 960·1000. ×1/2와 ×3(늘린 클립이 다음 클립과 맞닿음) | 못 깨뜨림(기준점이 클립 시작도 선택 전체도 아닌 「클립의 첫 선택 노트」임을 확인) | `review_r2_scale_anchor_per_clip_across_tracks` |
| 16 | 꼬리 노트(1000, 3000)가 기준점, 이웃 맞닿음, ×1/2와 ×2 | 못 깨뜨림(×2에서 꼬리는 길이 6000 꼬리로 남고 클립 길이 3840 유지) | `review_r2_scale_with_tail_as_anchor` |
| 17 | 일반 노트(0)가 기준점, 꼬리(1920, 끝 5760) 함께 `scale(3, 1)`: 꼬리 시작 5760이 클립 밖 | 깨짐: `apply`가 `Err(NoteOutsideClip { start: 5760, end: 17280, clip_length: 7680 })`. 지적 1과 같은 원인 | `review_r2_scale_tail_start_out_of_clip_applies` |

17개 중 4개 실패(시도 1–3·17, 원인 하나).

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 아니오 | `plan`이 겹침 거부, `resize_clip` 재검사. 시도 4·15 |
| 노트 시작은 클립 안 | 아니오 | `replace_note`가 `to.start < length` 검사(restoring 포함). 시도 10 |
| 숨김 꼬리는 분할로만 생긴다 | 아니오(꼬리 수는 늘지 않는다) | 일반 → 꼬리 거부(시도 6·7), 다른 클립으로 못 옮김(시도 8). 다만 기존 꼬리는 임의 모양·길이로 바뀔 수 있다(시도 9, 지적 4) |
| 꼬리 포함 절대 끝이 u64 안 | 아니오 | `replace_note`의 `start.checked_add(to.end())`. 시도 5·9 |
| 결과 묶음의 원자성 | 아니오 | 시도 2·7 |
| 연산이 `Ok`로 낸 묶음은 적용된다(함수 문서의 약속) | 예 | 시도 1–3·17(지적 1) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| EDT-2 그리드·셋잇단·강도·스윙 | 충족 | 1회차 시도 1–3, 이번 변경 없음 |
| EDT-3 선택 영역에 조옮김 | 충족 | `transform.rs:134-158`, 1회차 시도 5–8, 시도 4 |
| EDT-3 선택 영역에 시간 이동 | 부분 충족(미충족으로 셈) | 꼬리 노트를 클립 밖으로 미는 이동, 다른 노트가 늘린 클립에서 꼬리를 미는 이동이 `Ok` 뒤 적용 실패(시도 1–3, `transform.rs:311-315`, `clip.rs:176-177`) |
| EDT-3 선택 영역에 길이 스케일 | 부분 충족(미충족으로 셈) | 기준점은 개정 결정대로(`transform.rs:198-221`, 시도 15·16). 꼬리 시작이 클립 밖으로 나가는 배속은 `Ok` 뒤 적용 실패(시도 17, 지적 1) |
| 5.10 끝을 넘으면 마디 단위로 늘어남 | 충족 | 시도 3의 늘림 길이 5760, 1회차 시도 13 |
| 5.10 비파괴 트림(숨김 노트는 늘릴 때만 돌아옴) | 충족 | 조옮김·`shift(0)`은 길이를 바꾸지 않는다(1회차 시도 8–10, 시도 4) |
| 결정 개정 「꼬리는 시작만 클립 안이면 되고, 시작이 밖으로 나갈 때만 늘린다」 | 부분 충족(미충족으로 셈) | 늘림 길이 계산은 맞으나 적용이 거부된다(시도 1·3) |
| 결정 개정 「스케일 기준점은 클립마다」 | 충족 | `transform.rs:198-209`, 시도 15 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 이 Task는 `core`의 편집 계산만 바꾼다.
- 제품 코드의 `unwrap`/`expect`: 없음. `unwrap_or(0)`(`transform.rs:209`)은 같은 선택으로 `anchors`를 채운 뒤라 닿지 않는다.
- 산술: `needed_end = new.start() + 1`(`transform.rs:312`)은 `with_start`가 시작 + 길이(≥ 1)를 검사하므로 넘치지 않는다(시도 5). 조옮김은 i64(`transform.rs:138`).
- 에러가 삼켜지는 곳: 없음. 다만 `plan`이 「적용될 수 없는 `Ok`」를 낸다(지적 1).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | 꼬리 판정 시점이 `plan`과 `ReplaceNote`에서 다르다. `plan`은 계산 시점 길이로 「원래 꼬리」를 정해 시작만 덮도록 늘리고, 적용 시점의 `Clip::replace_note`는 `ExtendClip` 뒤의 길이로 `from.end() > self.length`를 다시 본다. 늘린 길이가 원래 꼬리 끝 이상이면 그 노트는 꼬리가 아니게 되고, 옮긴 끝이 늘린 길이를 넘으면 거부된다. 결과: 꼬리를 클립 밖으로 미는 이동(결정 개정이 명시한 경우)과, 다른 노트가 클립을 늘린 선택에서 꼬리를 미는 이동, 꼬리 시작을 클립 밖으로 내보내는 배속이 `Ok` 묶음을 낸 뒤 `apply`에서 `NoteOutsideClip`으로 실패한다(EDT-3 시간 이동·길이 스케일, 결정 개정 미충족, 함수 문서의 「참조가 맞지 않으면 `Err`」 약속 위반). 원자성은 지켜진다. 두 판정을 한 기준으로 맞춰야 한다. 예: 꼬리 노트의 `ReplaceNote`를 `ExtendClip`보다 먼저 낼 수 있는 경우와 없는 경우를 나누거나, `ReplaceNote`가 「꼬리였음」을 적용 전 길이가 아니라 편집이 지닌 값으로 판정하게 하되 「일반 → 꼬리」 거부(시도 6·7)는 유지해야 한다. 수정 뒤 `review_s0_t7_r2.rs`의 실패 4건이 통과해야 한다. | `crates/keyscore-core/src/transform.rs:311-315`, `338-345`, `crates/keyscore-core/src/clip.rs:176-177`, 테스트 `review_s0_t7_r2.rs` 시도 1–3·17 |
| 2 | 수정 | 결정 개정의 두 규칙(꼬리 규칙, 클립별 기준점)에 구현자 테스트가 없다. 1회차 리뷰어 테스트에만 기대고 있어, 지적 1처럼 「꼬리의 시작이 밖으로 나감」이라는 결정 문서의 바로 그 경우가 한 번도 실행되지 않았다. 클립별 기준점 테스트(뒤집은 리뷰어 테스트)도 「클립 시작 기준」 구현을 통과시킨다. 구현자 테스트(단위 또는 `tests/transform.rs`, 가능하면 속성 테스트: 꼬리 포함 임의 선택에 대해 `Ok`면 `apply`도 성공)를 더해야 한다. | `crates/keyscore-core/src/transform.rs:369` 이하, `crates/keyscore-core/tests/transform.rs` |
| 3 | 수정 | 개정 뒤 낡은 문서. `scale` 문서가 아직 「선택의 가장 이른 절대 시작을 기준으로」라고 한다. 보고서 「알려진 한계」가 「지우고 다시 넣기」와 「꼬리를 옮기면 결과 끝까지 늘려 드러난다」를 그대로 적고 있다(둘 다 이제 사실이 아니다). | `crates/keyscore-core/src/transform.rs:177-178`, `docs/reports/s0-t7/report.md:50-51` |
| 4 | 제안 | 공개 `Edit::ReplaceNote`는 기존 꼬리를 꼬리인 채 임의 시작·음높이·길이(u64 끝까지)로 바꿀 수 있다(시도 9). 꼬리 수는 늘지 않고, 개정 결정(꼬리는 꼬리로)과 스케일 규칙(길이도 비율만큼)에서 따라 나오므로 결함으로 보지 않는다. 그러나 `clip.rs`·README의 불변식 문구 「꼬리는 분할로만 생긴다」만으로는 「분할 때의 원래 끝을 넘는 숨김 내용」이 생길 수 있음이 드러나지 않는다. 결정 문서 개정 절이나 불변식 문구에 「꼬리는 분할로만 생기고, 그 노트의 편집으로 모양이 바뀔 수 있다」를 한 줄 적기를 권한다. | `crates/keyscore-core/src/clip.rs:24-25`, `crates/keyscore-core/README.md:26`, `crates/keyscore-core/src/edit.rs:96-98` |

1회차 지적 1–4: 해결(3.1). 1회차 제안 5–7: 반영(결정 개정, 부록, 결정 요청 `2026-10-12-quantize-grid-lengths.md`).

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1(차단)과 2–3(수정)을 고쳐야 한다. `review_s0_t7_r2.rs`의 실패 4건이 통과해야 하고, `review_s0_t7.rs` 18건은 계속 통과해야 한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(17개 중 4개 실패)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(1회차 이후 변경은 `core`의 transform·edit·clip·track, 리뷰어 테스트 1건의 기대값, 계획 문서)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(리뷰어 테스트 1건은 결정 개정에 따라 기대값과 이름이 바뀌었고 정당함, 3.1)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음. `HashMap`은 std)
- [x] 스타일 지적을 적지 않았다

## 자동 게이트(리뷰 테스트 포함)

- `cargo fmt --all -- --check`: 통과
- `cargo clippy --workspace --all-targets -- -D warnings`: 통과
- `cargo test --workspace --no-fail-fast`: 223개 중 `review_s0_t7_r2` 4개 실패(의도한 깨뜨리기), 나머지 219개 통과(`review_s0_t7` 18/18 포함)
- `cargo deny check`: 통과(advisories, bans, licenses, sources ok)
- `scripts/check-deps.sh`: ok, `check-deps.test.sh`: 11/11, `bench-check.test.sh`: 8/8
