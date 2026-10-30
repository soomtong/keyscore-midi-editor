# 적대적 리뷰: S3-T4b vim Step과 MML 호환 (리뷰 3회차)

- 리뷰어 세션: 구현자와 다른 세션(서브 에이전트, 구현 컨텍스트를 물려받지 않음). 1·2회차 리뷰어와도 다른 세션이다
- 대상 커밋: d4e3738(리뷰 2 대응 코드는 5de4d48)
- 범위: 리뷰 2 대응 커밋의 수정 범위(`step_tie`의 대상 검사, `step_discard`의 `n` 처리, 결정 2절 문구)와 그것이 만든 회귀

## 1. 코드만 읽고 적은 「내가 검사할 것」

`tests/step_vim_mml.rs`, `tests/review_s3_t4b.rs`, `tests/review_s3_t4b_r2.rs`를 열기 전에 `git show 5de4d48`, `step.rs`, `step_input.rs`(확정·쓰기·쉼표·지우기·타이·레가토·MML), `window.rs`의 undo·redo, `pianoroll/edit.rs`의 `plan_legato`, 결정 두 문서만 읽고 적었다.

1. `<= cursor`로 넓힌 타이 검사가 절반 음 화음을 받는지, 같은 커서에서 타이를 두 번 쳐도 섞인 길이 화음이 모두 늘어나는지.
2. 셋잇단으로 연 화음(`n Ctrl+c e`): `n`은 첫 음 하나에만 걸려 둘째 음(4분)이 스텝(640)보다 길다. 이 화음은 결정 2026-10-29 5절의 「닫은 화음」인데, 2026-10-30 2절 「모두 커서 이전에 끝날 때만」에 걸려 거부된다. 거부 알림이 사실과 맞는지.
3. 레가토 뒤: `plan_legato`는 끝을 다음 노트 시작에 붙이므로 줄일 수도 늘릴 수도 있다. 절반 음 화음에서 레가토가 짧은 음만 커서까지 늘리면 `<=` 검사를 통과한다(2절 문구와 맞음). 레가토가 커서를 넘기면 거부되는지(덮어쓰기에서 커서 뒤 노트가 남는 경우).
4. undo 뒤: `edit_undo`는 `step.last`를 지우지 않는다. 직전 스텝을 되돌린 뒤 `~`가 문서에 없는 노트를 늘리지 않는지(`splice`의 `NoteNotFound`), redo 뒤 다시 타이가 되는지.
5. 덮어쓰기 타이: 절반 음 화음을 늘리면서 [커서, 커서 + 길이)에서 시작하는 노트만 지우는지.
6. 「커서가 든 클립 안만」(결정 2절 31행): 음·쉼표는 `step_host`(시작 ≤ 커서 < 끝, 없으면 끝 = 커서), 타이는 `last.clip`, `Backspace`는 커서 바로 앞 틱을 품은 클립을 쓴다. 커서 = A 끝 = B 시작인 경계에서 셋이 서로 다른 클립을 고른다. 문구와 맞는지, 타이가 경계에서 B 노트와 겹친 채 남지 않는지.
7. `step_discard`: MML 대기만 버리면 `prefix`에 `n`만 남기고 `triplet_next`는 그대로. `Shift+Backspace`(같은 `step_discard`를 부름) 뒤에도 표시와 적용이 맞는지. 조합을 버리면 조합이 이미 소비한 셋잇단도 함께 사라지는지.
8. `prefix.retain(|ch| ch == 'n')`은 `n`을 여러 개 남길 수 있다. `n n l 8 backspace c`의 표시(`nnc_`)와 적용(셋잇단 한 번).
9. 길이 코드 뒤 MML 버림(`n 3 l 1 6 backspace c`): 길이 코드는 스티키로 이미 적용됐고 표시에서 빠진다. 표시와 적용.
10. `n`을 남긴 채 Step을 나갔다 들어오면 `triplet_next`·`prefix`가 그대로다. 다음 음의 표시와 적용이 맞는지.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 절반 음 화음 타이, 두 번 | 부분 | `r2_tie_after_chord_closed_by_half_note`(한 번). 두 번은 리뷰 시험으로 더함 |
| 2. 셋잇단 화음 타이 | 없음 | 리뷰 시험으로 더함(알림만 깨짐) |
| 3. 레가토 뒤 타이 | 부분 | `review_tie_after_legato_is_refused`(단음, 커서를 넘김). 화음·커서 안쪽 레가토는 리뷰 시험으로 더함 |
| 4. undo 뒤 타이 | 없음 | 리뷰 시험으로 더함 |
| 5. 덮어쓰기 타이 + 절반 음 화음 | 없음 | 단음 덮어쓰기 타이만(`review_s3_t4b.rs`). 리뷰 시험으로 더함 |
| 6. 클립 경계의 클립 선택 | 부분 | `r2_overwrite_rest_across_clip_boundary`(쉼표가 걸침). 커서가 경계에 있을 때의 음·`Backspace`·타이는 리뷰 시험으로 더함 |
| 7. MML 버림과 `n` | 부분 | `Backspace`만(`r2_triplet_prefix_after_discarded_mml`). `Shift+Backspace`, 조합 버림은 리뷰 시험으로 더함 |
| 8. `n n` | 없음 | 리뷰 시험으로 더함 |
| 9. 길이 코드 뒤 MML 버림 | 없음 | 리뷰 시험으로 더함 |
| 10. Step 나갔다 들어오기 | 없음 | 리뷰 시험으로 더함 |

