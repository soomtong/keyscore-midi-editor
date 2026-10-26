# 적대적 리뷰: S3-T3b 노트 편집 (리뷰 2회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰어로 새로 띄운 에이전트, 구현 컨텍스트 없음)
- 대상 커밋: `3da98df`(브랜치 `s3/t3b-note-edit`). 범위는 리뷰 1 대응 `git diff bfc95b4 HEAD`(`f2a9e0d` `0a32744` `149dcc8` `0d98c73`)와 그 주변
- 새 시험: `crates/keyscore-app/tests/review_s3_t3b_r2.rs`(11개, 1개 실패). 실행: `CARGO_TARGET_DIR=target/review-t3b cargo test -p keyscore-app --test review_s3_t3b_r2 --test review_s3_t3b --test note_ops --test note_ops_pure --test keymap_engine --test count_non_key_paths`

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. `.`의 `defer_in` 묶음 끊기가 실제로 다시 보낸 Action 뒤에 도는가(GPUI `Window::dispatch_action`과 `defer_in`이 같은 `App::defer` 큐를 쓰는가). `cmd-d`뿐 아니라 `r`(`edit::Repeat`)도.
2. 끊기를 더한 것이 묶음을 너무 끊지 않는가: `.` 뒤 연속 `cmd-d cmd-d`는 여전히 한 단위인가. `cmd-d . .`은 세 단위인가.
3. vim `0` = `operator::Count(0)`: 맨 `0`은 문서·커서·카운트를 바꾸지 않고, `1 0`은 10, `1 0 0`은 100, `0 3`은 3인가. 이동 키 뒤 카운트가 남지 않는가.
4. `g 0` 시퀀스가 깊은 컨텍스트(`Normal > PianoRoll`)의 단독 `0`에 가려지지 않는가. 카운트 뒤(`2 g 0`)에도 길이 기본값이고 카운트가 남지 않는가. 배치 뷰의 맨 `0`.
5. 기본 프리셋 `0`(길이 기본값)이 그대로인가. vim `Step` 컨텍스트의 `0`(`step::ResetLength`)과 충돌하지 않는가(`Step`은 `Normal`의 형제 컨텍스트).
6. 넓힌 `selection_after`: 같은 값 셋 합치기, 같은 값 둘 분할, 퀀타이즈로 대상 둘이 같은 값이 되는 경우, 선택 밖 같은 음 노트가 끼는가.
7. 레가토가 꼬리를 빼는 규칙: 꼬리 + 일반 노트를 함께 고르면 일반 노트만 붙고 토글 되돌리기가 되는가(기억의 `targets`에 꼬리가 남는다). 꼬리만 고르면 사용자에게 무엇이 보이는가.
8. 꼬리 제외가 결정 2-A(「각 대상 노트의 끝을 … 맞춘다」)와 REQ 5.8 `~`의 읽기를 바꾸는가, 그렇다면 `docs/decisions/`에 적혔는가.
9. 1회차 리뷰어 시험 `legato_toggle_back_restores_hidden_tail_without_touching_clips`의 기대 변경이 정당한가(지우지 않았는가, 클립을 건드리지 않는다는 단언이 남았는가).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. `.` 뒤 `r` 묶음 | 없음 | 1회차 시험은 `cmd-d`만 |
| 2. 끊기가 연속 같은 키 묶음을 해치지 않는가 | 없음 | |
| 3. vim `0`의 카운트 값 | 부분 | `vim_bare_zero_does_not_reset_cursor_note_length`는 문서만 본다 |
| 4. `g 0`과 단독 `0` | 부분 | 카운트 없는 `g 0`만(`escape g 0`) |
| 5. 기본 프리셋 `0` | 있음 | `note_ops`의 길이 코드 시험 |
| 6. `selection_after` 같은 값 | 부분 | 같은 값 둘 합치기 하나 |
| 7. 꼬리 + 일반 노트 레가토 | 없음 | 꼬리 단독만 |
| 8. 꼬리 제외의 결정 기록 | 해당 없음 | 계획 7만 고쳤다 |
| 9. 1회차 시험 기대 변경 | 있음 | 보고서 「리뷰 1 대응」에 근거 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 기본 프리셋 선택 노트에 `r` `.` `r` | 못 깨뜨림. 세 단위 | `dot_replayed_repeat_is_not_coalesced_with_next_r` |
| 2 | `cmd-d . .` 뒤 `cmd-z` 둘 | 못 깨뜨림. 세 단위, 하나씩 되돌린다 | `dot_twice_after_cmd_d_are_separate_units` |
| 3 | `cmd-d` `.` `cmd-d` `cmd-d` | 못 깨뜨림. 뒤 둘은 한 단위로 묶인다(셋째 단위), `cmd-z` 한 번에 둘이 사라진다 | `cmd_d_pair_after_dot_still_coalesces` |
| 4 | vim 맨 `0`, `1 0`, `1 0 0`, `escape`, `0 3`, `l` | 못 깨뜨림. None, 10, 100, None, 3, None. 문서·커서 그대로 | `vim_zero_extends_count_and_is_inert_alone` |
| 5 | vim `2 g 0`, 배치 뷰(`g a`)의 맨 `0` | 못 깨뜨림. 길이 기본값(960), 카운트 남지 않음. 배치 뷰 문서·undo 그대로 | `vim_g_zero_after_count_digits_still_resets_length` |
| 6 | 기본 프리셋 `0` | 못 깨뜨림. 길이 기본값 | `default_zero_still_resets_length` |
| 7 | 같은 값 셋 합치기 | 못 깨뜨림. 하나가 남고 선택에 남는다 | `join_of_three_identical_keeps_one_selected` |
| 8 | 같은 값 둘 + 다른 음 하나에서 같은 값만 골라 분할 | 못 깨뜨림. 조각 둘만 선택, 원래 값·다른 음은 들지 않는다 | `split_of_identical_pair_selects_only_halves` |
| 9 | (0,240) (10,240) 선택 + 선택 밖 (970,240) 퀀타이즈 | 못 깨뜨림. (0,240) 둘, 선택은 그 값 하나, 선택 밖 노트는 들지 않는다 | `quantize_onto_identical_target_keeps_selection_to_targets` |
| 10 | 꼬리 + 일반 노트 선택 `~` `~` | 못 깨뜨림. 일반 노트만 붙고(1920→끝 4800), 선택은 꼬리 + 붙은 노트, 다시 `~`가 원래대로 되돌린다. 클립 그대로, undo 2 | `legato_mixed_tail_and_normal_toggles_normal_only` |
| 11 | 꼬리만 골라 `~` | 깨짐(알림만). 상태줄은 「레가토: 바뀌는 노트가 없다」뿐이다. 다음 노트(1920)가 있어 사용자는 붙기를 기대하는데 꼬리를 뺐다는 이유가 보이지 않는다 | `legato_on_tail_only_explains_why` |

