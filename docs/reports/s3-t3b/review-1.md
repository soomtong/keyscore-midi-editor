# 적대적 리뷰: S3-T3b 노트 편집 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰어 역할로 새로 띄운 에이전트, 구현 컨텍스트 없음)
- 대상 커밋: `10e85b0`(브랜치 `s3/t3b-note-edit`, `git diff main...s3/t3b-note-edit`)
- 새 시험: `crates/keyscore-app/tests/review_s3_t3b.rs`(9개, 4개 실패). 실행: `CARGO_TARGET_DIR=target/review-t3b cargo test -p keyscore-app --test review_s3_t3b --test note_ops --test note_ops_pure`

## 1. 코드만 읽고 적은 「내가 검사할 것」

1. 숨김 꼬리 노트에 `~`를 두 번 눌러(붙이기 → 되돌리기) 꼬리가 원래대로 돌아오는가. `plan_restore`가 `map_notes`를 쓰므로 「꼬리 아님 → 꼬리」 바꾸기가 클립 연장·겹침 거부로 빠지지 않는가.
2. `.`이 coalescing Action(`Duplicate`·`Repeat`)을 `dispatch_action`(지연)으로 보낸 뒤 같은 키(`cmd-d`)를 누르면 두 편집이 undo 한 단위로 묶이지 않는가(키 관찰의 묶음 끊기가 지연 디스패치보다 먼저 돈다).
3. vim 프리셋에서 A9 뒤에도 맨 `0`이 `note::ResetLength`로 남아 있지 않은가(`Normal > PianoRoll`의 기본 `0`을 vim이 덮지 않는다).
4. 같은 값 노트 둘(완전히 같은 노트)을 합치면 결과와 편집 뒤 선택이 맞는가(`selection_after`의 `gone`이 집합이라 남은 사본까지 버리는가).
5. 숨김 꼬리 노트 분할·합치기가 적용되고 undo로 정확히 돌아오는가. 같은 값 노트가 여럿일 때 `map_notes`의 개수 검사.
6. `e` 대기 중 수식키 조합(`cmd-z`)·`Esc`·Shift 단독은 먹고 대기만 푸는가. vim `r` 뒤 시퀀스·카운트가 남지 않는가.
7. 레가토 기억: undo 뒤(판이 바뀜) 다시 `~`는 되돌리기가 아니라 붙이기인가.
8. 퀀타이즈: 이미 그리드 위면 「바뀌는 노트가 없다」와 undo 기록 없음.
9. 분할: 커서가 노트 시작·끝이면 거부, 가사는 앞쪽에만.
10. 배치 뷰·입력란·현재 클립 밖 선택에서 새 동작이 문서를 바꾸지 않는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 숨김 꼬리 노트 레가토 토글 되돌리기 | 없음 | `legato_tilde_toggles_back`은 일반 노트만. 꼬리는 분할·합치기에만 있다 |
| 2. `.`이 되풀이한 coalescing Action과 다음 같은 키의 묶음 | 없음 | `dot_repeats_last_edit`는 `Alt` 조작(`LastEdit::Op`, 바로 적용)만 묶음을 본다 |
| 3. vim 맨 `0` | 없음 | `vim_shift_k_is_show_info_and_g_digits_are_lengths`는 `g 0`만 본다 |
| 4. 같은 값 노트 합치기 뒤 선택 | 없음 | `selection_after_keeps_untouched_targets_and_adds_new_notes`는 중복 값이 없다 |
| 5. 꼬리·같은 값 노트 분할·합치기와 undo | 부분 | `split_keeps_hidden_tail_as_tail`, `join_with_tail_keeps_tail`(중복·undo 없음) |
| 6. `e` 대기와 수식키·카운트 | 부분 | `Esc`·`x`는 있다. `cmd-*`·vim 카운트 없음 |
| 7. 레가토 기억과 undo | 없음 | 다른 편집(`alt-k`)이 낀 경우만 |
| 8. 퀀타이즈 변화 없음 | 있음 | `quantize_q_uses_settings` |
| 9. 분할 경계·가사 | 있음 | `split_slash_and_vim_s_split_at_cursor`, `split_keeps_lyric_on_first_half_only` |
| 10. 배치 뷰·클립 밖 선택 | 있음 | `arrangement_view_note_edit_keys_do_nothing`, `outside_clip_selection_is_refused_for_new_edits`. 입력란 포커스는 T3a `edit_allowed` 재사용 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 클립 분할로 생긴 숨김 꼬리 노트(960, 7680, 끝 8640 > 클립 7680)를 선택하고 `~` 두 번 | 깨짐. 둘째 `~`가 「레가토: 클립 구간 [0, 11520)이 같은 트랙의 다른 클립과 겹친다」로 거부되고 노트는 붙인 채로 남는다. 기억은 `take()`로 이미 비워져 셋째 `~`도 되돌리지 못한다 | `legato_toggle_back_restores_hidden_tail_without_touching_clips` |
| 2 | `cmd-d` `.` `cmd-d` 뒤 undo 단위 수 | 깨짐. 2단위(기대 3). `.`이 보낸 `Duplicate`는 키 관찰의 묶음 끊기 뒤(지연 디스패치)에 적용돼 열린 채 남고, 다음 `cmd-d`가 거기에 묶인다. `cmd-z` 한 번이 둘을 함께 되돌린다 | `dot_replayed_duplicate_is_not_coalesced_with_next_cmd_d` |
| 3 | 같은 경로의 `alt-k` `.` `alt-k` | 못 깨뜨림(3단위). `LastEdit::Op`은 처리기 안에서 바로 적용돼 키 관찰이 끊는다 | `dot_replayed_alt_op_is_not_coalesced_with_next_alt_op` |
| 4 | vim 프리셋, 커서 노트(길이 240)에서 `1 0`(카운트 10을 치려다) | 깨짐. `0`이 기본 프리셋 `Normal > PianoRoll`의 `note::ResetLength`로 남아 길이가 960으로 바뀐다(undo 1) | `vim_bare_zero_does_not_reset_cursor_note_length` |
| 5 | 완전히 같은 노트 둘을 골라 `cmd-j` | 깨짐(선택만). 노트 하나가 남지만 선택이 빈다. `selection_after`의 `gone`이 집합이라 사본 하나가 사라진 값을 통째로 뺀다 | `join_of_identical_duplicates_keeps_remaining_note_selected` |
| 6 | 같은 값 꼬리 노트 둘 분할(넷) → 전체 합치기 → undo 두 번 | 못 깨뜨림. 꼬리 하나로 합쳐지고 클립 길이 그대로, undo로 원래 프로젝트와 같다 | `split_and_join_on_duplicate_tails_apply_and_undo_exactly` |
| 7 | `e` 대기 중 `cmd-z` | 못 깨뜨림. 먹고 대기만 푼다(문서·undo 그대로) | `pitch_pending_eats_cmd_z` |
| 8 | vim `3 r a` 뒤 `l` | 못 깨뜨림. 카운트가 남지 않는다 | `vim_count_before_r_does_not_leak_to_next_motion` |
| 9 | `~` `cmd-z` (같은 선택) `~` | 못 깨뜨림. 판이 달라 다시 붙인다 | `legato_after_undo_applies_again` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 아니오 | 시도 1에서 `plan_restore`가 겹치는 연장을 내지만 core `map_notes`가 거부한다(문서 그대로) |
| 노트는 클립 안(시작), 꼬리는 분할로만 | 아니오 | 분할은 원래 노트를 뒤쪽으로 바꾸고, 합치기는 꼬리 노트를 바꾼다. core `replace_notes`의 꼬리 판정(바꾸기 전 길이)을 통과한다. 시도 6 |
| 틱 ≥ 0, 길이 > 0 | 아니오 | 분할은 `시작 < c < 끝`만, 길이는 `with_length` 검사 |
| 채널 1–16, 음높이 0–127 | 아니오 | `pitch_in_octave_of`가 `Pitch::new`로 거른다 |
| 편집 하나 = undo 한 단위 | 예(묶음 쪽으로) | 시도 2: `.`의 지연 디스패치 뒤 같은 coalescing 키가 묶인다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.8 `/` 분할 | 충족 | `crates/keyscore-app/src/pianoroll/edit.rs:240`, `note_ops.rs:144` |
| 5.8 `Cmd+J` 합치기(결정 1-A) | 충족 | `edit.rs:284`. 같은 값 중복의 뒤 선택은 지적 4 |
| 5.8 `1`–`7` / 점음표 / `0`, 5.7 「길이 직접 지정」, 5.6 길이 표 | 충족 | `edit.rs:175`, `default.json:123-137` |
| 5.8 `e` 음 키 하나(결정 4-A, P3 두 레이아웃) | 충족 | `note_ops.rs:306-389`, `pianoroll/keys.rs` |
| 5.8 `q`(결정 3-A), EDT-2 설정 값 | 충족 | `note_ops.rs:241`, `settings.rs` `QuantizeSettings` |
| 5.8 `~` 레가토 토글(결정 2-A) | 미충족(부분) | 숨김 꼬리 노트는 되돌리지 못한다. `edit.rs:413-428`(지적 1) |
| 5.8 `.` 마지막 편집 반복 | 충족 | `note_ops.rs:394-419`. 묶음 문제는 EDT-1 행 |
| 5.7 되돌리기 묶음, 6.2 EDT-1 「같은 키를 연속으로 누른 조작만 하나로 묶는다」 | 미충족(부분) | `.` 뒤 `cmd-d`가 묶인다. `note_ops.rs:409`, `window.rs:148-158`(지적 2) |
| 5.8.1 vim `s` `r` `J` `K` `q` `~` `.` | 충족 | `vim.json:17-18, 43-44, 212-213, 235-236` |
| 5.8.1 `g` 행 `g1`…`g7`·`g0`(A9, 「숫자는 카운트라서」) | 미충족(부분) | `g` 접두는 있으나 맨 `0`이 vim에서 여전히 길이 기본값이다. `vim.json:217-237`에 `0` 덮기가 없고 `default.json:137`이 이긴다(지적 3) |
| 7.3 KM-7 표기 규칙(B25 a′) | 충족 | `default.json` 머리 주석, `shift-N` 14개 삭제, REQ 문구 |
| P3 두 레이아웃 | 충족 | `keys.rs` 표, KM-7 문자 기준(계이름형 `key_char`) |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. `keyscore-midi`·`keyscore-core`·엔진 파일에 변경이 없다(`git diff --stat`).
- 제품 코드의 `unwrap`/`expect`: `settings.rs` `QuantizeSettings::to_core`의 `.expect("범위로 줄인 강도·스윙")` 하나. 강도·스윙을 `min(100)`·`min(75)`로 줄인 뒤라 core `new`가 실패할 수 없다. 정당하다.
- 에러가 삼켜지는 곳: `note_legato`가 기억을 `take()`한 뒤 되돌리기가 실패하면 기억이 사라진다(`note_ops.rs:269`). 에러는 상태줄에 나오지만 다음 `~`는 붙이기로 간다(지적 1과 함께 고칠 것). `intercept_pitch_keys`의 `update(..).unwrap_or(false)`는 창이 닫힌 경우라 정당하다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 숨김 꼬리 노트의 레가토 되돌리기가 실패한다. `plan_restore`가 `map_notes`를 거치는데, 붙인 노트(이제 꼬리 아님)를 꼬리 값으로 바꾸므로 core가 「꼬리 아님 → 끝이 클립 밖」으로 보고 클립을 마디 단위로 늘리려 한다. 분할 직후처럼 오른쪽 클립이 붙어 있으면 겹침으로 거부되고, 오른쪽 클립이 없으면 클립이 늘어 꼬리가 일반 노트가 된다(계획 7 「되돌리기는 원래 값이라 꼬리도 원래대로다」와 다르다). 꼬리를 되살리는 경로(예: 레가토가 꼬리를 바꾸지 않게 하거나, 되돌리기를 core가 꼬리로 인정하는 편집으로)가 필요하다. 실패 때 `take()`로 기억이 사라지는 것도 함께 본다. 재현: `legato_toggle_back_restores_hidden_tail_without_touching_clips` | `crates/keyscore-app/src/pianoroll/edit.rs:413-428`, `crates/keyscore-app/src/note_ops.rs:269-276` |
| 2 | 수정 | `.`이 coalescing Action(`Duplicate`·`Repeat`)을 `window.dispatch_action`(지연)으로 보내면, 키 관찰의 `seal_history`가 그보다 먼저 돌아 되풀이된 편집이 열린 묶음으로 남는다. 다음 `cmd-d`(또는 `r`)가 거기에 묶여 `cmd-z` 한 번에 둘이 되돌려진다. EDT-1 「같은 키를 연속으로 누른 조작만」과 계획 8 「반복된 동작은 undo 한 단위」에 어긋난다. 재현: `dot_replayed_duplicate_is_not_coalesced_with_next_cmd_d`(대조군 `dot_replayed_alt_op_is_not_coalesced_with_next_alt_op`는 통과) | `crates/keyscore-app/src/note_ops.rs:403-409`, `crates/keyscore-app/src/window.rs:148-158` |
| 3 | 수정 | vim 프리셋에서 맨 `0`이 `note::ResetLength`로 남는다. 이 Task 전에는 처리기가 없어 아무 일도 없었으나 이제 커서 노트·선택 길이를 바꾼다. A9(「vim 길이 지정은 `g 1`…`g 7`, 기본값 `g 0`」)와 REQ 5.8.1 `g` 행(「숫자는 카운트라서」)의 취지대로면 vim의 `Normal > PianoRoll`에서 `0`을 덮어야 한다(`1`–`7`을 `operator::Count`로 덮은 것과 같은 자리). `1 0 l`을 치는 vim 사용자는 길이가 조용히 바뀐다. 재현: `vim_bare_zero_does_not_reset_cursor_note_length` | `assets/keymaps/vim.json:217-237`, `assets/keymaps/default.json:137` |
| 4 | 제안 | `selection_after`의 `gone`이 다중집합 차를 집합으로 접어, 같은 값 사본 하나만 사라져도 그 값을 선택에서 뺀다. 완전히 같은 노트 둘을 합치면 남은 노트가 선택에서 빠져 선택이 빈다(계획 1 「대상 가운데 그대로 남은 노트」). 개수 비교(뒤 개수 > 0이면 남김)로 고칠 수 있다. 재현: `join_of_identical_duplicates_keeps_remaining_note_selected` | `crates/keyscore-app/src/pianoroll/edit.rs:461-471` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1–3을 고치고 리뷰어 시험 4개(실패 중)를 통과시킨 뒤 재리뷰한다. 지적 4는 거절할 수 있으나, 거절하면 `join_of_identical_duplicates_keeps_remaining_note_selected`를 계획 규칙에 맞게 바꾸는 근거를 보고서에 적는다(시험은 지우지 않는다).

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(9개, 실패 4)
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`review_s2_t2.rs` 대조군 단언 변경은 B25 결정의 직접 결과로 보고서에 적혀 있다. 시험은 지워지지 않았다)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
