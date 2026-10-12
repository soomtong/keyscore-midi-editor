# 적대적 리뷰: S0-T7 퀀타이즈·조옮김·길이 스케일 연산 (리뷰 3회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트). 구현 컨텍스트를 물려받지 않았다.
- 대상 커밋: `26eb908` (`s0/t7-edit-ops`). 2회차(`1c6450d`) 이후 제품 코드 커밋 `0f1c877`, `4cb3226`, `a21bc43`, 테스트 커밋 `44b68f8`, `53a681a`, `4703e62`, 계획 커밋 `c845983`, `26eb908`
- 기준: `docs/reports/s0-t7/review-2.md`, `docs/decisions/2026-10-12-quantize-and-transform-rules.md`(개정·부록), `docs/reports/s0-t7/report.md` 「리뷰 대응 (review-2)」
- 재리뷰 횟수: 이번이 두 번째 재리뷰다. 프로토콜 [8]에 따라 다음 재리뷰(세 번째)에서도 판정이 나지 않으면 결정 요청으로 전환한다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

2회차 이후 `edit.rs`의 `ReplaceNotes`/`RestoreNotes`, `clip.rs`·`track.rs`의 `replace_notes`, `transform.rs`의 `plan` 변경분만 읽고 적었다.

1. `Clip::replace_notes`의 꼬리 허용은 `from.end() > before`(편집 직전 길이)로 판정하는데, `from`은 작업 사본 `notes`에서 찾는다. 앞 pair가 막 넣은 `to`를 뒤 pair의 `from`으로 쓰면, 그 값은 「원래 노트」가 아닌데도 끝이 `before`를 넘기만 하면 꼬리로 판정된다. 늘리는 편집(`length > before`)이면 일반 노트를 `(before, length]` 구간에 걸치게 옮긴 뒤 그 값을 다시 `from`으로 써서 새 길이를 넘는 꼬리를 만들 수 있을 것이다. pair 순서에 따라 결과가 달라지는가.
2. `plan`이 낸 묶음은 언제나 적용되는가. 꼬리 판정(계산 시점 `clip.length()`)과 적용 시점 판정(`before`)이 같은 상태를 보므로 맞아야 한다. 특히 선택 노트의 새 값이 다른 선택 노트의 원래 값과 같아 pair가 사실상 사슬이 될 때.
3. 일괄 연산이 이제 클립마다 편집 하나를 낸다. 1회차 리뷰어 테스트가 보던 「편집 사이의 원자성」(앞 편집 성공, 뒤 편집 실패)은 두 클립 이상에 걸친 묶음에서만 생긴다. 그 경우가 시험되는가.
4. `ReplaceNotes` 길이 규칙: 줄이기·0 거부(restoring 제외), 이웃까지 맞닿게 늘리기 허용·1틱 넘기 거부, `start + length` u64 초과, 꼬리 끝 u64 초과(`start.checked_add(to.end())`).
5. `to.start ≥ length` 거부, 꼬리가 아닌 노트의 끝이 새 길이를 넘으면 거부, 모르는 트랙·클립·다른 클립의 노트.
6. 같은 값 노트가 여럿일 때 같은 `from`을 여러 번: 개수까지 허용, 초과는 `NoteNotFound`, 실패하면 사본을 버려 상태 불변.
7. `RestoreNotes`의 역: pair를 뒤집고 순서도 뒤집는다. 사슬(`A→B`, `B→C`)과 같은 값 노트에서 undo/redo가 정확히 왕복하는가. `ExtendClip`·`ReplaceNote`·`InsertNote`와 섞인 히스토리, 여러 단위 undo/redo 반복.
8. 빈 `pairs` + 같은 길이: 히스토리에 무변화 단위가 쌓이는가(상태는 그대로여야 한다).
9. 늘린 길이는 마디 경계인가(마디 중간 박자표 변화 포함). `plan`의 겹침 검사가 늘릴 때만 도는가(`length > clip.length()`).
10. `heap_bytes`: `ReplaceNotes`/`RestoreNotes`의 pair 메모리를 센다.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 pair 사슬로 꼬리 만들기 | 없음 | `edit.rs` `replace_notes_judges_tails_by_length_before_the_edit`는 pair 하나뿐 |
| 2 `Ok` 묶음은 언제나 적용 | 부분 | 단위 테스트 3개(`transform.rs`)와 리뷰어 테스트뿐. 2회차가 권한 속성 테스트는 더해지지 않았다 |
| 3 편집 사이 원자성(두 클립) | 없음 | 1회차 리뷰어 테스트는 이제 한 편집 안의 원자성만 본다 |
| 4 길이 규칙 | 부분 | 단위 테스트에 줄이기·이웃 겹침. u64 끝·맞닿음 허용은 없음 |
| 5 시작 밖·일반 노트 넘침 | 부분 | 일반 노트 넘침만 |
| 6 같은 값 `from` 여러 번 | 없음 | |
| 7 undo/redo·섞인 히스토리 | 부분 | 단위 테스트 `round_trip`이 한 편집의 왕복만 |
| 8 빈 pairs | 없음 | |
| 9 마디 경계·박자표 중간 변화 | 부분 | 2회차 리뷰어 3/4 테스트 |
| 10 메모리 계산 | 없음 | 범위에 비해 사소해 시도하지 않았다 |