GPUI 확인(시도 1–3의 근거): `gpui-pre-0.3.8/src/window.rs:2486` `Window::dispatch_action`은 `cx.defer`로 디스패치를 미루고, `app/context.rs:301` `defer_in`도 `App::defer`(`Effect::Defer`)를 쓴다. 같은 효과 큐의 등록 순이라 끊기는 다시 보낸 Action 뒤에 돈다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 아니오 | 레가토가 꼬리를 바꾸지 않아 1회차의 겹치는 연장 요청 자체가 사라졌다. 시도 10 |
| 노트는 클립 안, 꼬리는 분할로만 | 아니오 | 꼬리는 레가토 대상에서 빠지고, 일반 노트의 새 끝은 다음 노트 시작(클립 안) |
| 편집 하나 = undo 한 단위 | 아니오 | 시도 1–3 |
| 선택은 현재 클립 노트의 값 | 아니오 | `selection_after`는 뒤 클립에 있는 대상 값과 다중집합 차의 새 노트만 낸다. 시도 7–9 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.7 되돌리기 묶음, 6.2 EDT-1 | 충족 | `crates/keyscore-app/src/note_ops.rs:409-414`. 시도 1–3 |
| 5.8 `.` 마지막 편집 반복 | 충족 | 같은 곳 |
| 5.8.1 `g` 행 `g1`…`g7`·`g0`(A9, 「숫자는 카운트라서」), KM-5 카운트 | 충족 | `assets/keymaps/vim.json:182`, `:230`, `crates/keyscore-app/src/keymap/mod.rs:511`. 시도 4–5 |
| 5.8 `0`(기본 프리셋 길이 기본값) | 충족 | `assets/keymaps/default.json:137`. 시도 6 |
| 5.8 `Cmd+J`·`/`·`q` 뒤 선택(계획 1) | 충족 | `crates/keyscore-app/src/pianoroll/edit.rs:467-476`. 시도 7–9 |
| 5.8 `~` 레가토 토글(결정 2-A) | 충족(해석 기록 필요) | `crates/keyscore-app/src/pianoroll/edit.rs:386-390`. 일반 노트는 결정대로. 숨김 꼬리 제외는 결정에 없는 읽기다(지적 1) |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 이번 diff는 `keyscore-app`의 편집·키맵과 키맵 파일뿐이다.
- 제품 코드의 `unwrap`/`expect`: 이번 diff에 새로 든 것 없음.
- 에러가 삼켜지는 곳: `defer_in`의 `view.update(..).ok()`(GPUI 안)는 창이 닫힌 경우라 정당하다. 1회차가 지적한 「되돌리기 실패 때 `take()`로 기억이 사라짐」은 꼬리를 빼면서 되돌리기가 실패할 경로(클립 연장)가 없어졌다. `plan_restore`가 실패할 남은 경우는 판이 같아 문서가 그대로이므로 없다고 본다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 레가토에서 숨김 꼬리 노트를 빼는 규칙이 결정 2-A(「각 대상 노트의 끝을 같은 클립에서 시작이 더 늦은 노트 … 가운데 가장 이른 시작에 맞춘다」)를 좁히는데 `docs/decisions/`에 적히지 않고 계획 7만 고쳤다. CLAUDE.md 「REQ와 어긋나거나 REQ에 없는 결정이 필요하면 임의로 정하지 않는다」·「REQ의 의미를 바꾸는 변경은 `docs/decisions/`에 먼저」에 걸린다. 규칙 자체는 1회차 리뷰가 제시한 선택지이고 동작은 일관된다(시도 10). `2026-10-26-s3-t3b-note-edit-details.md` 「후속」에 꼬리 제외를 에이전트의 읽기로 적고 사람이 다르게 보면 고칠 줄을 두거나(B25 a′ 선례), 결정 요청을 쓴다. 코드 수정은 필요 없다 | `crates/keyscore-app/src/pianoroll/edit.rs:386-390`, `docs/reports/s3-t3b/plan.md:39`, `docs/decisions/2026-10-26-s3-t3b-note-edit-details.md` |
| 2 | 제안 | 꼬리만 골라 `~`를 누르면 「레가토: 바뀌는 노트가 없다」만 나온다. 다음 노트가 있어 붙기를 기대하는 사용자에게 이유가 보이지 않는다. 대상에서 꼬리를 뺐을 때 「숨김 꼬리 노트는 레가토하지 않는다」 같은 문구를 낸다. 거절하면 `legato_on_tail_only_explains_why`의 기대를 바꾸고 근거를 보고서에 적는다(시험은 지우지 않는다) | `crates/keyscore-app/src/note_ops.rs:276-281`, `crates/keyscore-app/src/pianoroll/edit.rs:386-390` |

