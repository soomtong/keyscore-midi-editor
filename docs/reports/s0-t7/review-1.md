# 적대적 리뷰: S0-T7 퀀타이즈·조옮김·길이 스케일 연산 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트). 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: `5ad9592` (`s0/t7-edit-ops`, `main` 대비 5커밋)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`transform.rs`(`#[cfg(test)]` 위), `edit.rs`·`track.rs`·`clip.rs`·`note.rs` 변경분만 읽고 적었다.

1. 퀀타이즈 목표점을 독립 기준 모델(마디 시작부터 모든 그리드 점, 홀수 칸 스윙, 다음 마디 시작 후보, 동점은 앞쪽)과 비교. 셋잇단, 강도, 마디 중간 박자표 변화, 분모 256(그리드가 마디보다 긴 경우) 포함.
2. 짧은 마디(예: 7/16에 1/8 그리드)에서 마지막 홀수 칸의 스윙 점이 마디선을 넘을 때 결과가 다음 마디 시작을 넘지 않는가.
3. 강도 반올림: 음수 이동도 0 쪽(원래 위치 쪽)으로 자르는가.
4. u64 끝 근처 클립에서 `snap`·`bar_aligned_length`·`scale`이 패닉 없이 `Err`를 내는가(`bar_start(bar + 1)`이 `None`인 경우).
5. 조옮김: `pitch + semitones`의 i32 오버플로(i32 끝), 접기 루프의 종료와 반복 횟수, 정확히 12칸 음역, 피치 오프셋 노트.
6. 숨김 꼬리 노트(끝 > 클립 길이)를 고른 선택: `plan`이 `new.end() > clip.length()`만 보므로, 시간을 바꾸지 않는 조옮김도 클립을 늘리려 할 것이다. 분할 직후 오른쪽 클립과 맞닿아 있으면 겹침으로 거부될 것이다.
7. 같은 노트를 두 번 고른 선택, 같은 값의 노트가 여럿인 클립.
8. 여러 트랙·여러 클립에 걸친 선택의 원자성과 undo 한 번.
9. `scale`의 큰 비율: 결과가 u64를 넘을 때 어떤 오류가 나오는가(`relocate`가 `u64::try_from` 실패를 `NoteBeforeClipStart`로 바꾼다).
10. `Edit::ExtendClip`을 다른 공개 편집과 섞고 coalesce로 묶어도 `ShrinkClip` undo가 실패하지 않는가, 불변식(겹침, 노트 시작은 클립 안)이 지켜지는가.
11. 낡은 묶음(`ExtendClip` 성공 뒤 `RemoveNote` 실패)의 원자성.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 독립 기준 모델 비교 | 부분 | `quantize_swing_zero_equals_plain_grid`는 스윙 0·강도 100·박자표 변화 없음·분모 2–16만 본다 |
| 2 짧은 마디 스윙 | 없음 | |
| 3 강도 음수 반올림 | 부분 | 단위 테스트에 강도 50 양수 이동 값만 |
| 4 u64 끝 | 없음 | |
| 5 조옮김 i32 끝·큰 반음 | 없음 | 좁은 음역·오프셋은 단위 테스트에 있음 |
| 6 숨김 꼬리 노트 | 없음 | 계획의 「해석」에만 적혀 있다 |
| 7 중복 선택·같은 값 노트 | 없음 | 보고서 「알려진 한계」에 언급만 |
| 8 여러 트랙·클립 | 없음 | 속성 테스트는 클립 하나 |
| 9 scale 오버플로 오류 종류 | 없음 | |
| 10 ExtendClip 혼합 undo/redo | 부분 | `extend_clip_edit_round_trip_and_rejects_shrink`는 단일 연산 |
| 11 낡은 묶음 원자성 | 있음 | `stale_reference_is_rejected_on_apply` |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-core/tests/review_s0_t7.rs`에서 실행했다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 독립 기준 모델과 비교(스윙 0–75, 셋잇단, 강도 0–100, 박자표 변화 0–2개를 임의 틱에, 분모 1–256, 512사례) | 못 깨뜨림 | `review_quantize_matches_reference_model` |
| 2 | 강도 50%: 961→960(−0.5), 1923→1920(−1.5), 959→960(+0.5) | 못 깨뜨림(961, 1922, 959. 양방향 모두 원래 쪽으로 자름) | `review_quantize_strength_rounds_toward_original_both_directions` |
| 3 | 동점(480 → 0), 7/16·1/8·스윙 75의 마디 끝 | 못 깨뜨림(결과 ≤ 1680) | `review_quantize_tie_and_short_bar_swing` |
| 4 | 클립 시작 `u64::MAX − 5000`, 박자표 4/4·1/256·15/1에서 모든 그리드 퀀타이즈, `shift(i64::MAX/MIN)`, `scale(u32::MAX/1, 1/u32::MAX)` | 못 깨뜨림(패닉 없음) | `review_ops_near_u64_end_do_not_panic` |
| 5 | `transpose(±1000, i32::MIN, i32::MAX − 200, i32::MAX)` | 깨짐: `i32::MAX`에서 `transform.rs:133` 「attempt to add with overflow」 패닉 | `review_transpose_huge_semitones` |
| 6 | 음역 60–71(정확히 12칸), 0–127 전 음높이, −1000..1000(37 간격) | 못 깨뜨림(모두 음역 안, 음이름 보존) | `review_transpose_exact_octave_range_folds_every_pitch` |
| 7 | 음높이 100 + 오프셋 11.5반음, +20 | 못 깨뜨림(108로 접힘, 오프셋 유지) | `review_transpose_offset_note_folds_by_absolute_pitch` |
| 8 | 분할로 꼬리 노트가 남은 왼쪽 클립(오른쪽 클립과 맞닿음)의 노트 전부를 +1 조옮김 | 깨짐: `Err(ClipOverlap { start: 0, end: 7680 })` | `review_transpose_selection_with_hidden_tail_next_to_split_neighbor` |
| 9 | 같은 선택에 `shift(0)`, 강도 0 퀀타이즈 | 깨짐: 움직이지 않는 연산도 `Err` | `review_noop_shift_and_zero_strength_quantize_with_hidden_tail` |
| 10 | 오른쪽 클립을 지운 뒤 같은 조옮김 | 깨짐: 클립 길이가 3840 → 7680으로 늘어 숨김 꼬리가 드러남 | `review_transpose_does_not_resize_clip_with_hidden_tail` |
| 11 | 같은 `NoteRef`를 두 번 넣은 선택(클립엔 한 개) | 깨짐: `shift`는 `Ok(묶음)`을 내고 `apply`가 `Err(NoteNotFound)`. 문서는 그대로 | `review_duplicate_ref_in_selection` |
| 12 | 같은 값의 노트 둘 중 하나만 선택해 이동, undo | 못 깨뜨림 | `review_identical_notes_only_selected_one_moves` |
| 13 | 트랙 2 × 클립 2 선택, +1000 이동(뒤 클립은 늘어남), undo 한 번, −150은 전체 거부 | 못 깨뜨림 | `review_multi_track_multi_clip_selection` |
| 14 | 두 클립에 걸친 선택의 반속(1/2) | 결정대로 `Err(NoteBeforeClipStart)`. 지적 5 참고 | `review_scale_half_across_two_clips_is_rejected` |
| 15 | 길이 1 노트 둘(0, 2^39), `scale(u32::MAX, 1)` | 깨짐: 결과가 u64를 넘는데 `Err(NoteBeforeClipStart)` | `review_scale_overflow_reports_tick_overflow` |
| 16 | `ExtendClip` 뒤 `RemoveNote`가 실패하는 낡은 묶음 | 못 깨뜨림(문서·undo 길이 그대로) | `review_stale_transform_after_extend_is_atomic` |
| 17 | `ExtendClip`·`InsertNote`·`RemoveNote`·`SplitClip`·`MoveClip`을 임의 순서·임의 coalesce 키로 25개까지, 매 단계 불변식, 전부 undo/redo | 못 깨뜨림(256사례) | `review_extend_clip_mixed_with_other_edits_undo_redo` |
| 18 | 늘린 구간에 노트를 넣고 같은 coalesce 키로 묶은 뒤 다시 늘리고 undo/redo | 못 깨뜨림 | `review_extend_then_insert_in_extension_coalesced_undo` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 아니오 | `resize_clip`이 겹침을 검사한다. 시도 17 |
| 노트 시작은 클립 안 | 아니오 | `set_length`가 줄일 때 검사, `ShrinkClip`은 히스토리 역순으로만 온다. 시도 17·18 |
| 숨김 꼬리는 분할로만 생긴다 | 아니오 | `ExtendClip`은 늘리기만, 줄이기는 crate 내부 `Op`. 다만 꼬리를 「드러내는」 경로가 의도와 다르게 생긴다(지적 1) |
| 틱 ≥ 0, u64 안 | 아니오 | 시도 4·15. 오류 종류만 틀림(지적 4) |
| 절대 피치 0 이상 128 미만 | 아니오 | `with_pitch`가 `with_offset`으로 재검사. 시도 7 |
| 결과 묶음의 원자성 | 아니오 | 시도 11·16 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| EDT-2 그리드 1/4–1/64 | 충족 | `transform.rs:21-27`, `56-66` |
| EDT-2 셋잇단 | 충족 | `transform.rs:65`, 시도 1 |
| EDT-2 강도 0–100% | 충족 | `transform.rs:43`, `115-116`, 시도 1·2 |
| EDT-2 스윙 0–75% | 충족 | `transform.rs:240-242`, 시도 1·3 |
| EDT-3 선택 영역에 조옮김 | 부분 충족(미충족으로 셈) | 꼬리 노트가 든 선택은 거부되거나 클립 길이를 바꾼다(`transform.rs:273`, 시도 8·10). `i32` 끝에서 패닉(`transform.rs:133`, 시도 5) |
| EDT-3 선택 영역에 시간 이동 | 부분 충족(미충족으로 셈) | 꼬리 노트가 든 선택은 이동량 0도 거부(시도 9). 중복 선택은 `Ok` 뒤 적용 실패(시도 11) |
| EDT-3 선택 영역에 길이 스케일 | 충족(결정대로) | `transform.rs:176-207`. 여러 클립 반속은 항상 거부(지적 5, 결정의 결과) |
| 5.10 입력이 끝을 넘으면 마디 단위로 늘어남 | 충족 | `transform.rs:285-296`, `311-328` |
| 5.10 줄인 클립의 숨김 노트는 다시 늘릴 때만 돌아온다(비파괴 트림) | 미충족 | 조옮김만으로 클립이 늘어 숨김 노트가 드러난다(시도 10). 계획의 「해석」(옮기면 늘린다)은 결정 문서에 없다 |
| 5.12 음역 밖이면 옥타브를 접는다 | 충족 | `transform.rs:135-150`, 시도 6·7 |
| 2 녹음 후 자동 퀀타이즈(현재 길이 코드 그리드) | 범위 밖(S5)이나 타입이 부족 | 지적 7 |
| 결정 「시작만 옮기고 길이 유지」 | 충족 | `transform.rs:117` |
| 결정 「곡의 마디 기준, 마디 중간 박자표 변화 반영」 | 충족 | 시도 1(임의 틱 박자표 변화) |
| 결정 「끝을 넘으면 늘림, 겹치면 전체 거부, 앞은 거부」 | 충족. 단 「넘는다」의 판정이 꼬리 노트에서 어긋남(지적 1) | `transform.rs:273`, `290-294`, `225` |
| 결정 「가장 이른 시작 기준 스케일」 | 충족 | `transform.rs:191-198` |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 이 Task는 `core`의 편집 계산만 바꾼다.
- 제품 코드의 `unwrap`/`expect`: 없음. `unwrap_or(u8::MAX)`(`transform.rs:144`, 범위 검사 뒤라 닿지 않음), `unwrap_or(abs)`(`transform.rs:257`, 후보가 비지 않음)는 패닉하지 않는다.
- 산술 오버플로: `transform.rs:133` `i32::from(pitch) + semitones`가 넘친다(지적 2). 그 밖의 덧셈·뺄셈(`112`, `194`, `197-198`, `237`, `289`, `327`)은 클립 불변식(시작 + 범위가 u64 안)과 `bar_start ≤ abs`로 안전함을 확인했다. `ratio`는 u128에서 최대 약 2^97이라 넘치지 않는다.
- 에러가 바뀌어 원인이 숨는 곳: `transform.rs:225` `u64::try_from` 실패(u64 초과)를 `NoteBeforeClipStart`로 바꾼다(지적 4).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | `plan`이 「결과 끝 > 클립 길이」만으로 클립을 늘린다. 숨김 꼬리 노트는 원래부터 끝이 길이를 넘으므로, 시간을 바꾸지 않는 조옮김이나 이동량 0·강도 0 연산도 클립을 늘리려 한다. 분할 직후처럼 오른쪽 클립이 맞닿아 있으면 `ClipOverlap`으로 연산 전체가 거부되고(EDT-3 미충족), 이웃이 없으면 조옮김만으로 클립이 3840 → 7680으로 늘어 숨김 노트가 드러난다(REQ 5.10 비파괴 트림과 어긋남). 최소한 조옮김은 클립 길이를 바꾸지 않아야 하고, 시간 연산도 「원래 끝 이하로 남는 꼬리」는 늘림 대상이 아니어야 한다. 꼬리 노트를 옮겼을 때 클립을 늘릴지(계획의 「해석」)는 REQ 5.10과 결정 문서 어디에도 없으므로 `docs/decisions/`에 결정 요청을 써야 한다. | `crates/keyscore-core/src/transform.rs:273`, 테스트 `review_s0_t7.rs:349`, `363`, `373` |
| 2 | 수정 | `transpose`의 `i32::from(pitch) + semitones`가 `semitones`가 `i32::MAX − 127` 근처면 넘친다. debug는 패닉, release는 감싸서 음이름이 틀린 결과(2^32 mod 12 = 4반음 어긋남)를 `Ok`로 낸다. 접기 루프도 반복 횟수가 `|semitones| / 12`에 비례한다(i32 끝에서 약 1.8억 회). i64 산술과 `rem_euclid`로 한 번에 접어야 한다. | `crates/keyscore-core/src/transform.rs:133-140`, 테스트 `review_s0_t7.rs:277` |
| 3 | 수정 | 같은 `NoteRef`를 두 번 고른 선택에 연산 함수가 `Ok(묶음)`을 내고, 그 묶음은 `apply`에서 `NoteNotFound`로 실패한다. 함수 문서는 「참조가 맞지 않으면 `Err`」라고 약속한다. 중복을 「클립에 있는 같은 값 노트 수」와 비교해 연산 단계에서 거부하거나(전용 오류), 중복을 하나로 합치는 규칙을 정해야 한다. | `crates/keyscore-core/src/transform.rs:210-220`, `270-283`, 테스트 `review_s0_t7.rs:395` |
| 4 | 수정 | `relocate`가 `u64::try_from` 실패를 모두 `NoteBeforeClipStart`로 바꾼다. `scale(u32::MAX, 1)`처럼 결과가 u64를 넘을 때도 「클립 시작 앞」이라고 알린다. 음수면 `NoteBeforeClipStart`, u64 초과면 `TickOverflow`로 나눠야 한다. | `crates/keyscore-core/src/transform.rs:224-225`, 테스트 `review_s0_t7.rs:493` |
| 5 | 제안 | 결정의 두 규칙(선택의 가장 이른 시작 기준, 클립 시작 앞은 거부)이 겹쳐, 두 클립 이상에 걸친 선택의 반속은 언제나 거부된다. 결정대로이므로 결함은 아니지만, 사람이 이 결과를 알고 결정했는지 결정 문서에 한 줄 확인을 받기를 권한다(예: 클립마다 기준점을 따로 둘지). | `crates/keyscore-core/src/transform.rs:191-198`, 테스트 `review_s0_t7.rs:472` |
| 6 | 제안 | 「해석」 절의 스윙 정의(홀수 칸을 `그리드 × 스윙%` 뒤로), 동점은 앞쪽, 마디 끝 처리, 강도의 0 쪽 자르기는 REQ에 식이 없는 결정이다. 구현과 기준 모델은 일치했으나(시도 1), 이것들이 결정 문서가 아니라 계획 파일에만 있다. REQ의 의미를 정하는 내용이므로 결정 문서에 옮기기를 권한다. | `docs/reports/s0-t7/plan.md` 「해석」 |
| 7 | 제안 | REQ 2·5.1(녹음 후 자동 퀀타이즈)과 5.12(AI 응답)는 「현재 길이 코드 그리드」를 쓰는데, 길이 코드는 온음표·2분음표·점음표를 포함한다(5.6). `Grid`는 EDT-2의 1/4–1/64만 표현한다. S5에서 막히지 않도록 결정 요청(EDT-2 범위와 길이 코드 그리드의 관계)을 미리 써 두기를 권한다. | `crates/keyscore-core/src/transform.rs:21-27`, `56-66` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1(차단)과 2–4(수정)를 고치고, 지적 1의 「옮긴 꼬리 노트에 대한 클립 늘림」은 결정 요청으로 사람의 결정을 받아야 한다. `review_s0_t7.rs`의 실패 6건이 통과해야 한다(지적 1의 시간 연산 쪽 기대값은 결정에 따라 조정할 수 있다).

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(18개 중 6개 실패)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`Pitch::MIN`·`MAX` 공개 상수는 보고서에 적힘)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 자동 게이트(리뷰 테스트 포함)

- `cargo fmt --all -- --check`: 통과
- `cargo clippy --workspace --all-targets -- -D warnings`: 통과
- `cargo test --workspace --no-fail-fast`: `review_s0_t7` 18개 중 6개 실패(의도한 깨뜨리기), 나머지 전부 통과
- `cargo deny check`: 통과
- `scripts/check-deps.sh`: ok, `check-deps.test.sh`: 11/11, `bench-check.test.sh`: 8/8
