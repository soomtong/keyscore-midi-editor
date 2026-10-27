# 적대적 리뷰: S3-T3c vim 오퍼레이터·텍스트 객체·비주얼 선택·카운트 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 새 세션(구현 컨텍스트를 물려받지 않은 리뷰 전용 에이전트)
- 대상 커밋: `0ee70f0`(브랜치 `s3/t3c-vim-operators`, `main...` 차이 전체)
- 깨뜨리기 시험: `crates/keyscore-app/tests/review_s3_t3c.rs`(19개, 5개 실패 = 지적 재현)
- 실행: `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t3c` → 14 통과, 5 실패. 같은 target으로 `vim_operators`·`vim_pure`·`review_s2_t2_r2`·`review_s2_t2_r3`(22)는 모두 통과. `cargo clippy -p keyscore-app --test review_s3_t3c -- -D warnings`와 `rustfmt --check`는 깨끗하다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

`src/vim.rs`, `src/pianoroll/vim.rs`, `src/selection.rs`·`src/window.rs` 차이를 읽고 시험 파일을 열기 전에 적었다.

1. 오퍼레이터 대기는 키 입력 경로(`intercept_vim_keys`, `vim_after_keystroke`)에서만 끝난다. 메뉴·팔레트로 실행한 Action(`select::All`, `edit::Undo`)이 선택을 만들면 대기와 선택이 공존하고, `context_path`는 `Selection` 경로라 `Operator`가 빠진다(불변식 「대기와 비주얼·선택은 동시에 없다」).
2. `y` 대기(대기 시간 뒤) 중에 다시 `y`(접두)를 누르고 `Esc`: `ignore_replayed_operator`는 대기가 없을 때만 켜지므로 Esc가 비운 `y`가 재생되어 겹친 오퍼레이터(복사)가 실행될 수 있다.
3. 비주얼의 클립은 기준 칸의 클립인데 `d` `c`는 「선택이 현재 클립 밖이면 거부」(S3-T3a `edit_targets`)다. `v }`나 `v Ctrl+l Ctrl+l`로 커서가 클립을 벗어나면 거부되고, 거부 경로가 `end_visual`로 선택까지 지운다.
4. `y` 접두 재전송의 순서: `y g g`, `y 2 i n`, `y j`, `d y y`.
5. `.`의 `Selection` 대상: `v w d` 뒤 `.`은 지금 커서 노트에만 작용한다(vim은 같은 길이).
6. `c`가 노트 없이 Step으로 갈 때 `LastEdit`를 바꾸는가(계획 8절 「대상 없음은 바꾸지 않는다」).
7. 카운트가 `v` 같은 비모션 키를 지나 새는가.
8. 마지막 클립 마지막 노트의 `d w`(다음 노트 없음).
9. 뷰 왕복·`Cmd+A`·undo 뒤 비주얼 상태.
10. 대기 중 표준 키(`cmd-z`)와 메뉴 모션.
11. 기본 프리셋 회귀: 모든 이동 처리기가 `run_motion`을 거친다.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 메뉴·팔레트 Action과 대기 | 없음 | 시험 26은 대기 없는 팔레트 객체만 본다 |
| 2 `y` 대기 중 `y escape` | 없음 | 시험 11은 대기 없는 `y escape`만 |
| 3 비주얼이 클립을 벗어난 뒤 `d` | 없음 | 시험 15·18은 한 클립 안 |
| 4 `y` 접두 재전송 | 부분 | `y i b` `y w` `3 y w`만. `y g g` `y 2 i n` `y j` `d y y`는 리뷰에서 더함(모두 통과) |
| 5 비주얼 오퍼레이터 뒤 `.` | 없음 | 시험 21은 모션·객체만 |
| 6 빈 칸 `c`의 `.` 기억 | 없음 | |
| 7 카운트 누수 | 없음 | 리뷰에서 더함(통과) |
| 8 마지막 노트 `d w` | 없음 | |
| 9 뷰 왕복·`Cmd+A`·undo | 부분 | 시험 19가 `g a`·`g b`·`Shift`. `Cmd+A`·undo는 리뷰에서 더함(통과) |
| 10 대기 중 표준 키·메뉴 모션 | 없음 | |
| 11 기본 프리셋 | 있음 | 시험 25 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | `d` 대기 중 팔레트 `select::All` | 깨짐: `pending=Some(Delete)` + 선택 6개, 경로 `Normal > PianoRoll > Selection`, 상태줄 `NORMAL d` | `menu_select_all_while_pending_breaks_invariant` |
| 2 | `d w` 뒤 `d` 대기 중 메뉴 `edit::Undo` | 깨짐: undo가 되살린 D4를 선택, 대기 남음 | `menu_undo_while_pending_breaks_invariant` |
| 3 | `d` 대기 중 메뉴 `cursor::NextNote`(앞 카운트 3) | 못 깨뜨림(계획대로 작용, 3 × 1) | `menu_motion_while_pending_applies_operator` |
| 4 | D4에서 `v Ctrl+l Ctrl+l`(클립 끝 틈) 뒤 `d` | 깨짐: 「선택이 현재 클립에 없다」로 거부되고 선택(D4…A4)도 비주얼도 사라짐 | `visual_past_clip_end_then_delete_loses_selection` |
| 5 | D4에서 `v }` 뒤 `d` | 깨짐: 4와 같다 | `visual_to_next_clip_then_delete_refused` |
| 6 | `y`(대기 시간 넘김) → `y escape` | 깨짐: Esc가 대기 입력 `y`를 재생해 겹친 오퍼레이터가 커서 노트를 복사 | `yank_pending_then_y_escape_copies` |
| 7 | `y j` | 못 깨뜨림(커서·클립보드 그대로, 대기 끝) | `yank_prefix_then_pitch_motion_cancels` |
| 8 | `y g g` | 못 깨뜨림(C4·D4 복사, 커서 (0, 62)) | `yank_prefix_then_g_g` |
| 9 | `y 2 i n` | 못 깨뜨림(Step으로 새지 않음, D4 복사) | `yank_prefix_count_then_object` |
| 10 | `d y y` 빠르게 | 못 깨뜨림(복사 없음, 취소) | `delete_pending_then_y_y_cancels` |
| 11 | `d w` 뒤 `y y p` | 못 깨뜨림(복제, 대기로 새지 않음) | `yyp_still_duplicates` |
| 12 | `3 v w` | 못 깨뜨림(한 노트만 이동) | `count_before_visual_does_not_leak` |
| 13 | `v cmd-a` | 못 깨뜨림(비주얼 끝, 클립 전체, `NORMAL`) | `visual_cmd_a_ends_visual` |
| 14 | `v w d` 뒤 `.` | 계획대로(커서 칸에 노트가 없어 「없다」). 제안 2 | `dot_after_visual_delete_targets_cursor_note` |
| 15 | `v w g a g a` 뒤 `d` | 못 깨뜨림(선택도 비고 `d`는 대기) | `visual_view_round_trip_clears_selection` |
| 16 | 마지막 클립의 마지막 노트에서 `d w` | 계획대로(「대상이 없다」). 제안 1 | `delete_word_on_last_note_does_nothing` |
| 17 | `d cmd-z` | 계획대로(undo가 실행되지 않고 취소). 제안 4 | `pending_swallows_standard_keys` |
| 18 | `v w x` 뒤 `u` | 못 깨뜨림(비주얼 없음, `NORMAL`) | `visual_delete_undo_state` |
| 19 | `d w` 뒤 빈 칸 `c c` → `escape` → D4B에서 `.` | `.`이 `d w`가 아니라 `c c`(지우고 Step). 제안 3 | `change_on_empty_cell_replaces_last_edit` |

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 오퍼레이터 대기와 선택·비주얼은 동시에 없다 | 있다 | 대기 중 메뉴·팔레트 `select::All`·`edit::Undo`(시도 1·2). `vim_after_keystroke`는 키 관찰에서만 돌고 선택 처리기는 대기를 비우지 않는다 |
| 키 컨텍스트의 `Operator`는 대기와 함께 | 있다 | 위 상태에서 `pending`은 있는데 경로는 `Selection`(`vim.rs:171`). `i`가 Step으로, `i n`이 객체가 아니게 된다 |
| 비주얼이면 기준 칸이 있다 | 없다 | `set_anchor(None)`이 비주얼을 지우고 `set_visual`은 기준 칸 없으면 무시(`pianoroll/mod.rs`) |
| 문서는 `apply`로만, 클립 블록 불변 | 없다 | 오퍼레이터 삭제는 `plan_delete`. `c a c`·`d a c` 뒤 클립 길이 그대로(시험 23) |
| 모션 반복은 진행이 없으면 멈춘다 | 없다 | `motion_destination`의 `next == at` 검사, `E`는 클립 끝에서 `None` |
| 기본 프리셋 동작 불변 | 없다 | 카운트는 vim에만, `d` `v` 그대로(시험 25) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.8.1 `d` `y` `c` 오퍼레이터(`dw`) | 충족 | `src/vim.rs:352`(`start_operator`), `:491`(`apply_operator`), 시험 1 |
| 5.8.1 텍스트 객체 `dac` `yib` `vin` | 충족 | `src/pianoroll/vim.rs` `object_notes`, `src/vim.rs:720`, `assets/keymaps/vim.json` `Normal > Operator`·`Normal > Visual` |
| 5.8.1 `v` `V` `Ctrl+v` 비주얼 | 부분 충족 | `src/vim.rs:777`·`:807`. 비주얼 선택이 클립을 벗어나면 `d` `c`가 거부되고 선택을 잃는다(지적 3) |
| 5.8.1 `D` `C` | 충족 | `src/note_edit.rs` `bar_end_targets`, `src/vim.rs:585` |
| 5.8.1 모션 `w` `b` `W` `B` `e` `E` `f` `F` `;` `,` `gg` `G` `{` `}` | 충족 | `src/pianoroll/vim.rs` `motion_target`, `src/selection.rs` 처리기 → `run_motion` |
| 5.8.1·5.7 숫자 카운트 접두(`3l`, `d3w`) | 충족 | `src/vim.rs:95`·`:609`·`:640`, 시험 5·6 |
| 5.8.1 `.` 마지막 편집 반복 | 충족 | `src/note_ops.rs` `LastEdit::Operator`, 시험 21 |
| KM-5 카운트 접두는 vim 프리셋에서만 | 충족 | 시험 25 |
| P8 vim 오퍼레이터는 vim 프리셋에서만 | 충족 | `assets/keymaps/default.json` 변경 없음, 시험 25 |

