# 적대적 리뷰: S3-T4b vim Step과 MML 호환 (리뷰 2회차)

- 리뷰어 세션: 구현자와 다른 세션(서브 에이전트, 구현 컨텍스트를 물려받지 않음). 1회차 리뷰어와도 다른 세션이다
- 대상 커밋: 651c824567a0d2403eb894453b74814cd46e8868(리뷰 1 대응 코드는 73f8ba8)

## 1. 코드만 읽고 적은 「내가 검사할 것」

`tests/step_vim_mml.rs`, `tests/review_s3_t4b.rs`를 열기 전에 `git show 73f8ba8`, `step.rs`, `step_input.rs`, `window.rs`(`apply_mode_event`, `render`, undo·redo), `settings_tabs.rs`, `settings_window.rs`, 키맵, core `splice`, 두 결정 문서만 읽고 적었다.

1. 지적 1 수정(`step_tie`의 `last.notes.iter().all(|n| s + n.end() == cursor)`)이 레가토 말고 다른 정당한 직전 스텝을 막지 않는지. 특히 `Ctrl+c` 뒤 `Shift+e`(절반)로 닫은 화음은 노트 길이가 섞여 모든 노트가 커서에서 끝나지 않는다. 결정 2026-10-29 5절은 「닫은 화음」도 타이 대상이고 「그 노트들을 늘린다」.
2. 타이 대상과 다른 편집 뒤의 `last`: Step 재진입(`mode::StepAtBarStart` 등)으로 커서가 옮겨진 뒤, undo 뒤, 레가토 undo 뒤. `splice`가 `replace` 노트를 값으로 찾으므로(`NoteNotFound`) 문서와 어긋난 `last`가 틀린 편집을 만들 수는 없다는 가정을 확인.
3. 지적 3 수정은 그리기(`render` → `refresh_step_layout`) 중에 `step_settle_mml`을 부른다. 그리기 중 `say`·`cx.notify()`가 터지지 않는지, 숫자 없는 `l`·`o`이면 알림이 나오는지, `l16`이면 길이가 확정되는지, 이어지는 건반형 키가 정상인지.
4. 지적 7 수정의 `check_trail` → 지우기 → `drop_after`: 열쇠가 지우기 전 판으로 맞춰지고 지운 뒤 `rekey`로 이어지는지. 타이로 마디를 넘은 스텝(시작 < 새 커서 < 끝) 뒤 `Shift+Backspace`, 이어지는 `Backspace`.
5. 제안 5 수정(`prefix.retain(|ch| ch == 'n')`)의 같은 갈래: `n l 8` 뒤 `Backspace`로 대기를 버리면 `step_discard`가 `prefix`만 비우고 `triplet_next`는 그대로 둔다. 표시와 실제가 어긋나는지.
6. 설정 창 두 선택 상자: 파일에서 바뀐 목록 밖 값(250, 0)이 덧붙여 보이는지, 덧붙인 목록에서 고른 자리의 값이 맞는지, 팔레트로 바꾼 기본 길이가 열린 설정 창 표시에 따라오는지.
7. 덮어쓰기와 클립 경계: 덮어쓰기 쉼표가 다음 클립으로 걸칠 때(`reach`는 다음 클립 시작에서 자름) 다음 클립에서 그 범위에 시작하는 노트, 덮어쓰기 음이 다음 클립에 닿을 때(삽입처럼 거부되는지).
8. MML 숫자 범위: `l164`·`l0`·`l016`·`l128`(세 자리 상한 뒤 더 친 숫자), `l.4`, 점64분(90), 팔레트 `MmlDigit(10)`·`MmlDigit(-1)`, `o9 b`(127 초과), `o0 c`.
9. 제안 6 수정: vim `{` `}`가 MML 대기를 먼저 확정하고 대기 컨텍스트가 닫히는지.
10. 진입 처리기: `mode::Step*`가 `step_handlers`로 옮겨진 뒤 배치 뷰(Normal)에서 vim `A`·`a`가 Step으로 들어가는지와 커서. Step 안 재선택이 덮어쓰기를 끄고 커서를 옮기는지, 그 뒤 타이 대상이 없어지는지.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1. 섞인 길이 화음 뒤 타이 | 없음 | `step_input.rs`의 `tie_extends_previous_step_and_advances`는 같은 길이 화음만. 리뷰 시험으로 더함(깨짐) |
| 2. 다른 편집 뒤 `last` | 부분 | 레가토 뒤만(`review_tie_after_legato_is_refused`). 재진입 뒤는 리뷰 시험으로 더함 |
| 3. 그리기 중 MML 확정 | 부분 | `l 1` 한 경우의 컨텍스트만(`review_mml_wait_does_not_survive_layout_switch`). 숫자 없는 대기·알림·`l16`은 리뷰 시험으로 더함 |
| 4. `Shift+Backspace` 자취 열쇠 | 부분 | 거부 경로는 시험 없음(삽입 지우기는 실제로 거부될 경로를 못 찾음). 타이로 마디를 넘은 스텝은 리뷰 시험으로 더함 |
| 5. `n` + MML 대기 버림 | 없음 | 리뷰 시험으로 더함(깨짐) |
| 6. 설정 창 목록 밖 값 | 없음 | 기존 시험은 목록 안 값만(`settings_input_has_step_length_and_wait_choices`) |
| 7. 덮어쓰기와 다음 클립 | 없음 | 리뷰 시험 두 개로 더함(하나 깨짐) |
| 8. MML 숫자 범위 | 부분 | `mml_length` 단위 시험과 `mml_length_reads_denominators_and_dot`. 창 경로의 세 자리 상한·팔레트 범위 밖 숫자는 리뷰 시험으로 더함 |
| 9. `{` `}`와 MML | 없음 | 리뷰 1 대응에 시험이 없다. 리뷰 시험으로 더함 |
| 10. 배치 뷰 진입·재선택 | 부분 | 피아노롤 진입과 팔레트 재선택(`review_palette_reentry_toggles_overwrite_and_commits`)만 |

