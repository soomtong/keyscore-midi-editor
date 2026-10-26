# 적대적 리뷰: S3-T3b 노트 편집 (리뷰 3회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰어로 새로 띄운 에이전트, 구현 컨텍스트 없음)
- 대상 커밋: `5a1e290`(브랜치 `s3/t3b-note-edit`). 범위는 좁은 확인: 리뷰 2 대응 `git diff d217889 HEAD`(`note_ops.rs` 레가토 알림, 결정 문서 「후속」 해석 줄, 보고서 「리뷰 2 대응」)와 전제가 바뀐 기존 시험을 고친 `834ea4a`
- 새 시험: `crates/keyscore-app/tests/review_s3_t3b_r3.rs`(7개, 모두 통과). 실행: `CARGO_TARGET_DIR=target/review-t3b cargo test -p keyscore-app --test review_s3_t3b_r3 --test review_s3_t3b_r2 --test review_s2_t2_r2 --test review_s3_t3a_r2 --test note_ops`(7 + 11 + 23 + 9 + 15 통과)

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. `834ea4a`의 `review_s3_t3a_r2` 변경: `y y p`가 vim에서 정말 `edit::Repeat` 하나로 가는가, 아니면 `y y`(복사) 뒤 `p`(붙여넣기)로 갈라져 알림만 우연히 맞는가. 알림만 보는 단언은 이 차이를 못 가린다.
2. 같은 변경의 전제: vim `r`이 이제 반복으로 넘어가지 않는가(REQ 5.8.1 `r` = 음높이 바꾸기, 충돌 메모의 「`r`(반복) → `yyp`」).
3. `834ea4a`의 `review_s2_t2_r2` 변경: `checked > 0` 단언을 빼면 `check_shifted` 두 시험은 빈 검사로 늘 통과한다. B25 a′(「`shift-N` 바인딩은 지운다」)를 지키는 다른 시험이 있는가. 기호 그대로 쓴 바인딩(`#` `!`)이 macOS 입력에 닿는지는 다른 시험이 보는가.
4. 레가토 알림의 `has_tail`: 꼬리가 없을 때 꼬리 문구가 새지 않는가. 끝이 클립 끝과 딱 맞는 노트를 꼬리로 보지 않는가(`>` 대 `>=`).
5. 꼬리 + 이미 붙은 일반 노트(바뀌는 노트 없음)에서 알림이 꼬리를 말하고 문서·undo를 건드리지 않는가.
6. 결정 문서 「후속」의 해석 줄이 2-A의 의미를 바꾸는지, REQ를 고치지 않았는지.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. `y y p`가 반복 하나인가 | 부분 | 고친 시험은 알림만 본다 |
| 2. vim `r`이 반복이 아님 | 없음 | 커밋 메시지의 설명뿐 |
| 3. `shift-N` 바인딩이 없음(B25 a′) | 없음 | `check_shifted`는 이제 빈 검사. 기호 그대로 입력은 `review_s2_t2.rs:615`, `note_ops.rs:311`이 본다 |
| 4. 꼬리 없을 때 알림 그대로 | 없음 | |
| 5. 꼬리 + 붙은 노트 | 없음 | 꼬리 단독은 `review_s3_t3b_r2::legato_on_tail_only_explains_why` |
| 6. 결정 기록 | 해당 없음 | 문서 대조 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | vim에서 `p`로 새 클립에 사본을 둔 뒤 `y y p`. 알림과 함께 클립·undo가 그대로인지 | 못 깨뜨림. 거부 알림, 클립·undo 그대로(복사+붙여넣기로 갈라지지 않았다) | `vim_yyp_after_p_is_rejected_repeat_not_copy_and_paste` |
| 2 | vim에서 C4를 고르고 `y y p` | 못 깨뜨림. 바로 뒤(960)에 한 번만 복제, undo 하나, 사본이 선택 | `vim_yyp_duplicates_selection_once` |
| 3 | 같은 자리에서 처음 시험처럼 vim `r` | 못 깨뜨림. 반복 거부 알림이 나오지 않고(`SetPitch`), `escape` 뒤 문서 그대로. 시험 변경의 전제가 맞다 | `vim_r_after_p_no_longer_reaches_repeat` |
| 4 | 두 프리셋의 모든 레이어에서 `shift-<숫자·기호>` 타(시퀀스 안 포함)를 찾는다 | 못 깨뜨림. 하나도 없다 | `no_preset_binds_shift_digit_or_symbol` |
| 5 | 꼬리 없이 이미 붙은 노트에 `~` | 못 깨뜨림. 「바뀌는 노트가 없다」만, 꼬리 문구 없음, undo 0 | `legato_nothing_without_tail_keeps_plain_message` |
| 6 | 끝이 클립 끝(7680)과 딱 맞는 노트에 `~` | 못 깨뜨림. 꼬리 문구 없음 | `legato_note_ending_exactly_at_clip_end_is_not_tail` |
| 7 | 이미 꼬리에 붙은 일반 노트 + 꼬리를 골라 `~` | 못 깨뜨림. 「바뀌는 노트가 없다(… 숨김 꼬리 …)」, 클립·undo 그대로 | `legato_tail_plus_already_legato_note_explains_and_changes_nothing` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 노트는 클립 안(숨김 꼬리는 분할로만) | 아니오 | 이번 변경은 알림 문자열만 바꾼다(`note_ops.rs:273-289`) |
| 키맵에 `shift-N` 표기 없음(B25 a′) | 아니오 | 시도 4 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.8.1 vim `r` = 음높이 바꾸기, 반복은 `yyp` | 충족 | `assets/keymaps/vim.json:42-44`, `docs/REQ.md:334`·`:371`, 시도 1–3 |
| 7.3 KM-7 기호·`Shift+숫자`는 입력 글자 그대로 | 충족 | `docs/REQ.md:835`, `assets/keymaps/default.json:124-136`, 시도 4, `review_s2_t2.rs:615` |
| 5.8 `~` 레가토(결정 2-A와 「후속」 해석) | 충족 | `docs/decisions/2026-10-26-s3-t3b-note-edit-details.md:37`, 시도 5–7 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(앱 UI 경로만 바뀜)
- 제품 코드의 `unwrap`/`expect`: 새로 생긴 것 없음(`has_tail`은 `and_then`·`is_some_and`)
- 에러가 삼켜지는 곳: 없음

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | `check_shifted`에서 `checked > 0`을 뺀 뒤 두 시험(`shifted_symbol_bindings_match_us_keystrokes_*`)은 지금 빈 검사로 늘 통과한다. 원래 의도(「`shift-N` 바인딩은 macOS 입력에서도 닿는다」)는 바인딩이 다시 생기면 살아나므로 약화는 아니다. 다만 B25 a′ 자체를 지키는 시험이 없었다. `let _ = checked;` 대신 `assert_eq!(checked, 0, …)`로 B25를 단언하거나, 이 리뷰의 `no_preset_binds_shift_digit_or_symbol`처럼 프리셋에 `shift-N`이 없음을 직접 보는 편이 의도가 드러난다 | `crates/keyscore-app/tests/review_s2_t2_r2.rs:474-476` |
| 2 | 제안 | `review_s3_t3a_r2`의 고친 시험은 알림만 단언한다. `y y p`가 복사+붙여넣기로 갈라지면 붙여넣기 알림이 마지막이 되어 잡히긴 하지만, 클립 수·undo가 그대로인지도 단언하면 더 단단하다(시도 1이 그 단언을 갖는다) | `crates/keyscore-app/tests/review_s3_t3a_r2.rs:336-344` |

`834ea4a`의 두 변경은 회귀나 결정 위반이 아니다. vim `r`은 REQ 5.8.1과 `vim.json`대로 `edit::SetPitch`이고 반복은 `y y p`이므로 `r`을 누르던 처음 시험이 오히려 처리기 없는 우연에 기대고 있었다(시도 3). `shift-N` 단언 제거는 B25 a′가 그 바인딩을 지우라고 정한 결과다.

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