「부분 충족」 1건(5.8.1 비주얼)은 지적 3으로 미충족으로 센다.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(변경은 UI 스레드 처리기뿐).
- 제품 코드의 `unwrap`/`expect`: `src/vim.rs:452` `unreachable!("위에서 돌려줬다")` 하나. 바로 위 `if target == OperatorTarget::Selection { return … }`가 있어 정당하다. 새 `expect` 없음.
- 에러가 삼켜지는 곳: `src/vim.rs:319` `let _ = me.update(…)`(창이 닫힌 뒤의 지연이라 무시가 맞다). `src/vim.rs:540`·`:553` 삭제 거부 때 `false`를 돌려주며 비주얼을 끝내지 않는다(`delete_notes`가 이미 알림). 동작은 일관되지 않지만 문서 손상은 없다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 오퍼레이터 대기가 키 입력 밖의 선택 변경으로 끝나지 않는다. 대기 중 메뉴·팔레트로 `select::All`이나 `edit::Undo`를 실행하면 대기와 선택이 공존한다. 상태줄은 `NORMAL d`인데 키 컨텍스트에는 `Operator`가 없어 `i`가 Step으로 가고 `i n`은 객체가 아니다. 다음 모션은 선택이 아니라 모션 범위를 지운다(7절 「선택이 있으면 바로」와도 어긋남). 계획 「불변식」의 「오퍼레이터 대기와 비주얼은 동시에 없다(선택·비주얼이 있으면 대기에 들지 않는다)」 위반이다. 재현: `menu_select_all_while_pending_breaks_invariant`, `menu_undo_while_pending_breaks_invariant`. 고치는 길 예: 선택이 비지 않게 되는 곳(`set_selection`/`sync_selection_mode`)에서 대기를 비우거나, 메뉴·팔레트 디스패치(`dispatch::run_action`)가 대기를 끝낸다 | `crates/keyscore-app/src/vim.rs:328`, `:171`; `crates/keyscore-app/src/selection.rs:127` |
| 2 | 수정 | `y` 대기(대기 시간 뒤) 중 `y escape`가 복사를 실행한다. 대기 분기는 대기 입력이 있을 때 Esc를 키맵에 넘기기만 하고(`return false`), 키맵이 대기 입력을 비우며 재생한 `operator::Yank`가 같은 오퍼레이터라 겹친 오퍼레이터로 커서 노트를 복사한다. 대기 없는 `y escape`는 `ignore_replayed_operator`로 막는데 대기 있는 경로는 빠졌다. 계획 2절 「대기 입력이 `y`일 때의 `Esc`는 `y`를 실행하지 않고 취소한다」와 A13(Esc는 시퀀스만 취소) 위반. `d` 대기에서 `y escape`는 재생된 `y`가 「다른 오퍼레이터」로 대기까지 취소한다(A13은 대기를 남긴다). 재현: `yank_pending_then_y_escape_copies` | `crates/keyscore-app/src/vim.rs:264`–`267` |
| 3 | 수정 | 비주얼 선택이 기준 칸의 클립에 남은 채 커서가 그 클립을 벗어나면(`v }`, `v Ctrl+l Ctrl+l`) `d` `c`는 「선택이 현재 클립에 없다」로 거부되고, 거부 경로가 `end_visual`로 선택과 기준 칸까지 지운다. S3-T3a는 거부 때 선택을 남긴다(`selection.rs` `apply_note_op` 주석 「선택 자체는 남는다」). 결정 3 A 「선택이 있을 때 d y c는 선택에 바로 작용」과 계획 6절(클립 = 기준 칸의 클립)·7절(현재 클립 밖이면 거부)이 겹친 결과다. 적어도 거부 때 선택·비주얼을 남겨야 하고, `v }` `d`처럼 vim에서 자연스러운 입력이 늘 거부되는 것은 계획에 기록하거나(사람 판단) 비주얼 중에는 선택의 클립을 대상으로 허용할지 정해야 한다. 재현: `visual_past_clip_end_then_delete_loses_selection`, `visual_to_next_clip_then_delete_refused` | `crates/keyscore-app/src/vim.rs:504`–`509`, `:821`; `crates/keyscore-app/src/note_edit.rs:91` |
| 4 | 제안 | 마지막 클립의 마지막 노트에서 `d w`(다음 노트 없음)는 「대상이 없다」로 아무것도 지우지 않는다. vim `dw`는 마지막 단어도 지운다. 계획 4절 글자 그대로라 기록만 한다. 앞으로 가는 배타 모션의 목적지가 없을 때 「현재 클립 끝」을 목적지로 보는 안을 검토할 만하다. 재현(통과, 기록): `delete_word_on_last_note_does_nothing` | `crates/keyscore-app/src/vim.rs:436`–`439` |
| 5 | 제안 | 비주얼 오퍼레이터의 `.`은 `OperatorTarget::Selection`이라 지금 선택(없으면 커서 노트)에 작용한다. `v w d` 뒤 커서가 빈 칸이면 `.`은 「지울 노트가 없다」다. vim은 같은 길이를 되풀이한다. 계획 8절 글자 그대로. 재현(통과, 기록): `dot_after_visual_delete_targets_cursor_note` | `crates/keyscore-app/src/vim.rs:375`–`377` |
| 6 | 제안 | 노트가 없는 `c`(빈 칸 `c c`, 빈 범위 `c l`)도 `LastEdit`를 바꾼다. 계획 8절 「실패한 오퍼레이터(대상 없음)는 바꾸지 않는다」의 「대상 없음」에 드는지 계획이 정하지 않았다. 지금은 직전 `d w`가 사라지고 `.`이 「커서 노트 지우고 Step」이 된다. 어느 쪽이든 계획에 한 줄 적기를 권한다. 재현(통과, 기록): `change_on_empty_cell_replaces_last_edit` | `crates/keyscore-app/src/vim.rs:552`–`557` |
| 7 | 제안 | 대기 중에는 `cmd-z`·`cmd-s`·`?`(오버레이)·`ctrl-space`(재생) 같은 표준·전역 키도 실행되지 않고 대기만 취소된다(계획 2절 글자 그대로). vim과 같지만 `cmd` 표준 키(KM 표준 레이어)는 대기를 취소하고 실행하게 두는 쪽이 사용자 기대에 가깝다. 재현(통과, 기록): `pending_swallows_standard_keys` | `crates/keyscore-app/src/vim.rs:269`–`281` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1·2·3(수정)을 고치고 재현 시험 5개(`review_s3_t3c.rs`의 실패 시험)가 통과하면 재리뷰한다. 제안 4–7은 거절할 수 있으며 거절 이유를 보고서에 적는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`review_s2_t2_r2.rs`·`review_s2_t2_r3.rs`의 모델 변경은 GPUI `resolve_bindings_for_input`(gpui-pre 0.3.8 `keymap.rs:192`)의 「`NoAction`이 같거나 약한 출처의 뒤 순위 바인딩을 가린다」와 맞는다. 같은 레이어에서 깊은 `null`이 얕은 바인딩을 가리는 경우를 바르게 모델링하며 단언을 약하게 하지 않았다. 두 시험 파일은 통과한다)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