## 3. 깨뜨리기 시도

새 시험 파일: `crates/keyscore-app/tests/review_s3_t4b_r2.rs`. 실행: `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t4b_r2` → 12개 중 9 통과, 3 실패. 같은 방식으로 `review_s3_t4b`(11개), `step_vim_mml`(`-- --skip preview_from_bar`, 21개), `step_input`(28개)도 돌려 모두 통과했다. 새 시험 파일은 `cargo fmt --check`와 `cargo clippy --test review_s3_t4b_r2 -- -D warnings`를 통과한다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 계이름형 `ctrl-c shift-e enter ~`: 4분 C와 8분 E로 닫은 화음 뒤 타이. 리뷰 1 지적 1 수정 전에는 두 노트를 늘렸다. 지금은 「타이: 바로 앞에 친 음이 없다」로 거부되고 아무것도 바뀌지 않는다 | 깨짐(회귀) | `r2_tie_after_chord_closed_by_half_note` |
| 2 | 덮어쓰기 중 팔레트 `mode::StepAtBarStart`: 덮어쓰기 꺼짐, 커서 0, 이어지는 `~`는 거부 | 못 깨뜨림 | `r2_reentry_moves_and_drops_tie_target` |
| 3 | 계이름형 `l`(숫자 없음) 대기 중 건반형으로: 그리기 중 확정이 「MML: …」 알림을 내고 길이 그대로, 패닉 없음, 이어지는 `a`는 C4 4분 | 못 깨뜨림 | `r2_layout_switch_settles_bare_mml_with_notice` |
| 4 | `l 1 6` 대기 중 건반형으로: 16분 확정, 이어지는 건반형 `4 a`는 8분 C4 | 못 깨뜨림 | `r2_layout_switch_settles_l16` |
| 5 | `l164`·`l0`·`l016`·`l1288`, `l.4`, `l64.`(90), 팔레트 `MmlDigit(10)`·`(-1)`(대기 남음), `o9 b`(알림), `o0 c`(12) | 못 깨뜨림 | `r2_mml_numbers_out_of_range` |
| 6 | vim `l 8 }`: 8분 확정, 대기 컨텍스트 닫힘 | 못 깨뜨림 | `r2_braces_settle_mml_first` |
| 7 | `n l 8 backspace c`: 대기를 버린 뒤 조합 표시는 `c_`(`n` 없음)인데 쓴 노트는 셋잇단 4분(640). `step_discard`가 `prefix`만 비우고 `triplet_next`는 남긴다 | 깨짐(표시) | `r2_triplet_prefix_after_discarded_mml` |
| 8 | 덮어쓰기 쉼표 [3360, 4320)가 다음 클립(3840 시작)으로 걸침: 다음 클립의 3840 노트가 남는다(쉼표는 다음 클립 시작에서 잘리고 지우기는 이 클립 안만) | 깨짐(결정 문구와 다름, 제안) | `r2_overwrite_rest_across_clip_boundary` |
| 9 | 같은 자리 덮어쓰기 음: 다음 클립과 겹쳐 거부, undo 단위·커서 그대로 | 못 깨뜨림 | `r2_overwrite_note_into_next_clip_is_refused` |
| 10 | 설정 파일 값 250 ms → 상자에 「250 ms」, 아래로 한 칸 → 300. 팔레트 `SetStepDefaultLength(sixtyfourth)` → 열린 설정 창이 「64분음표」. 0 ms → 「0 ms」 | 못 깨뜨림 | `r2_settings_choices_follow_outside_values` |
| 11 | 배치 뷰(Normal)에서 vim `A`(960 → 3840, Step), `Esc` 뒤 `a`(노트 끝 960) | 못 깨뜨림 | `r2_entry_from_arrangement_view` |
| 12 | 2880에서 `a ~ s`(타이로 마디를 넘은 스텝 2880–4800, 다음 스텝 4800–5760) 뒤 `Shift+Backspace`: 3840까지 지움(undo 한 단위), 두 스텝 기록 버림, 이어지는 `Backspace`는 4분 한 스텝(2880)으로 타이 노트를 지움 | 못 깨뜨림 | `r2_shift_backspace_after_tie_across_bar` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 못 찾음 | 덮어쓰기 음이 다음 클립에 닿으면 `splice`가 `ClipOverlap`으로 거부(시도 9). 쉼표의 `reach`는 다음 클립 시작에서 자름(시도 8) |
| 노트는 클립 안(시작 기준) | 못 찾음 | 덮어쓰기 지우기 범위는 클립 기준 [rel, rel + len)이고 `splice`가 꼬리 규칙(시작만 클립 안)을 지킨다 |
| 확정 하나 = `apply` 하나 = undo 한 단위 | 못 찾음 | `Shift+Backspace`(시도 12), 덮어쓰기 거부 시 undo 길이 그대로(시도 9) |
| 조합과 MML 대기는 동시에 없다 | 못 찾음 | `step_key`·`step_mml_open`·`step_octave_once`가 모두 대기를 먼저 확정 |
| MML 대기는 계이름형에서만 | 못 찾음 | 시도 3·4, 리뷰 1 시험 8·9 |
| 타이는 직전 스텝이 커서에서 끝날 때만 | 지키지만 너무 넓게 막는다 | 섞인 길이 화음이 막힘(시도 1) |
| `last`가 문서와 어긋나도 틀린 편집 없음 | 못 찾음 | `splice`가 `replace` 노트를 값으로 찾아 없으면 `NoteNotFound`. 재진입 뒤는 커서 검사로 거부(시도 2) |
| 셋잇단 접두의 표시 = 적용 | 깨짐(표시만) | 시도 7 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.1 vim `i`로 Step | 충족 | `crates/keyscore-app/src/step_input.rs:1120`(`enter_step`), 배치 뷰에서도(시도 11) |
| 5.1 타이 `~` | 충족(단음). 화음은 결정 2026-10-29 5절 위반 | `step_input.rs:819-824`. 지적 1 |
| 5.1 계이름 MML `o5` `l8` `r` `c+` `c-` | 충족 | `assets/keymaps/default.json:297-318`, `step_input.rs:983-1068`. 숫자 범위(시도 5), 레이아웃 경계(시도 3·4) |
| 5.6 기본 길이 「설정에서 바꿀 수 있다」 | 충족 | `settings_window.rs:182-185`, `settings_tabs.rs:557-582`(시도 10) |
| 5.6 옥타브 대기 300 ms 「설정 가능」 | 충족 | `settings_window.rs:186-195`, `settings_tabs.rs:338-347`(목록 밖 값, 시도 10) |
| 5.6 셋잇단 `n`과 상태줄 조합 표시 | 충족(표시 어긋남은 제안) | `step_input.rs:702-709`. 지적 3 |
| 5.8.1 `I` `a` `A` 진입 | 충족 | `step_input.rs:1074-1150` |
| 5.8.1 `R` 덮어쓰기 | 충족 | `step_input.rs:296-301`, `624-640`, `845-850`, `727-731`. 다음 클립 경계는 지적 2(결정 문구) |
| 5.8.1 Step 표 `Shift+Space` `Shift+Backspace` `Shift+Enter` `{` `}` | 충족 | `step_input.rs:578-581`, `677-699`, `964-980`, `880-912`, `assets/keymaps/vim.json:212-221` |
| 5.11 Step `Tab` | 충족 | `step_input.rs:770-794` |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 리뷰 1 대응 커밋은 `step.rs`·`step_input.rs`·결정 문서만 바꿨다.
- 제품 코드의 `unwrap`/`expect`: 새로 더한 것 없음(`git show 73f8ba8 -- crates/*/src`).
- 에러가 삼켜지는 곳
  - `refresh_step_layout`(그리기 중)의 MML 확정은 `say`와 `cx.notify()`를 부른다. 그리기 중 자기 뷰 알림이라 다음 프레임을 한 번 더 그리게 되지만 대기가 비워지므로 되풀이되지 않는다. 시험에서 패닉·무한 그리기 없음(시도 3). 확정은 그 창이 그려질 때 일어나므로 레이아웃을 바꾼 뒤 그 창이 그려지기 전의 키는 옛 컨텍스트로 디스패치된다. `SetKeyLayout` 처리기가 `refresh_windows`를 부르므로 실제 피해는 찾지 못했다.
  - `step_tie`의 새 검사는 섞인 길이 화음을 「바로 앞에 친 음이 없다」로 거부한다. 알림이 사실과 다르다(지적 1).
  - `step_mml_digit`은 범위 밖 숫자와 `l.` 뒤 숫자를 조용히 버린다. 대기 확정 때 `l.` 등으로 알리므로 결정 4절과 맞다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 리뷰 1 지적 1의 수정이 회귀를 만들었다. 타이 대상 검사가 직전 스텝의 「모든」 노트가 커서에서 끝나기를 요구해, `Ctrl+c` 뒤 `Shift+e`로 닫은 화음(4분 + 8분)의 타이가 「타이: 바로 앞에 친 음이 없다」로 거부된다. 결정 2026-10-29 5절은 닫은 화음을 타이 대상으로 하고 「그 노트들을 늘린다」. 수정 전에는 두 노트를 늘렸다. 레가토만 막으려면 레가토가 바꾼 스텝을 표시하거나(예: 레가토 뒤 `LastStep`에 끝이 바뀌었음을 남김), 스텝의 끝(`last.end`)을 정하는 노트(화음은 첫 음, 곧 스텝 길이)만 본다. 시험 `r2_tie_after_chord_closed_by_half_note`(기대가 결정과 다르다고 보면 결정 문서에 섞인 화음의 규칙을 적고 시험 기대를 고친다) | `crates/keyscore-app/src/step_input.rs:819-824` |