리뷰 1 지적별 판정:

- 지적 1: 고쳐졌다(다른 방식). 1회차 시험의 기대 변경은 정당하다. 시험을 지우지 않았고, 원래 핵심 단언(클립·꼬리 그대로)은 남았으며, 보고서에 근거가 있다. 다만 규칙 변경의 결정 기록이 빠졌다(위 지적 1).
- 지적 2: 고쳐졌다. 끊기가 연속 같은 키 묶음을 해치지 않는다(시도 3).
- 지적 3: 고쳐졌다. `g 0`·기본 프리셋 `0`·`Step`과 충돌하지 않는다. 참고로 이동 처리기가 아직 `take_count`를 쓰지 않아(사용처 없음) `1 0 l`은 지금 한 스텝이다. 이 Task 범위 밖이므로 지적하지 않는다.
- 지적 4: 고쳐졌다. 넓힌 규칙이 분할·퀀타이즈·레가토 뒤 선택에 원치 않는 노트를 남기지 않는다(시도 8–10). 선택이 값 집합이라 「같은 값 사본이 남으면 선택」은 화면에서 이전과 구별되지 않는다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1(결정 기록)을 고치고 지적 2를 받아들이거나 근거와 함께 거절한 뒤 재리뷰한다. 코드 회귀는 찾지 못했다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(11개, 실패 1)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(`review_s3_t3b.rs`는 기대만 바뀌었다)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
