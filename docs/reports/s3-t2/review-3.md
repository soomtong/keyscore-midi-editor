# 적대적 리뷰: S3-T2 커서·선택·수식키 체계 (리뷰 3회차)

- 리뷰어 세션: 구현자와 다른 세션(구현 맥락 없음. 2회차 리뷰·보고서 「리뷰 2 대응」·수정 커밋 diff·제품 코드만 읽음)
- 대상 커밋: `97eebee` (수정 범위 `git diff 8cc5d26..97eebee`, 커밋 `ac39bca` `8eba899` `efbc52b` `97eebee`)
- 범위: 2회차 지적 1–6의 수정, 수락한 제안 8·9. 판정 기준은 2회차 수정 범위의 회귀·결정 위반·결함. 그 밖의 새 경계는 「제안」
- 리뷰 테스트: `crates/keyscore-app/tests/review_s3_t2_r3.rs`

## 1. 코드만 읽고 적은 「내가 검사할 것」

테스트 파일을 열기 전에 수정 커밋의 diff와 `selection.rs`(`extend`·`apply_note_op`·`follow_cursor_note`·`shift_anchor`·`cursor_moved`·`clip_at`), `pianoroll/select.rs`, `pianoroll/ops.rs`(`plan_note_op`), `pianoroll/mod.rs`(`move_cursor`), `window.rs`(`apply_mode_event`·`set_selection`), `keymap/mod.rs`(`build`·`uses_reserved_key`·`save_user`·`apply`·`canonical_keys`·`swap_alt_cmd`), `keymap/user.rs`(`bind`·`clear`·`restore`·`file_keys`), `settings_window.rs`(`finish_capture`·`bind`·`edit_user`·`confirm_sheet`·`import_from`), `core::ClipId`(전역 카운터), GPUI `Keystroke::parse`(gpui-pre 0.3.8 `platform/keystroke.rs:120–189`)를 읽고 적었다.