| 2 | 제안 | 덮어쓰기 쉼표가 다음 클립으로 걸치면 쉼표 범위에서 시작하는 다음 클립의 노트는 남는다(지우기는 커서가 든 클립 안만). 같은 자리의 덮어쓰기 음은 거부된다. 클립 단위 규칙으로 보면 일관되지만 결정 2절 「스텝이 덮는 범위 [자리, 자리 + 스텝 길이)에서 시작하는 노트를 지운다」와 문구가 다르다. 결정에 「커서가 든 클립 안에서」를 적거나 다음 클립 노트도 지운다. 시험 `r2_overwrite_rest_across_clip_boundary` | `step_input.rs:623-640`, `docs/decisions/2026-10-30-s3-t4b-vim-step-mml.md:25-31` |
| 3 | 제안 | 리뷰 1 제안 5와 같은 갈래: `n l 8` 뒤 `Backspace`가 대기를 버리면 `step_discard`가 `prefix`를 비워 `n`이 표시에서 사라지지만 `triplet_next`는 남아 다음 음이 셋잇단이 된다(표시 `c_`, 노트 640). 버릴 때 `n`을 표시에 남기거나 셋잇단도 함께 버린다(어느 쪽이든 표시와 적용을 맞춘다). 시험 `r2_triplet_prefix_after_discarded_mml` | `step_input.rs:702-709` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단 사항은 없다. 리뷰 1 지적 1–4와 제안 5–7의 수정은 각자의 경로에서 맞게 동작한다(리뷰 1 시험 11개 통과, 같은 갈래 시도 2–6·12 못 깨뜨림). 다만 지적 1 수정이 섞인 길이 화음의 타이를 막는 회귀를 만들었다(지적 1). 이것을 고치거나 결정 문서에 규칙을 적고 시험 기대를 맞춘 뒤 다시 본다. 제안 2·3을 거절하면 해당 시험의 기대를 고치고 거절 이유를 보고서에 적는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(리뷰 1 대응 커밋은 `step.rs`·`step_input.rs`·결정 문서만)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