## 3. 깨뜨리기 시도

### 3.1 2회차 지적 해결 확인 (수정을 되돌린 /tmp 사본에서)

`git archive HEAD`로 `/tmp/ks-r3`에 풀고, `plan`의 편집 생성부만 2회차 시점(`ExtendClip` + `ReplaceNote`)으로 되돌렸다. 저장소 작업 트리는 건드리지 않았다(사본은 지웠다).

| 지적 | 되돌린 것 | 결과 |
| --- | --- | --- |
| 1 꼬리 판정 시점(차단) | `plan` 끝의 `ReplaceNotes` 생성을 `ExtendClip` + `ReplaceNote`로 | `review_s0_t7_r2` 4개 실패(시도 1–3·17 그대로), 구현자 `tail_start_beyond_extended_length_still_applies` 실패, 이번 회차 속성 테스트 2개 실패(`NoteOutsideClip { start: 4303, end: 6048, clip_length: 6047 }` 등) |
| 2 구현자 테스트 누락 | 같은 사본 | 위와 같이 구현자 테스트가 결함을 잡는다. 클립별 기준점 테스트는 첫 선택 노트가 480·1000이라 「클립 시작 기준」과 구별된다(코드로 확인) |
| 3 낡은 문서 | 읽어서 확인 | `scale` 문서와 보고서 「알려진 한계」는 고쳐졌다. 다만 보고서 「구현 요약」 11행과 「리뷰어에게」 58행이 아직 `ExtendClip`이 먼저 오는 옛 묶음 모양을 적는다(지적 2) |
| 4 꼬리 모양(제안) | 읽어서 확인 | `clip.rs` 문서와 README에 반영 |

HEAD에서 `review_s0_t7` 18개, `review_s0_t7_r2` 17개 모두 통과한다.

1회차 리뷰어 테스트 `review_stale_transform_after_extend_is_atomic`의 단언 변경은 정당하다. 원래 단언(`edits[0]`이 `ExtendClip`)은 결함이 아니라 당시 묶음 모양을 적은 것이고, 테스트의 목적(늘리기를 포함한 낡은 묶음이 원자적으로 실패)은 바뀐 단언으로도 그대로 확인된다. 다만 그 테스트는 이제 편집 하나 안의 원자성(`Clip::replace_notes`의 사본)만 본다. 원래 보던 「앞 편집 성공, 뒤 편집 실패」는 두 클립 묶음으로 옮겨 보강했다(시도 9, 통과).

### 3.2 3회차 시도