1. 지적 1(넓히기는 좁히지 않는다) 우회와 모순. `extend`는 `picked`의 클립이 지금 선택의 클립과 같을 때만 합친다(`current.clip() == Some(clip_id)`). (a) 2회차 시도 6·7의 경로(조작 뒤 변위가 커서 쪽)가 닫혔는가. (b) 같은 축으로 되돌아오는 `Shift+l` → `Shift+h`가 선택을 줄이지 않는가(규칙대로면 유지). (c) `b`로 만든 선택 뒤 빈 칸으로 넓히기가 선택을 지우지 않는가(구현자 시험 10의 새 기대값). (d) 선택을 유지한 채 수식키 없는 이동으로 클립 사이 틈에 간 뒤 `Shift+l`: `picked`가 `None`이면 `set_selection(Selection::default())`로 선택을 비운다. 「넓히기는 좁히지 않는다」와 모순인가(규칙의 글자는 「같은 클립의 지금 선택」이라 클립이 없으면 적용 밖). (e) 선택이 A, 커서가 B(수식키 없는 이동), `Shift+l`: `picked`=B, 선택이 B의 노트로 바뀌어 A가 떨어진다. 클립이 다르니 규칙 밖이지만 5.11 「선택 유지」 흐름과 겹친다. (f) `Esc`·뷰 전환이 선택과 기준 칸을 함께 지우는가(`select_clear`, `apply_mode_event`). (g) 합칠 때 트랙을 보지 않는데 `ClipId`가 전역 카운터라 문제없는가.
2. 지적 2(기준 칸·커서 모두 틈이면 직사각형의 첫 클립) 모순. 세 규칙의 우선순위가 「기준 칸 클립 → 커서 클립 → 첫 클립」이라 기준 칸이 틈이고 커서가 A → B → B 뒤 틈으로 가면 고르는 클립이 A → B → (첫 클립) A로 뒤집히는가. 그때 지금 선택(B)과 클립이 달라 합치지 않고 B의 선택이 떨어지는가(「넓히기」가 선택을 바꿔치기). 직사각형이 두 클립을 덮을 때 뒤 클립의 노트는 무시되는가(설계대로라면 문서에 있어야 한다).
3. 지적 3(격자 되돌림) 경계. `moved_tick > abs_last`일 때만 한 스텝씩 물러나고 `max(abs_start)`. (a) 격자 위에서 `Shorten` 연타가 격자를 지키며 계속 적용되는가(2880 → 1920 → 960). (b) 변위가 음수(`NudgeEarlier`·`NudgeBarEarlier`)일 때 커서가 노트 시작 앞으로 가면 시작으로 당기는가. (c) 노트가 스텝보다 짧을 때(`[0, 480)`) `NudgeLater`·`Lengthen` 뒤 커서가 노트 안인가. (d) 노트 시작이 격자 밖(`[100, 1500)`)이고 커서가 격자 위(960)에서 `Shorten`하면 물러난 뒤 `max(abs_start)`로 100에 가 격자를 벗어난다(계획에 「그래도 시작 앞이면 시작」으로 적혀 있음). (e) 그 뒤 `h`/`l`.
4. 지적 4(그대로 둔 노트의 선택 유지). 선택 {C4 960, D4 1920} + `<` → 선택 {C4, D4'}이고 이어서 `>`가 둘에 가는가. `OctaveUp`에서 하나가 밖이면 전무 거부 뒤 선택 불변인가. `moved`의 `from`과 같은 값의 다른 노트가 선택에 있을 수 없는가(`BTreeSet`이라 없음).
5. 지적 5(`Keystroke::parse` 기반 예약 키). `parse`는 수식키 토큰을 떼고 남은 키를 `make_ascii_lowercase`하므로 `F20`·`shift-f20`·`ctrl-F20`·`fn-f20`·`cmd-f20`·시퀀스 `g F20`이 모두 `key == "f20"`이다. 각각 KM-6 오류가 나고 `g` 뒤 `escape`가 아무 Action도 내지 않는가. 수식키 교환이 켜져도 같은가(`swap_alt_cmd`는 키 이름을 바꾸지 않음). `f20->x`(key_char 표기)도 잡히는가. `build`의 검사가 `KeyBinding::load` 앞이라 오류 두 개가 나지 않는가.
6. 지적 6(`save_user` 거부)의 파급. `save_user`는 쓰기 전에 `user.bindings()` 전체를 훑는다. (a) press-to-bind 경로(`UserKeymap::bind("x", …, "f20")` → `save_user`)가 `Err`이고 파일·`.bak`·`errors`가 모두 그대로인가. (b) 가져오기(`import_from` → `save_user`)에 `F20`이 들어 있으면 거부 메시지가 나고 파일이 그대로인가. (c) 손으로 쓴 `keymap.json`에 이미 `f20` 항목이 있으면(핫리로드는 KM-6 오류 하나) 편집기의 모든 쓰기(`bind` 다른 키, `clear`)가 그 항목 때문에 거부되는가. 그 `f20` 항목은 `build`가 뺐으니 실효 바인딩에 없고 `clear`는 실효 바인딩만 지우므로 편집기로는 못 없애는가(`restore`는 이름으로 지우니 가능). 메시지가 사용자가 누른 키가 아니라 파일의 `f20`을 가리켜 혼란스럽지 않은가. (d) `set_swap_alt_cmd`가 `save_user` 실패를 조용히 넘기고 `apply`만 하는가(기존 동작).
7. 제안 8(리뷰 시험을 실패 상태로 먼저 커밋): `ac39bca`(시험만)에서 `review_s3_t2_r2` 7개가 실패하고 `8eba899`·`efbc52b` 뒤 13개가 통과하는가(worktree로 확인).
8. 제안 9: 계획 1의 한 줄과 거부 메시지의 `Shift+p`·`Cmd+←` 안내.
9. 회귀: `pianoroll_select`(CoreMIDI 둘은 단독), `command_line`, `review_s3_t2`, `review_s3_t2_r2`, `settings_keymap`, `review_s2_t6a*`, `keymap_*`. 기존 시험 삭제·`#[ignore]` 없음(수정 범위의 시험 변경은 `pianoroll_select.rs` 시험 10 기대값 하나).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1(a) 조작 뒤 변위가 커서 쪽 | 있음(`review_s3_t2_r2` 시도 6·7, 이제 통과) | 닫힘 |
| 1(b) 같은 축으로 되돌아오는 넓히기 | 없음 | 시도 3, 못 깨뜨림(규칙대로 유지) |
| 1(c) `b` 뒤 빈 칸 넓히기 | 있음(`pianoroll_select` 시험 10 꼬리) | 시도 3 뒷부분, 못 깨뜨림 |
| 1(d) 선택 유지 + 틈에서 넓히기 | 없음 | 시도 2에서 깨짐(선택이 빈다). 규칙의 글자 밖이라 제안 |
| 1(e) 선택 A·커서 B에서 넓히기 | 없음 | 시도 1의 셋째 단계가 같은 경로(A가 떨어지고 B로 바뀜). 설계대로 보이나 계획에 글자가 없다(제안) |
| 1(f) `Esc`·뷰 전환과 기준 칸 | 없음 | 시도 4: 뷰 전환은 둘 다 지움(못 깨뜨림). 빈 선택의 기준 칸은 Esc가 안 지움(깨짐, 수정 범위 밖이라 제안) |
| 1(g) `ClipId` 전역성 | 해당 없음 | `core/clip.rs:10–16` 전역 카운터. 문제없음 |
| 2 틈 기준 칸의 클립 뒤집힘 | 없음(`review_s3_t2_r2` 시도 4는 A를 덮지 않는 픽스처) | 시도 1에서 깨짐 |
| 3(a) `Shorten` 연타·격자 | 부분(`review_s3_t2_r2` 시도 5는 한 번 + `l`) | 시도 5: 세 번 연타·거부·undo 한 단위·`l`, 못 깨뜨림 |
| 3(b)(c) 음수 변위·짧은 노트 | 없음 | 시도 6 앞부분, 못 깨뜨림 |
| 3(d)(e) 격자 밖 시작 | 없음 | 시도 6 뒷부분: 계획 글자대로(시작으로), 못 깨뜨림 |
| 4 그대로 둔 노트·다음 조작·전무 거부 | 부분(`review_s3_t2_r2` 시도 8은 선택만 본다) | 시도 7, 못 깨뜨림 |
| 5 `F20` 표기 전부·수식키 교환 | 부분(`review_s3_t2` 시도 7 소문자, `_r2` 시도 9 대문자) | 시도 8: 7가지 표기 + 교환, 못 깨뜨림 |
| 6(a)(b) `save_user`·가져오기 거부와 파일 불변 | 부분(`review_s3_t2_r2` 시도 10은 `is_err`만) | 시도 9, 못 깨뜨림 |
| 6(c) 손으로 쓴 f20이 편집기 쓰기를 막음 | 없음 | 시도 10에서 깨짐. 시도 11: 그 Action의 지우기·복원으로는 빠져나온다(못 깨뜨림) |
| 6(d) `set_swap_alt_cmd`의 조용한 실패 | 없음 | 코드만 봄(`keymap/mod.rs:454–461`). 기존 동작, 실행하지 않아 표에 넣지 않음 |
| 7 제안 8 커밋 순서 | 해당 없음 | worktree에서 확인(3절 끝) |
| 8 제안 9 | 없음 | 시도 12, 못 깨뜨림 |
| 9 회귀 | 해당 없음 | 3절 끝 |

## 2회차 지적 처리 확인

| 2회차 | 대응 커밋 | 확인 | 결과 |
| --- | --- | --- | --- |
| 1 수정: 조작 뒤 넓히기가 선택을 줄임 | `8eba899` `extend`가 같은 클립의 지금 선택과 합침(`selection.rs:195–201`), 계획 1 | `_r2` 시도 6·7 통과, 시도 3(이 문서) | 고쳐짐. 규칙은 같은 클립 안에서 선다. 클립이 없는 자리(시도 2)·다른 클립(시도 1)에서는 선택이 비거나 바뀌는데 계획에는 「좁히려면 Esc」만 있다 |
| 2 수정: 기준 칸·커서 모두 틈이면 빈 선택 | `8eba899` 직사각형의 첫 클립(`selection.rs:173–185`) | `_r2` 시도 4 통과, 시도 1(이 문서) | 부분. 세 규칙의 우선순위가 커서 위치에 따라 바뀌어 틈 → A → 틈 → B → 틈에서 고르는 클립이 A·A·B·A로 뒤집히고 B의 선택이 떨어진다 |
| 3 수정: 당긴 커서가 격자 밖 | `8eba899` 한 스텝씩 물러남(`selection.rs:433–440`), 계획 2 | `_r2` 시도 5 통과, 시도 5·6(이 문서) | 고쳐짐. 격자 밖 시작 노트는 시작으로(계획 글자대로) |
| 4 수정: 그대로 둔 노트가 선택에서 빠짐 | `8eba899` 선택 = (대상 − 바뀐 원래 값) ∪ 새 값(`selection.rs:397–404`) | `_r2` 시도 8 통과, 시도 7(이 문서) | 고쳐짐 |
| 5 수정: `F20` 우회 | `efbc52b` `uses_reserved_key`(`keymap/mod.rs:254–257`) | `_r2` 시도 9 통과, 시도 8(이 문서) | 고쳐짐. 수식키가 붙은 `f20`(`shift-f20` 등)도 거부하는데 가로채기는 수식키 없는 `f20`만 디스패치하므로 필요 이상으로 넓다. 해롭지 않아 지적하지 않음 |
| 6 수정: 편집기가 f20을 파일에 씀 | `efbc52b` `save_user`가 쓰기 전 전체 바인딩을 훑어 거부(`keymap/mod.rs:561–568`) | `_r2` 시도 10 통과, 시도 9·10·11(이 문서) | 부분. 새 바인딩은 거부되고 파일·`.bak`은 그대로다. 그러나 손으로 쓴 파일에 f20이 이미 있으면 다른 Action의 편집도 모두 거부된다(수정 전에는 됐다) |
| 7 누락 시험 | `ac39bca` `review_s3_t2_r2.rs` | 재실행 13 통과 | 처리됨 |
| 8 제안: 시험을 실패 상태로 먼저 커밋 | `ac39bca`(시험·문서) → `8eba899`·`efbc52b`(수정) | worktree `ac39bca`에서 `_r2` 실행: 6 통과 / 7 실패(2회차 문서와 같은 7개). HEAD에서 13 통과 | 수락대로 |
| 9 제안: 경계 넘은 넓히기 뒤 거부 안내 | `8eba899` 메시지에 `Shift+p·Cmd+←`, 계획 1 한 줄 | 시도 12 | 수락대로 |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-app/tests/review_s3_t2_r3.rs`에 있고 `cargo test -p keyscore-app --test review_s3_t2_r3`로 실행했다(CoreMIDI를 쓰지 않는다). 「깨짐」은 단언이 실패한 것이다. 실행 결과 12개 가운데 8 통과 / 4 실패. 파일은 `rustfmt --check`와 `cargo clippy --test review_s3_t2_r3 -- -D warnings`를 통과한다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 클립 A(1–2마디, C4 960)·B(3–4마디, C4 480). 틈(0)에서 `shift-l` ×4 → A 안(A의 C4) → ×4 → A 뒤 틈(첫 클립 규칙으로 A 유지) → ×4 → B 안(커서 클립 규칙으로 B, A의 선택은 떨어짐) → ×4 → B 뒤 틈 | 깨짐: 마지막 한 스텝에 선택 클립이 B에서 A로 바뀐다(`[0, 16320)`의 첫 클립 = A). 직사각형은 여전히 B의 노트를 덮는데 B의 선택이 사라지고 A의 노트가 선택된다 | `brk3_shift_extend_from_gap_flips_picked_clip_a_b_a` |
| 2 | `b`로 1마디 노트 3개 선택 → 커서를 B 뒤 틈(6마디)으로 → `l`(선택 유지, 5.11) → `shift-l` | 깨짐: 직사각형이 어느 클립에도 걸치지 않아 `picked`=`None`, 선택 0개. 「넓히기는 좁히지 않는다」가 클립 없는 자리에서는 서지 않는다 | `brk3_shift_extend_in_gap_with_kept_selection_clears_it` |
| 3 | 노트 4개(0·960·1920·2880). `shift-l` ×3(4개) → `shift-h` ×3(되돌아옴) → `shift-k` → `Esc` → `b` → `alt-k` → `shift-l` | 못 깨뜨림: 되돌아와도 4개, 다른 축도 4개, Esc는 선택·기준 칸 둘 다 비움, `b` 뒤 조작 뒤 넓히기도 4개, 기준 칸은 옮기기 전 커서 (0, 61) | `brk3_shift_extend_back_and_after_bar_select_does_not_shrink` |
| 4 | `shift-l`로 D4 선택 → `v`(뷰 전환) → 선택·기준 칸 확인 → 피아노롤로 → 빈 칸에서 `shift-l` ×2(선택 빔, 기준 칸 남음) → `Esc` | 반은 못 깨뜨림(뷰 전환은 선택·기준 칸을 함께 지움), 반은 깨짐: 선택이 비면 `Selection` 컨텍스트가 없어 Esc가 `select::Clear`에 닿지 않고 기준 칸 (9600, 40)이 남는다. 다음 `Shift+이동`은 옛 기준 칸에서 넓힌다 | `brk3_view_toggle_clears_anchor_but_escape_with_empty_selection_keeps_it` |
| 5 | 노트 `[0, 3840)`, 커서 2880. `<` ×4: 2880 → 1920 → 960 → 0, 네 번째는 「한 스텝 이하」 거부·커서 유지. `cmd-z` 한 번으로 3840 복원, `l` → 960 | 못 깨뜨림 | `brk3_shorten_repeat_keeps_grid_until_one_step_then_rejects` |
| 6 | 짧은 노트 `[0, 480)`에 `>` `alt-l` `<` `<`(거부) `alt-h`: 커서 늘 노트 안·격자 위. 격자 밖 시작 `[3940, 5340)`에 커서 4800으로 `<` → 한 스텝 물러난 3840이 시작 앞이라 3940(시작), 그 뒤 `l`은 4900 | 못 깨뜨림(계획 2 글자대로. 노트가 격자 밖이면 커서도 밖인데 `n`/`p`도 같다) | `brk3_cursor_follow_with_short_note_and_off_grid_start` |
| 7 | 선택 {C4 960, D4 1920} + `<` → {C4, D4 960} → `>` → 둘 다 1920. 선택 {C4, 120} + `alt-shift-k` → 전무 거부, 문서·선택 불변 | 못 깨뜨림 | `brk3_shorten_keeps_short_note_then_lengthen_hits_both_and_all_or_nothing_keeps_selection` |
| 8 | 사용자 키맵 `shift-f20` `ctrl-F20` `fn-f20` `cmd-f20` `alt-F20` `g F20` `f20->x`(7개), vim 프리셋 → 오류 7개 모두 「예약」 문구. 수식키 교환을 켜도 7개. `g` 뒤 `escape`는 뷰 그대로, `g a`는 산다 | 못 깨뜨림 | `brk3_every_spelling_of_f20_is_rejected_even_with_swap` |
| 9 | 손으로 쓴 파일(주석 포함, `ctrl-alt-u`) 위에 `UserKeymap::bind(…, "F20")` → `save_user` | 못 깨뜨림: `Err`에 f20, 파일 글자 그대로, `.bak`·`.tmp` 없음, 엔진 오류 0. 가져오기와 같은 경로(`parse` → `save_user`)도 `Err`·파일 불변 | `brk3_save_user_with_f20_leaves_file_bak_and_errors_untouched` |
| 10 | 손으로 쓴 파일에 `f20: view::Toggle` 하나(핫리로드는 KM-6 오류 1). 설정 창에서 `track::ToggleMute`에 press-to-bind `ctrl-alt-u` | 깨짐: 메시지 「키 'f20'는 쓸 수 없다. 'f20'는 앱이 시퀀스 취소에 예약한 키다」, 파일 불변(`ctrl-alt-u` 없음). 사용자가 누른 키와 무관한 거부. 수정 전 커밋에서는 썼다(worktree 확인은 `_r2` 시도 10으로 대신함: `save_user`가 `Ok`였다) | `brk3_hand_written_f20_blocks_every_editor_write` |
| 11 | 같은 파일에서 `view::Toggle`을 골라 `backspace`, 그리고 다시 써서 `r` | 못 깨뜨림: 둘 다 f20 항목을 지우고 저장된다(실효 바인딩은 `last_user`로 계산해 build가 뺀 항목도 든다). 빠져나올 길은 「그 Action을 골라 지우기·복원」이다 | `brk3_hand_written_f20_is_removed_by_clear_or_restore_of_that_action` |
| 12 | 경계 넘은 `shift-l` 뒤 `alt-k` 거부 메시지에 `Shift+p`·`Cmd+←`, 안내대로 `shift-p` 뒤 적용됨. 계획 1의 세 문장 | 못 깨뜨림 | `brk3_rejection_message_names_shift_p_and_cmd_left` |

실행하지 않은 것(적지 않음): `set_swap_alt_cmd`의 `save_user` 실패 경로, 음높이 끝(0·127)에서의 `shift_anchor` clamp(2회차와 같음).

수정 전 비교(제안 8): `git worktree add /tmp/ks-r3-before ac39bca`에서 `cargo test -p keyscore-app --test review_s3_t2_r2` → 6 통과 / 7 실패(2회차 문서의 7개와 같다). HEAD에서는 13 통과. worktree는 지웠다. 주의: 같은 `target/`을 쓰면 worktree의 빌드가 본 트리의 산출물을 덮어 그 뒤의 본 트리 시험이 옛 코드로 돈다(한 번 겪고 `touch`로 다시 빌드했다). 다음 리뷰어는 `CARGO_TARGET_DIR`를 따로 둔다.

구현자·기존 시험 재실행(모두 통과): `pianoroll_select` 12(CoreMIDI 둘 제외) + `cursor_move_onto_note_sends_preview_on_track_port_channel`·`play_from_cursor_starts_at_piano_roll_cursor` 각각 단독, `command_line` 7, `review_s3_t2` 13, `review_s3_t2_r2` 13, `settings_keymap` 9, `review_s2_t6a` 21 / `_r2` 13 / `_r3` 8 / `_r4` 5, `keymap_engine` 11, `keymap_overlay` 4, `keymap_source` 4, `keymap_user` 4.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 아니오 | 수정은 `plan_note_op`·`map_notes`를 바꾸지 않았다 |
| 노트는 클립 안 | 아니오 | 같음 |
| 틱 ≥ 0 | 아니오 | `follow_cursor_note`의 `max(abs_start)`·`max(0)`, `shift_anchor`의 `max(0)`. 시도 5·6 |
| 채널 1–16 | 해당 없음 | 채널을 만지지 않는다 |
| 음높이 0–127 | 아니오 | 시도 7의 전무 거부. `shift_anchor`의 clamp 그대로 |
| 선택의 노트는 실제 노트 | 부분적으로 예(해롭지 않음, 1·2회차와 같음) | `Selection::set`·`set_selection`은 `pub`. 조작 시점에 (트랙, 클립) 일치와 `map_notes`의 `NoteNotFound`로 막힌다. 새 합치기(`notes.extend(current.notes())`)는 같은 클립의 지금 선택을 더하므로 없는 노트를 만들지 않는다(문서가 바뀌면 `selection_document_changed`가 뺀다) |
| 선택 유무 = FSM `Selection` 컨텍스트 | 아니오 | `replace_selection`·`set_selection` 한곳. 시도 2·3·4에서 `has_selection`과 선택이 일치 |
| 커서는 스텝 격자의 한 칸(5.3) | 격자 위에서 시작하면 아니오 | 시도 5·6. 노트 시작이 격자 밖이면 커서가 그 시작으로 간다(`n`/`p`와 같은 태도, 계획 2에 적혀 있음) |
| 선택의 클립은 하나 | 아니오 | `Selection`은 (트랙, 클립, 노트) 하나. 시도 1은 클립이 「바뀌는」 것이고 둘이 섞이지는 않는다 |

## 5. REQ 대조

2회차 5절에서 이번 수정이 닿는 행만 다시 본다. 나머지는 1·2회차 판정 그대로다.

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.3 「커서는 (틱, 음높이) 한 칸」 | 충족 | `selection.rs:433–440`, 시도 5·6. 2회차 부분 충족이 닫혔다 |
| 5.3 「앞뒤 클립의 노트는 … 편집은 되지 않는다」 | 충족 | 2회차 그대로, 시도 12 |
| 5.7 표 `Shift` 행 「선택 넓히기」 | 부분 충족 | 같은 클립 안에서는 넓히기만 한다(`selection.rs:195–201`, 시도 3). 기준 칸이 틈이면 커서가 틈을 지날 때 선택 클립이 뒤집혀 선택이 바뀌어 친다(`selection.rs:173–185`, 시도 1). 클립 없는 자리의 넓히기는 지금 선택을 비운다(`selection.rs:205`, 시도 2) |
| 5.7 표 `Alt`·`Alt+Shift` 행(대상은 선택) | 충족 | `selection.rs:397–404`, 시도 7. 2회차 부분 충족이 닫혔다 |
| 5.7 「같은 조작을 연속으로」 | 충족 | 시도 5(`<` ×3 → `cmd-z` 한 번) |
| 5.8 `Esc` + A13 시퀀스 대기 중 Esc | 충족 | `keymap/mod.rs:254–257`, 시도 8. 2회차 부분 충족(`F20`)이 닫혔다 |
| 5.8 `Esc` 「닫을 것이 없으면 아무 일도 없다」 | 충족(글자대로) | 시도 4: 선택이 비면 Esc는 아무 일도 하지 않는다. 남은 기준 칸을 「닫을 것」으로 볼지는 REQ에 없다(제안) |
| KM-3 press-to-bind·`Backspace`·`r` | 부분 충족 | `keymap/mod.rs:561–568`. 파일에 f20 항목이 있으면 다른 Action의 press-to-bind가 거부된다(시도 10). 지우기·복원은 된다(시도 11) |
| KM-6 키맵 오류 보고 | 충족 | 거부 메시지가 예약 키를 말하고 파일에 쓰지 않는다(시도 9). 2회차의 거짓 성공은 닫혔다 |
| KM-9 가져오기 | 충족 | `import_from` → `save_user` 거부·파일 불변(시도 9 뒷부분) |
| 5.11 「선택을 유지한 채」 | 충족 | 수식키 없는 이동은 선택을 유지(시도 2 앞부분) |

「부분 충족」은 미충족으로 센다: 5.7 `Shift` 행(시도 1·2), KM-3(시도 10).

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 수정은 `keyscore-midi`·`preview.rs`를 건드리지 않았다(`git diff --stat 8cc5d26..97eebee`의 7개 파일은 `keymap/mod.rs`·`selection.rs`·시험 둘·문서 셋).
- 제품 코드의 `unwrap`/`expect`(수정 범위): 없음. `uses_reserved_key`는 `is_ok_and`, `follow_cursor_note`는 `unwrap_or(u64::MAX)`(clamp 뒤라 도달하지 않음, 2회차와 같음).
- 에러가 삼켜지는 곳:
  - `extend`의 첫 클립 규칙 `or_else`: `current_track()`이 `None`이면 `None`으로 떨어져 선택을 비운다. 트랙이 없으면 선택할 것도 없으니 정당.
  - `save_user`의 거부는 `Err`로 돌아오고 편집기 세 경로(`bind`·`edit_user`·`confirm_sheet`·`import_from`)가 모두 메시지로 보인다. `set_swap_alt_cmd`만 `is_ok()`로 삼키고 `apply`한다(기존 동작, 실행하지 않음).
  - `uses_reserved_key`는 `Keystroke::parse` 실패를 「예약 키 아님」으로 본다. 그 뒤 `KeyBinding::load`가 같은 토큰을 거부해 오류가 난다. 정당.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 2회차 지적 2의 수정이 새 결함을 만들었다. 클립을 고르는 세 규칙(기준 칸 클립 → 커서 클립 → 직사각형의 첫 클립)이 커서 위치에 따라 바뀌어, 기준 칸이 틈이면 커서가 틈 → A → 틈 → B → 틈으로 갈 때 고르는 클립이 A·A·B·A로 뒤집힌다(시도 1). 마지막 한 스텝에 B의 선택이 사라지고 A의 노트가 선택된다. 「넓히기는 좁히지 않는다」(지적 1의 규칙)와도 어긋난다. 2회차가 적은 다른 길 「선택이 비어 있지 않으면 선택의 클립을 쓴다」를 첫 클립 규칙 앞에(또는 대신) 두면 A·A·B·B가 되고 시도 1·`_r2` 시도 4가 모두 선다. 계획 1에 한 줄 | `crates/keyscore-app/src/selection.rs:168–185` |
| 2 | 수정 | 2회차 지적 6의 수정이 편집기 경로를 좁혔다. `save_user`가 쓰기 전에 파일의 모든 바인딩을 훑어 예약 키가 하나라도 있으면 거부하므로, 손으로 쓴 `keymap.json`에 `f20` 항목이 이미 있으면(핫리로드는 KM-6 오류 한 줄) 다른 Action의 press-to-bind·지우기가 모두 「키 'f20'는 쓸 수 없다」로 거부된다(시도 10). 수정 전 커밋에서는 썼다(`_r2` 시도 10의 `Ok`). 메시지는 사용자가 누른 키가 아니라 파일의 항목을 가리키고, 편집기가 쓰지 않았다는 말도 없다. `current_layers`는 문법 오류만 「고쳐 쓰지 않는다」로 다루므로 정책도 어긋난다. 길은 둘 가운데 하나: (a) 이번 편집이 새로 넣는 바인딩(`UserKeymap::bind`의 `keys`)만 거부한다(2회차가 적은 첫 선택지), (b) 지금처럼 전체를 거부하되 메시지를 「keymap.json을 고쳐 쓰지 않는다: 'f20' 항목이 있다(view::Toggle을 골라 지우거나 복원한다)」로 바꾼다. (a)가 2회차 지적 6의 취지에 맞다 | `crates/keyscore-app/src/keymap/mod.rs:561–568`, `settings_window.rs:557–580`(`bind`) |
| 3 | 수정 | 누락 시험: 위 1·2. `review_s3_t2_r3.rs`의 실패 시험 `brk3_shift_extend_from_gap_flips_picked_clip_a_b_a`·`brk3_hand_written_f20_blocks_every_editor_write`가 그대로 쓸 수 있다. 고친 뒤 이 파일을 PR에 두고 `brk3_` 접두 그대로 통과시킨다. 제안으로 분류한 시도 2·4의 시험은 기대값을 구현자가 고를 수 있다(거절하면 그 시험의 기대값을 지금 동작으로 바꾸고 이유를 보고서에 적는다) | `crates/keyscore-app/tests/review_s3_t2_r3.rs` |
| 4 | 제안 | 「넓히기는 좁히지 않는다」의 경계가 계획에 없다. (a) 선택을 유지한 채 클립 없는 자리에서 `Shift+이동`하면 선택이 빈다(시도 2). (b) 선택이 A인데 커서가 B에서 `Shift+이동`하면 선택이 B의 노트로 바뀐다(시도 1의 셋째 단계). 둘 다 설계로 둘 수 있으나 계획 1에 「직사각형이 어느 클립에도 걸치지 않으면 선택을 비운다 / 다른 클립이면 그 클립의 선택으로 바꾼다」를 적는다. (a)는 `picked`가 `None`일 때 지금 선택을 두는 것으로도 닫힌다 | `selection.rs:195–205`, `docs/reports/s3-t2/plan.md` 1 |
| 5 | 제안 | 선택이 비어 기준 칸만 남은 상태(빈 칸을 넓힌 뒤)에서 `Esc`는 `Selection` 컨텍스트가 없어 기준 칸을 지우지 않는다(시도 4). 다음 `Shift+이동`이 옛 기준 칸에서 넓힌다. 5.8 「닫을 것이 없으면 아무 일도 없다」의 글자는 지키지만 기준 칸을 「진행 중 조작」으로 보면 Esc가 지워야 한다. 수정 범위 밖(2회차 이전부터)이라 제안. `select_clear`를 `Normal > PianoRoll`에도 걸거나 기준 칸이 있으면 FSM에 `Select`를 넣지 않고도 Esc 처리기가 닿게 한다 | `selection.rs:81–84, 322–325`, `assets/keymaps/default.json` |

차단은 없다. 모두 선택·커서(UI 상태)와 키맵 편집기의 결함이고 문서(모델)는 바뀌지 않는다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

2회차 지적 1·3·4·5는 닫혔고 다른 경로로 깨뜨리지 못했다(시도 3·5·6·7·8). 지적 2의 수정은 클립 선택 규칙을 뒤집히게 만들었고(시도 1), 지적 6의 수정은 편집기의 다른 편집을 막는다(시도 10). 둘 다 작고 기계적인 수정이다(1: 선택의 클립을 첫 클립 규칙 앞에, 2: 새로 넣는 바인딩만 거부 또는 메시지). 수정 1–3을 고치고 `review_s3_t2_r3.rs`가 통과하면 재리뷰한다. 제안 4·5는 거절할 수 있으며 거절 이유를 보고서에 적는다.

이번이 3회 재리뷰다(`plan/01-protocol.md` [8]). 구현자가 지적 1·2의 심각도에 동의하지 않으면 다음 재리뷰 대신 결정 요청(`docs/decisions/`)으로 넘긴다. 결정이 필요한 물음은 둘이다: 「기준 칸이 틈일 때 넓히기가 고르는 클립은 무엇인가」(5.7 `Shift` 행의 REQ에 없는 세부)와 「편집기는 파일에 예약 키 항목이 있을 때 다른 편집을 쓰는가」(KM-3·KM-6의 교차).

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(12개, 4개 깨짐. 그 가운데 2개는 수정, 2개는 제안)
- [x] REQ ID를 하나씩 대조했다(5절. 수정이 닿는 행만, 나머지는 1·2회차 판정 그대로)
- [x] `git diff`에 Task 범위 밖의 변경이 없다. `8cc5d26..97eebee`의 7개 파일은 `keymap/mod.rs`·`selection.rs`(계획의 인터페이스 안), `pianoroll_select.rs`(시험 10 기대값)·`review_s3_t2_r2.rs`(리뷰 시험), 계획·보고서·리뷰 문서
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다. 수정 범위의 기존 시험 변경은 `pianoroll_select.rs` 시험 10의 꼬리 기대값(빈 직사각형 넓히기 → 선택 유지)으로, 지적 1의 규칙이 요구하는 변경이다
- [x] 새 의존성이 있으면 라이선스를 확인했다 — 새 의존성 없음(`Cargo.toml`·`Cargo.lock` 변경 없음)
- [x] 스타일 지적을 적지 않았다