## 3. 깨뜨리기 시도

새 시험 파일: `crates/keyscore-app/tests/review_s3_t4b_r3.rs`. 실행: `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t4b_r3` → 15개 중 14 통과, 1 실패(알림 문구, 제안 1). 같은 방식으로 `review_s3_t4b`(11개), `review_s3_t4b_r2`(12개), `step_input`(28개), `step_vim_mml`(`-- --skip preview_from_bar`, 21개), `--lib step`(5개)도 돌려 모두 통과했다. 새 시험 파일은 `rustfmt --check`와 `cargo clippy --test review_s3_t4b_r3 -- -D warnings`를 통과한다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 계이름형 `n ctrl-c e enter ~`: C 640, E 960, 커서 640. 타이는 거부되고 문서·undo 그대로(2절 문구와 맞음). 알림은 「타이: 바로 앞에 친 음이 없다」 | 깨짐(알림 문구만, 제안) | `r3_tie_after_triplet_chord` |
| 2 | `ctrl-c shift-e enter`(뒤 노트 G가 960으로 밀림) 뒤 팔레트 `step::TieLegato`: E만 960으로. 이어 `~`: C·E 1920, G 1920으로 밀림, 커서 1920 | 못 깨뜨림 | `r3_tie_after_legato_that_stays_within_step` |
| 3 | 덮어쓰기에서 같은 화음(G 1500은 남음) 뒤 레가토: C·E 1500(커서 960을 넘음). `~`는 거부, undo·커서 그대로 | 못 깨뜨림 | `r3_overwrite_half_chord_legato_past_cursor_refuses_tie` |
| 4 | 덮어쓰기 `ctrl-c shift-e enter ~`: E 1440, C 1920, [960, 1920)에서 시작한 G(960)는 지우고 A(1920)는 남김 | 못 깨뜨림 | `r3_overwrite_tie_on_half_chord` |
| 5 | `c ctrl-d shift-f enter` 뒤 undo, `~`: 「타이: 클립에 그 노트가 없다」, 문서·undo 그대로. redo 뒤 `~`는 D·F를 늘림 | 못 깨뜨림 | `r3_tie_after_undo_does_not_edit` |
| 6 | `ctrl-c shift-e enter ~ ~`: E 2400, C 2880, 커서 2880 | 못 깨뜨림 | `r3_double_tie_on_half_chord` |
| 7 | 커서 = A 끝 = B 시작에서 덮어쓰기 음: B의 노트를 바꿈 | 못 깨뜨림 | `r3_overwrite_note_at_boundary_uses_next_clip` |
| 8 | 같은 경계에서 덮어쓰기 `Backspace`: A의 노트(2880)를 지우고 B는 그대로, 커서 2880. 음(B)과 다른 클립을 쓴다 | 못 깨뜨림(동작은 안전, 문구는 제안 2) | `r3_overwrite_backspace_at_boundary_uses_previous_clip` |
| 9 | A 끝에서 끝난 직전 스텝 뒤 경계에서 덮어쓰기 `~`: 「클립 구간 [0, 7680)이 … 겹친다」로 거부, 문서 그대로(B 노트와 겹친 채 남는 일 없음) | 못 깨뜨림 | `r3_overwrite_tie_at_clip_boundary` |
| 10 | 같은 경계에서 삽입 `~`: 같은 거부, undo 그대로 | 못 깨뜨림 | `r3_insert_tie_at_clip_boundary` |
| 11 | vim `i n l 8 shift-backspace c`(커서 960, 노트 G 0): 마디 처음까지 지움, 표시 `nc_`, 노트 640 | 못 깨뜨림 | `r3_triplet_survives_shift_backspace_discard` |
| 12 | `n c backspace d`: 표시 `d_`, 노트 960(셋잇단은 조합과 함께 버려짐) | 못 깨뜨림 | `r3_triplet_discarded_with_composition` |
| 13 | `n n l 8 backspace c`: 표시 `nnc_`, 노트 640(셋잇단 한 번) | 못 깨뜨림(표시 중복은 기존 동작, 제안 3) | `r3_double_n_then_mml_discard` |
| 14 | `n 3 l 1 6 backspace c`: 대기 표시 `nl16_`, 버린 뒤 `nc_`, 노트 640 | 못 깨뜨림 | `r3_length_code_then_mml_discard` |
| 15 | `n l 8 backspace` 뒤 Normal로 나갔다 Step으로 다시: `c`의 표시 `nc_`, 노트 640(표시와 적용이 같다) | 못 깨뜨림 | `r3_triplet_across_step_exit` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 못 찾음 | 경계의 타이가 클립을 늘리려 하면 `splice`가 거부(시도 9·10) |
| 노트는 클립 안(시작 기준) | 못 찾음 | 타이·덮어쓰기는 `last.clip` 안의 `splice` |
| 확정 하나 = `apply` 하나 = undo 한 단위 | 못 찾음 | 거부 경로에서 undo 길이 그대로(시도 1·3·5·9·10) |
| 타이는 직전 스텝이 커서를 넘어 끝나지 않을 때만 | 지킴 | 시도 1–6. `last`가 문서와 어긋나면 `NoteNotFound`로 거부(시도 5) |
| 셋잇단 접두의 표시 = 적용 | 못 찾음(중복 `n` 표시는 제안) | 시도 11–15 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.1 타이 `~` | 충족 | `crates/keyscore-app/src/step_input.rs:799-883`. 절반 음 화음(시도 2·4·6), 레가토 뒤(시도 2·3), undo 뒤(시도 5) |
| 5.1 계이름 MML `o5` `l8` `r` | 충족 | `step_input.rs:986-1068`, 버림과 `n`(시도 11–15) |
| 5.6 셋잇단 `n`과 상태줄 조합 표시 | 충족 | `step_input.rs:702-715`, `step.rs:317-326` |
| 5.8.1 `R` 덮어쓰기 | 충족 | `step_input.rs:293-301`, `618-627`, `721-728`, `845-851`. 경계 동작은 안전(시도 7–10), 결정 문구는 제안 2 |
| 5.8.1 `Shift+Backspace` | 충족 | `step_input.rs:675-699`(시도 11) |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 리뷰 2 대응 커밋은 `step_input.rs`와 결정 문서만 바꿨다.
- 제품 코드의 `unwrap`/`expect`: 새로 더한 것 없음(`git show 5de4d48 -- crates/*/src`).
- 에러가 삼켜지는 곳
  - `step_tie`의 두 거부(직전 스텝이 없음, 노트가 커서를 넘어 끝남)가 같은 알림 「바로 앞에 친 음이 없다」를 낸다. 셋잇단 화음·레가토 뒤에는 직전 스텝이 있으므로 사실과 다르다(제안 1).
  - undo 뒤의 타이는 `splice`의 「클립에 그 노트가 없다」로 알린다. 편집은 없으니 안전하지만 사용자에게는 내부 사정이 보인다(제안 1과 같은 갈래).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | `n Ctrl+c e`로 닫은 셋잇단 화음은 둘째 음(4분)이 스텝(640)을 넘어 끝나 타이가 거부된다. 결정 2절 「모두 커서 이전에 끝날 때만」과 맞으므로 동작은 받는다. 다만 알림이 「타이: 바로 앞에 친 음이 없다」라 사실과 다르다(직전 스텝은 있다). 노트가 커서를 넘어 끝나는 거부는 다른 알림(예: 「타이: 직전 스텝의 노트가 커서 뒤까지 이어진다」)으로 나누거나, 결정 2절에 셋잇단 화음 예를 적는다. 시험 `r3_tie_after_triplet_chord`(거절하면 알림 단언을 고친다) | `crates/keyscore-app/src/step_input.rs:825-831` |