모두 `crates/keyscore-core/tests/review_s0_t7_r3.rs`에서 실행했다(공개 API만).

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 클립(0..3840, 꼬리 없음)의 일반 노트 (0, 100)에 `ReplaceNotes { length: 7680, pairs: [(plain, (3800, 1000)), ((3800, 1000), (3800, 100000))] }`. 첫 pair는 끝 4800 ≤ 7680이라 일반 노트로 허용, 둘째 pair의 from은 끝 4800 > before 3840이라 「원래 꼬리」로 판정 | 깨짐: `Ok(())`, 길이 7680인 클립에 끝 103800의 꼬리가 생긴다. 분할 없이 꼬리 수가 0 → 1 | `review_r3_replace_notes_chain_turns_plain_note_into_tail` |
| 2 | 같은 구멍을 최소 늘림으로: length 3841, `(a, (3839, 2))`, `((3839, 2), (3839, u32::MAX))`. 순서를 뒤집은 묶음은 `NoteNotFound` | 깨짐: 정순은 `Ok`, 길이 3841 클립에 길이 u32::MAX 꼬리. 같은 pair 집합이 순서에 따라 거부·허용으로 갈린다 | `review_r3_replace_notes_chain_order_matters` |
| 3 | 일반 노트를 새 길이 밖으로(끝 7700 > 7680), 시작 = 새 길이(7680), 시작 = 같은 길이(3840) | 못 깨뜨림(`NoteOutsideClip`, 상태·undo 길이 불변) | `review_r3_replace_notes_rejects_plain_overhang_and_outside_start` |
| 4 | 길이 3839(줄이기), 0, 7681(이웃 7680을 1틱 침범), 7680(맞닿음), undo. 시작 u64::MAX − 3840 클립을 3841로, 꼬리 끝이 u64를 넘게 | 못 깨뜨림(앞 셋 거부·맞닿음 허용·undo 복원, u64 둘 다 거부) | `review_r3_replace_notes_length_rules` |
| 5 | 같은 값 셋인 클립에서 같은 from 네 번(거부), 세 번 + 늘리기(허용), undo/redo 3회 반복 | 못 깨뜨림 | `review_r3_replace_notes_duplicate_from_and_round_trip` |
| 6 | 빈 pairs, 같은 길이 | 못 깨뜨림(상태 불변, undo 한 단위가 쌓이지만 되돌려도 같다) | `review_r3_replace_notes_empty_is_noop` |
| 7 | 모르는 트랙, 같은 트랙의 다른 클립 id로 이 클립의 노트를 from | 못 깨뜨림(`Err`, 상태 불변) | `review_r3_replace_notes_rejects_unknown_targets` |
| 8 | `ExtendClip` → `ReplaceNotes`(늘림 + 사슬 `a→x`, `b→a`) → `ReplaceNote` → `InsertNote` + `ReplaceNotes` 한 묶음. 끝까지 undo, 끝까지 redo를 두 번 | 못 깨뜨림(단계마다 상태 일치) | `review_r3_mixed_history_round_trips` |
| 9 | 두 클립 선택의 `shift(+500)`(첫 클립은 늘림). 둘째 클립 노트를 먼저 지워 낡게 한 뒤 적용 | 못 깨뜨림(`NoteNotFound`, 첫 클립 길이·undo 길이 불변) | `review_r3_stale_batch_across_two_clips_is_atomic` |
| 10 | 속성 2000건: 트랙 1–2개, 분할(오른쪽 유지 또는 삭제)로 만든 꼬리, 맞닿거나 떨어진 이웃 클립, 마디 중간 5/8 박자표 변화, 임의 선택, 네 연산(퀀타이즈 전 설정, 조옮김 ±300, 이동 ±9000·0·+20000, 배율 1–4/1–4). `Ok`면 적용 성공, 꼬리 수 비증가, 노트 시작은 클립 안, 늘린 끝은 마디 경계, 트랙 안 클립 겹침 없음, undo/redo 왕복 | 못 깨뜨림. 2000건 중 약 1770건이 `Ok`(꼬리 있는 경우 약 800건, 클립이 늘어난 경우 약 80건) | `review_r3_transform_ok_batches_always_apply` |
| 11 | 10과 같은 성질을 늘림에 집중해 2000건: 이동 +1..20000, 배율 2–5/1–2 | 못 깨뜨림. 늘어난 경우 약 500건(꼬리 포함 약 270건, 박자표 변화 포함 약 270건) | `review_r3_growing_transforms_always_apply` |

11개 중 2개 실패(시도 1·2, 원인 하나). 시도 10·11은 3.1의 되돌린 사본에서 실패하므로 2회차 결함을 잡는 힘이 있다.

`plan`이 내는 묶음은 이 구멍에 닿지 않는다. `plan`의 from은 모두 선택 시점의 원래 값이고, 사슬이 생기는 경우(새 값이 다른 선택 노트의 원래 값과 같음)에도 두 값이 같아 끝도 같으므로 꼬리 판정이 원래 노트의 판정과 일치한다(시도 10·11). 구멍은 공개 `Edit::ReplaceNotes`를 직접 만드는 호출자에게만 열려 있다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 아니오 | `Track::replace_notes`가 늘릴 때 겹침 검사. 시도 4·10·11 |
| 노트 시작은 클립 안 | 아니오 | pair마다 `to.start < length`, 마지막 전체 검사. 시도 3·10 |
| 꼬리의 개수는 분할로만 는다(README, `clip.rs` 문서, 결정 `2026-10-12-clip-split-and-velocity.md` Q1) | 예 | `Edit::ReplaceNotes`의 pair 사슬(시도 1·2, 지적 1) |
| 꼬리 포함 절대 끝이 u64 안 | 아니오 | `start.checked_add(to.end())`. 시도 4 |
| 실패한 묶음은 원자적 | 아니오 | 시도 3·5·7·9 |
| 연산이 `Ok`로 낸 묶음은 적용된다 | 아니오 | 시도 10·11 |
| undo/redo 왕복 | 아니오 | 시도 5·8·10·11 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| EDT-2 그리드·셋잇단·강도·스윙 | 충족 | 1회차 시도 1–3, 시도 10(퀀타이즈 전 설정이 적용됨) |
| EDT-3 선택 영역에 조옮김 | 충족 | 시도 10 |
| EDT-3 선택 영역에 시간 이동 | 충족 | 2회차 결함 해결(3.1), 시도 10·11 |
| EDT-3 선택 영역에 길이 스케일 | 충족 | 클립별 기준점(`transform.rs` 단위 테스트), 시도 10·11 |
| 5.10 끝을 넘으면 마디 단위로 늘어남 | 충족 | 시도 10·11의 마디 경계 검사(마디 중간 박자표 변화 포함) |
| 5.10 비파괴 트림(숨김 노트는 늘릴 때만 돌아옴) | 충족 | 조옮김·`shift(0)`은 길이 불변(구현자 `tail_note_keeps_clip_length_when_start_stays_inside`) |
| 결정 개정 「꼬리는 시작만 클립 안이면 되고, 시작이 밖으로 나갈 때만 늘린다」 | 충족 | `transform.rs:311-316`, `review_s0_t7_r2` 시도 1–3·17 통과 |
| 결정 개정 「스케일 기준점은 클립마다」 | 충족 | `scale_anchors_each_clip_at_its_first_selected_note` |
| 결정 Q1(clip-split) 「꼬리는 분할로만 생긴다」 | 부분 충족(미충족으로 셈) | 연산 경로는 지키나 공개 `Edit::ReplaceNotes`로 깨진다(`clip.rs:218`, 시도 1·2) |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 이 Task는 `core`의 편집 계산만 바꾼다.
- 제품 코드의 `unwrap`/`expect`: 이번 변경분에 없음.
- 산술: `Track::replace_notes`가 `start.checked_add(length)`로 겹침 검사 전에 넘침을 거부하고, `plan`의 `clip.start().get() + length`는 `bar_aligned_length`가 `checked_add`와 `bar_start`의 `None`으로 막은 값이다.
- 에러가 삼켜지는 곳: 없음.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | `Clip::replace_notes`의 꼬리 허용 판정 `from.end() > before`가 「원래 노트였는가」가 아니라 「지금 찾은 값의 끝」을 본다. `from`을 앞 pair가 넣은 `to`에서도 찾으므로, 늘리는 편집에서 일반 노트를 `(before, length]`에 걸치게 옮긴 뒤 그 값을 다음 pair의 from으로 쓰면 새 길이를 넘는 꼬리가 된다(시도 1: 0 → 1개, 시도 2: 길이 u32::MAX 꼬리). 공개 API로 「꼬리의 개수는 분할로만 는다」(README·`clip.rs` 문서, 결정 Q1)를 깨므로 프로토콜 [5]-3에 따라 차단이다. 같은 pair 집합이 순서에 따라 허용·거부로 갈리는 것도 같은 원인이다. 일괄 연산 경로는 닿지 않는다(3.2 끝). 고치는 방법은 구현자 판단이나, 예: 모든 from을 원래 노트 목록에서 먼저 찾아 지우면서 꼬리 여부를 원래 값으로 기록한 뒤 to를 넣는다(동시 치환 의미. `plan`의 from은 모두 원래 값이고 개수 검사도 이미 하므로 그대로 맞는다. 역편집도 to가 모두 존재하므로 맞는다). 또는 꼬리 허용을 「원래 목록에 있던 노트의 자리」에만 붙여 사슬로 옮겨 가게 한다. 수정 뒤 `review_s0_t7_r3.rs`의 실패 2건이 통과하고, 속성 테스트 2건은 계속 통과해야 한다. `Edit::ReplaceNotes` 문서에 pair 의미(동시 치환인지 차례 치환인지)를 적기를 권한다. | `crates/keyscore-core/src/clip.rs:212-218`, `crates/keyscore-core/src/edit.rs:105-114`, 테스트 `review_s0_t7_r3.rs` 시도 1·2 |
| 2 | 수정 | 보고서에 옛 묶음 모양이 남아 있다. 「구현 요약」이 묶음을 `ExtendClip…`, `RemoveNote…`, `InsertNote…` 순서라 하고, 「리뷰어에게」가 「`ExtendClip`이 먼저 오고, 그 뒤 노트 삭제·삽입」이라 한다. 지금은 클립마다 `ReplaceNotes` 하나다(2회차 지적 3과 같은 종류). | `docs/reports/s0-t7/report.md:11`, `docs/reports/s0-t7/report.md:58` |
| 3 | 제안 | 2회차 지적 2가 권한 「`Ok`면 적용 성공」 속성 테스트가 구현자 테스트에 없다. 이번 회차 `review_r3_transform_ok_batches_always_apply`·`review_r3_growing_transforms_always_apply`가 그 역할을 하며 2회차 결함을 잡는 것을 확인했다(3.1). 이를 그대로 두는 것으로 충분하다고 보지만, 구현자 테스트(`tests/transform.rs`)로 옮길지는 구현자가 정한다. | `crates/keyscore-core/tests/transform.rs` |