| 2 | 제안 | 결정 2절 31행 「지우는 노트는 커서가 든 클립(삽입과 같은 클립 규칙) 안의 것뿐이다 … (`Backspace`가 커서 바로 앞 틱의 클립에서만 지우는 것과 같다)」는 한 문장에 두 규칙을 같다고 적는다. 커서 = A 끝 = B 시작이면 덮어쓰기 음·쉼표는 B(`step_host`), `Backspace`는 A(커서 바로 앞 틱), 타이는 직전 스텝의 클립(`last.clip`, 여기서는 A)을 쓴다(시도 7·8·9). 동작은 안전하다(타이는 겹침으로 거부). 문구를 「음·쉼표는 커서가 든 클립, 타이는 직전 스텝의 클립, `Backspace`·`Shift+Backspace`는 커서 바로 앞 틱의 클립. 어느 경우든 그 한 클립 안만 지운다」로 나누기를 권한다 | `docs/decisions/2026-10-30-s3-t4b-vim-step-mml.md:31` |
| 3 | 제안 | `n n`은 셋잇단을 한 번만 적용하는데 `prefix.retain(|ch| ch == 'n')`가 `n`을 둘 다 남겨 `nnc_`로 보인다(리뷰 2 대응 전에도 `n n c`는 `nnc_`였으니 회귀는 아니다). `step_triplet`에서 이미 `triplet_next`면 `n`을 더 붙이지 않으면 표시와 적용이 맞는다. 시험 `r3_double_n_then_mml_discard`는 적용만 단언한다 | `crates/keyscore-app/src/step_input.rs:540-549`, `709` |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

리뷰 2 지적 1의 수정(`<= cursor`)은 절반 음 화음 타이를 되살리고(시도 2·4·6) 레가토로 커서를 넘긴 노트는 계속 막는다(시도 3). undo 뒤 어긋난 `last`는 편집 없이 거부된다(시도 5). 제안 3 대응(`step_discard`)은 `Backspace`·`Shift+Backspace`·조합 버림·Step 재진입에서 표시와 적용이 맞는다(시도 11–15). 제안 2의 결정 문구는 코드와 동작이 안전하게 맞지만 문장이 경계의 세 규칙을 섞어 적는다. 차단·수정 사항은 없고 제안 셋만 남는다. 제안을 거절하면 보고서에 이유를 적고 `r3_tie_after_triplet_chord`의 알림 단언을 고친다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(리뷰 2 대응 커밋은 `step_input.rs`·결정 문서만)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