2회차 지적 1(차단): 해결(3.1). 지적 2: 해결(구현자 단위 테스트 3개가 되돌린 사본에서 실패함을 확인). 지적 3: 대부분 해결, 남은 두 줄은 지적 2. 제안 4: 반영.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1(차단)과 2(수정)를 고쳐야 한다. 수정은 `Clip::replace_notes`의 꼬리 판정 한 곳과 보고서 두 줄로 국한된다. `review_s0_t7_r3.rs`의 실패 2건이 통과하고, `review_s0_t7`(18), `review_s0_t7_r2`(17), `review_s0_t7_r3`의 나머지 9건이 계속 통과해야 한다. 다음 회차는 세 번째(마지막) 재리뷰다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(11개 중 2개 실패)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(2회차 이후 변경은 `core`의 clip·track·edit·transform, README 한 줄, 리뷰어 테스트 1건의 단언, 계획·보고 문서)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(리뷰어 테스트 1건의 단언 변경은 정당함, 3.1)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다

## 자동 게이트(리뷰 테스트 포함)

- `cargo fmt --all -- --check`: 통과
- `cargo clippy --workspace --all-targets -- -D warnings`: 통과
- `cargo test --workspace --no-fail-fast`: 238개 중 `review_s0_t7_r3` 2개 실패(의도한 깨뜨리기), 나머지 236개 통과(`review_s0_t7` 18/18, `review_s0_t7_r2` 17/17 포함)
- `cargo deny check`: 통과(advisories, bans, licenses, sources ok)
- `scripts/check-deps.sh`: ok, `check-deps.test.sh`: 11/11, `bench-check.test.sh`: 8/8
