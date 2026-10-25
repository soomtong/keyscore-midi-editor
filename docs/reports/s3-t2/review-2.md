# 적대적 리뷰: S3-T2 커서·선택·수식키 체계 (리뷰 2회차)

- 리뷰어 세션: 구현자와 다른 세션(구현 맥락 없음. 1회차 리뷰·보고서 「리뷰 1 대응」·수정 커밋 diff·제품 코드만 읽음)
- 대상 커밋: `8cc5d26` (수정 범위 `git diff d2a466c..8cc5d26`, 커밋 `16f621f` `673a921` `242cbaa` `41b601a` `8cc5d26`)
- 범위: 1회차 지적 1–6의 수정, 수락한 제안 7·10·11·12, 거절한 제안 8·9의 이월. 판정 기준은 회귀·결정 위반·결함
- 리뷰 테스트: `crates/keyscore-app/tests/review_s3_t2_r2.rs`(13개 시도, 실행 결과 6 통과 / 7 실패. 실패가 「깨짐」이다)

## 1. 코드만 읽고 적은 「내가 검사할 것」

테스트 파일(`review_s3_t2.rs`, `pianoroll_select.rs`, `command_line.rs`)을 열기 전에 수정 커밋의 diff와 `selection.rs`, `pianoroll/select.rs`, `pianoroll/ops.rs`, `pianoroll/mod.rs`(`move_cursor`·`current_clip`·`retain_existing`), `keymap/mod.rs`, `settings_window.rs`(press-to-bind), `files.rs`, `preview.rs`, `window.rs`, GPUI `Keystroke::parse`(gpui-pre 0.3.8)를 읽고 적었다.

1. 지적 1(선택이 현재 클립 밖이면 거부) 우회: 선택이 클립 A, 커서가 클립 B → 거부 메시지가 나고 선택이 남는가. 커서가 클립 사이 틈(`clip_under_cursor` = `None`)일 때도 거부인가. 트랙을 바꾼 뒤 거부인가. `Shift+l`로 A의 경계를 넘어 커서만 B에 들어간 상태(기준 칸은 A, 선택은 A의 노트)에서 `Alt+k`가 거부되는가(선택은 흐린 클립의 노트라 5.3상 거부가 맞다).
2. 지적 2(기준 칸이 틈이면 커서 클립) 우회: 기준 칸 틈 + 커서 클립 안 → 커서 클립. 기준 칸 틈 + 커서 틈 → 빈 선택. 기준 칸 A + 커서 B → A의 노트만(직사각형이 B의 노트를 덮어도 B는 들지 않음). 기준 칸 A + 커서 틈 → A.
3. 지적 3(커서 노트 조작 뒤 커서 당기기) 우회: `Shorten` 연타가 되는가. 당긴 커서 틱이 `end − 1`이라 스텝 격자(960의 배수)를 벗어나고, `move_cursor`의 `StepBack/Forward`는 `tick ∓ step`이라 그 뒤 `h`/`l`도 영원히 격자 밖인가. `NudgeBarEarlier`·`OctaveUp`에서 clamp가 맞는가.
4. 지적 4(조작 뒤 기준 칸 변위) 우회: 기준 칸만 옮기고 커서는 두므로, 변위가 커서 쪽을 향하면(기준 칸 < 커서 틱에서 `NudgeLater`, 기준 칸 < 커서 음높이에서 `TransposeUp`) 직사각형이 줄어 다음 `Shift+이동`이 방금 옮긴 노트를 떨어뜨리는가. 음높이 끝(0·127)에서 기준 칸 clamp가 노트를 떨어뜨리는가.
5. 조작 뒤 선택 갱신이 `planned.moved`만 남기는데, `Shorten`은 한 스텝 이하 노트를 `targets`에서 빼므로 그 노트가 선택에서 사라지는가(선택 {960, 1920} + `<` → 선택 {960'}만).
6. 지적 5(`f20` 예약) 우회: 사용자 키맵 `"F20"`(대문자)은 `k == "f20"` 비교를 지나고 GPUI `Keystroke::parse`가 `make_ascii_lowercase`로 `f20`에 걸어, `g` 뒤 `escape`가 그 Action을 실행하는가. `fn-f20`·`shift-f20`·시퀀스 `g f20`은 어떤가. 수식키 교환이 켜지면 `swap_alt_cmd`가 parse/unparse로 소문자화해 잡히는가(비일관). 프리셋 세 파일에 f20이 없는가. 키맵 편집기 press-to-bind로 F20을 누르면 `UserKeymap::bind` → `save_user`가 파일에 쓰고 「걸었다」를 말한 뒤 `build`가 거부해 거짓 성공 메시지와 영구 KM-6 오류가 남는가.
7. 제안 7(`Shift+p` = 앞 클립): 첫 클립 중간에서는 아무 일도 없는가(수정 전에는 클립 시작으로 갔다). 틈에서는 바로 앞 클립인가. 인접 클립(`A.end == B.start`)의 B 시작에서 A로 가는가. 기존 시험 9가 그대로 통과하는가.
8. 제안 10(`:e` 틸드·기본 폴더): `~/x.ksq`, `~` 단독, `~user/x`(풀지 않음), HOME 미설정, 상대 경로가 `default_directory` 기준, 절대 경로 그대로. 실패 메시지가 풀린 경로를 보이는가.
9. 제안 11(미리듣기 실패 창마다 한 번): 엔진이 없는 시험 환경에서 커서가 처음 노트 위로 가면 상태줄에 알림이 난다. 조작 메시지 뒤 `follow_cursor_note` → `cursor_moved` → 알림이 조작 결과 메시지를 덮는가. 다른 창과 플래그를 공유하지 않는가. 기존 시험 가운데 커서 이동 뒤 상태줄을 검사하는 것이 깨지지 않는가.
10. 제안 12: `moved.first()`가 `None`인 경로(없음: `targets`가 비면 `Ok(None)`으로 먼저 돌아간다). `expect` 주석.
11. 문서: 계획 1·2·3이 수정과 글자 그대로 맞는가. 단계 파일 S3-T3·S3-T4 이월 줄. 1회차 시험 7의 기대값 변경이 「리뷰어 허용」 범위인가(diff로 확인).
12. 범위·회귀: diff에 Task 밖 파일이 없는가. 기존 시험 삭제·`#[ignore]` 없음. `pianoroll_select`·`command_line`·`review_s3_t2`·`inspector`·`keymap*` 시험 통과.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 거부 메시지·선택 유지·틈·경계 넘김 | 부분(`review_s3_t2` 시도 1·2는 「노트가 그대로」만 본다) | 메시지·선택 유지·undo 불변·돌아오면 다시 됨은 시도 1, 틈은 시도 2, 경계 넘김은 시도 3으로 확인. 모두 못 깨뜨림 |
| 2 기준 칸 틈의 네 조합 | 부분(`review_s3_t2` 시도 3: 틈 → 클립 안만) | 틈 → 클립 → 틈은 시도 4에서 깨짐 |
| 3 `Shorten` 연타와 격자 | 부분(`review_s3_t2` 시도 4는 커서 3000, 이미 격자 밖에서 시작) | 격자 위(2880)에서 시작하면 시도 5에서 깨짐 |
| 4 조작 뒤 넓히기(변위가 커서 쪽) | 부분(`review_s3_t2` 시도 6은 `alt-k` 한 번 뒤 `shift-l`, 변위 방향이 직사각형과 직각) | 같은 축으로 변위가 커서 쪽이면 시도 6·7에서 깨짐 |
| 5 `Shorten`의 선택 갱신 | 없음(`pianoroll_select` 시험 7 `alt_operations_without_selection…`과 `plan_note_op_rules`는 문서만 본다) | 시도 8에서 깨짐 |
| 6 `F20`·편집기 경로 | 부분(`review_s3_t2` 시도 7은 소문자 `f20`만) | 시도 9·10에서 깨짐 |
| 7 `Shift+p` 가장자리 | 부분(`pianoroll_select` 시험 9: B 시작과 A 시작에서만, `review_s3_t2` 시도 5: B 중간) | 첫 클립 중간·틈·대칭은 시도 11, 못 깨뜨림 |
| 8 `:e` 틸드·기본 폴더 | 없음(`command_line` 시험 18은 절대 경로만, `review_s3_t2` 시도 10은 파서만) | 시도 12, 못 깨뜨림 |
| 9 미리듣기 알림 한 번 | 없음 | 시도 13, 못 깨뜨림 |
| 10 `first()`·`expect` | 해당 없음 | 6절 |
| 11 문서 | 해당 없음 | 계획 1·2·3, 단계 파일 이월 둘 확인. 1회차 시험 파일은 커밋 `8eb0c9c` 한 번에 「수정 뒤 기대값」으로 들어와 수정 전 실패 상태가 저장소에 없다(제안 8) |
| 12 범위·회귀 | 해당 없음 | 3절 끝의 재실행 목록 |

## 1회차 지적 처리 확인

| 1회차 | 대응 커밋 | 확인 | 결과 |
| --- | --- | --- | --- |
| 1 차단: 다른 클립·트랙의 선택을 편집 | `16f621f` `apply_note_op` 거부(`selection.rs:326–335`) | `review_s3_t2` 시도 1·2 통과, 시도 1·2·3(이 문서) | 고쳐짐. 거부 메시지, 선택 유지(5.11), 문서 불변, 돌아오면 다시 적용. 커서가 틈일 때도 거부 |
| 2 수정: 기준 칸이 틈이면 빈 선택 | `16f621f` `extend`의 `or_else(clip_at(after))`(`selection.rs:169–171`) | `review_s3_t2` 시도 3 통과, 시도 4(이 문서) | 부분. 커서가 클립을 지나 다시 틈이면 직사각형이 노트를 덮어도 선택이 빈다 |
| 3 수정: `Shorten` 연타 불가 | `16f621f` `follow_cursor_note`(`selection.rs:385–414`) | `review_s3_t2` 시도 4 통과, 시도 5(이 문서) | 연타는 된다. 당긴 커서가 `end − 1`이라 스텝 격자 밖이고 그 뒤 `h`/`l`도 격자 밖 |
| 4 수정: 조작 뒤 낡은 기준 칸 | `16f621f` `shift_anchor`(`selection.rs:418–434`) | `review_s3_t2` 시도 6 통과, 시도 6·7(이 문서) | 부분. 기준 칸만 옮기고 커서는 두므로 변위가 커서 쪽이면 다음 넓히기가 노트를 떨어뜨린다 |
| 5 수정: 사용자 `f20` | `673a921` `build`의 토큰 비교(`keymap/mod.rs:229–234`) | `review_s3_t2` 시도 7 통과, 시도 9·10(이 문서) | 부분. `F20`(대문자)이 비교를 지나 그대로 실행된다. 편집기 press-to-bind 경로는 쓰고 성공을 말한다 |
| 6 누락 시험 | `8eb0c9c` `review_s3_t2.rs` 13개 | 재실행 13 통과 | 처리됨(제안 8 참고) |
| 7 제안 `Shift+p` | `242cbaa` `prev_clip_start`(`select.rs:121–127`), 계획 3 | 시도 11, `pianoroll_select` 시험 9 통과 | 수락대로. 첫 클립 중간은 그대로(REQ 글자대로) |
| 8 제안 undo 뒤 선택 | 거절, `13-S3` S3-T3 이월 줄 | 단계 파일 확인 | 이유가 선다(`core::Document`가 undo한 편집을 돌려주지 않음) |
| 9 제안 미리듣기 `prepare` | 거절, `13-S3` S3-T4 이월 줄 | 단계 파일 확인 | 이유가 선다(포트 캐시의 세대 문제, Step 미리듣기와 함께) |
| 10 제안 `:e` 틸드 | `41b601a` `open_ex_path`(`files.rs:300–320`) | 시도 12 | 수락대로. 상대 경로는 `default_directory` |
| 11 제안 알림 한 번 | `16f621f` `preview_notice_shown` | 시도 13 | 수락대로. 창마다 따로 |
| 12 제안 `expect`·`first()` | `16f621f` | 코드 | 수락대로 |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-app/tests/review_s3_t2_r2.rs`에 있고 `cargo test -p keyscore-app --test review_s3_t2_r2`로 실행했다(CoreMIDI를 쓰지 않는다). 「깨짐」은 단언이 실패한 것이다. 파일은 `cargo fmt --check`와 `cargo clippy --test review_s3_t2_r2 -- -D warnings`를 통과한다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 클립 A에서 `shift-l`로 D4 선택 → `shift-n`으로 B → `alt-k`: A 불변, 선택 유지, FSM `Selection`, 메시지 「선택이 현재 클립에 없다」, `undo_len` 불변. `shift-p`로 A(0)에 돌아와 `alt-k` → D4가 63 | 못 깨뜨림 | `brk2_alt_op_with_selection_in_other_clip_rejects_and_keeps_selection` |
| 2 | 선택 D4, 커서를 B 뒤의 틈(6마디)에 두고 `alt-k` | 못 깨뜨림: 거부·메시지·선택 유지 | `brk2_alt_op_with_selection_while_cursor_in_gap_rejects` |
| 3 | 커서 (14400, F4)에서 `shift-l` → 커서는 B의 시작(15360), 선택은 기준 칸의 클립 A의 F4(B의 G4는 음높이 밖). 이어서 `alt-k` | 못 깨뜨림: 선택은 A, 조작은 거부, 문서 불변. 설계상 일관되나 「`Shift+l` 직후 `Alt+k`가 거부되는」 흐름이다(제안 9) | `brk2_shift_extend_across_boundary_selects_anchor_clip_and_alt_op_is_rejected` |
| 4 | 1회차 시도 3의 픽스처(A 0–1마디, B 2–3마디 시작에 C4). 틈(1마디+2박)에서 `shift-l` ×3 → B 안, C4 선택(지적 2 수정 확인). 다시 `shift-l` ×4로 B의 끝을 지나 틈(3마디+1박). 직사각형 `[5760, 13440) × 60`은 여전히 C4를 덮는다 | 깨짐: 선택이 빈다. `clip_at(anchor)`도 `clip_at(after)`도 `None` | `brk2_shift_extend_from_gap_past_clip_end_drops_selection` |
| 5 | 노트 `[0, 3840)`, 커서 격자 위(2880). `alt-shift-h` → 노트 `[0, 2880)`, 커서는 `end − 1` = 2879. 이어서 `l` | 깨짐: 커서 2879(격자 밖), `l` 뒤 3839. `move_cursor`는 `tick ± step`이라 격자로 돌아오지 않는다 | `brk2_shorten_pulls_cursor_off_the_step_grid` |
| 6 | 노트 4개(0·960·1920·2880, C4). 커서 0에서 `shift-l` ×3 → 4개 선택(기준 칸 0, 커서 2880). `alt-l` ×3 → 노트 2880·3840·4800·5760, 기준 칸 2880, 커서 2880 그대로. `shift-l` | 깨짐: 선택 4 → 2(`[2880, 4800)`이 4800·5760을 덮지 않음). 1회차 지적 4의 다른 경로 | `brk2_shift_extend_after_nudge_later_drops_notes_from_selection` |
| 7 | 화음 60·61·62. `shift-k` ×2 → 3개 선택(기준 칸 60, 커서 62). `alt-k` ×2 → 62·63·64, 기준 칸 62, 커서 62. `shift-k` | 깨짐: 선택 3 → 2(64가 `62..=63` 밖) | `brk2_shift_extend_after_transpose_up_drops_notes_from_selection` |
| 8 | 선택 {C4 960, D4 1920}에 `alt-shift-h`. D4만 줄고 C4는 그대로(계획 2) | 깨짐: 선택이 {D4'}만 남고 C4가 빠진다(`replace_selection(planned.moved)`, `moved`는 `targets`만). 다음 `>`는 D4에만 간다 | `brk2_shorten_drops_unchanged_short_note_from_selection` |
| 9 | 사용자 `keymap.json` `Normal` `"F20"`=`view::Toggle`(대문자), vim 프리셋, `g` 뒤 `escape` | 깨짐: 키맵 오류 0개, 뷰가 배치 뷰로 바뀐다. `build`는 토큰 `"F20" == "f20"`을 비교하고 GPUI `Keystroke::parse`는 이름 키를 소문자로 바꾼다 | `brk2_uppercase_f20_user_binding_bypasses_reservation` |
| 10 | 편집기 경로 재현: `current_layers` → `UserKeymap::bind("view::Toggle", Normal, "f20")` → `save_user` | 깨짐: `save_user`가 `Ok`(편집기는 「걸었다」를 말한다), 파일에 `"f20": "view::Toggle"`이 남고 `errors`에 KM-6 오류가 매번 뜬다. 거짓 성공 메시지 | `brk2_editor_bind_path_writes_f20_and_reports_success` |
| 11 | `Shift+p`: 첫 클립 중간(960)에서 그대로, B 뒤 틈(6마디)에서 B 시작, 인접 B 시작에서 A, `Shift+n`은 A 중간에서 B | 못 깨뜨림 | `brk2_shift_p_first_clip_middle_stays_and_gap_goes_to_previous_clip` |
| 12 | 없는 파일로 `:e ~/x`, `:e x`(문서 경로 없음), `:e ~`. 실패 메시지의 풀린 경로 | 못 깨뜨림: `$HOME/x`, `default_directory()/x`(`~/Documents`), `$HOME`. 홈은 건드리지 않았다 | `brk2_ex_e_expands_tilde_and_resolves_relative_to_default_dir` |
| 13 | 엔진 없는 환경에서 `n`으로 노트 위 → 알림 한 번(「창마다 한 번」), `n n n` 뒤 메시지 수 불변, 두 번째 창은 따로 알림, 커서 노트 `alt-k`의 커서 따라가기는 더 알리지 않음 | 못 깨뜨림 | `brk2_preview_failure_notice_once_per_window` |

실행하지 않은 것(적지 않음): `fn-f20`·`shift-f20`·`g f20` 사용자 바인딩(코드로만 봄: 수식키가 붙은 것은 수식키 없는 flush 키와 맞지 않고, 시퀀스 토큰은 `split_whitespace`가 잡는다), 수정 전 커밋 `d2a466c`에서의 같은 시험(worktree를 만들지 않았다. 시도 4–10은 모두 수정이 만든 코드 또는 수정이 건드린 갱신 경로의 결함이라 수정 전·후 비교로 얻을 것이 없다).

구현자·기존 시험 재실행(모두 통과): `review_s3_t2` 13, `command_line` 7, `pianoroll_select` 12(CoreMIDI 둘 제외) + `cursor_move_onto_note_sends_preview_on_track_port_channel`·`play_from_cursor_starts_at_piano_roll_cursor` 각각 단독, `inspector` 11(CoreMIDI, 단독), `commands` 12, `files` 30, `inspector_logic` 5, `keymap_engine` 11, `keymap_source` 4, `keymap_user` 4, `pianoroll` 13, `review_s2_t3` 13 / `_r2` 12 / `_r3` 9, `review_s3_t1` 19 / `_r2` 10, `settings_keymap` 9, `status_bar` 2, `window` 12.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 아니오 | 수정은 `plan_note_op`·`map_notes` 경로를 바꾸지 않았다. 1회차와 같음 |
| 노트는 클립 안 | 아니오 | 같음 |
| 틱 ≥ 0 | 아니오 | `follow_cursor_note`는 `clamp(abs_start, …)`와 `max(0)`, `shift_anchor`는 `max(0)`. 커서가 격자 밖으로 가는 것(시도 5)은 틱 ≥ 0을 깨지 않는다 |
| 채널 1–16 | 해당 없음 | 채널을 만지지 않는다 |
| 음높이 0–127 | 아니오 | `shift_anchor`의 `clamp(0, 127)` 뒤 `Pitch::new`, 실패 시 원래 음높이. `plan_note_op`의 전무 규칙 그대로 |
| 선택의 노트는 실제 노트 | 부분적으로 예(해롭지 않음, 1회차와 같음) | `Selection::set`·`set_selection`은 `pub`. 조작은 `map_notes`가 `NoteNotFound`로 거부. 이번 수정으로 「선택의 (트랙, 클립) = 현재 클립」을 조작 시점에 검사하므로 1회차의 「현재 클립의 노트라는 보장 없음」은 편집 경로에서는 닫혔다 |
| 선택 유무 = FSM `Selection` 컨텍스트 | 아니오 | `replace_selection`·`set_selection` 한곳. 시도 1에서 거부 뒤에도 일치 |
| 커서는 스텝 격자의 한 칸(5.3) | 예 | `follow_cursor_note`가 `end − 1`로 당긴다(시도 5). 모델 불변식이 아니라 UI 상태라 「차단」으로 세지 않는다 |

## 5. REQ 대조

1회차 5절에서 이번 수정이 닿는 행만 다시 본다. 나머지는 1회차 판정 그대로다.

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.3 「앞뒤 클립의 노트는 … 편집은 되지 않는다」 | 충족 | `selection.rs:326–335` 거부. 시도 1·2·3. 1회차 미충족이 닫혔다 |
| 5.3 「커서는 (틱, 음높이) 한 칸」 | 부분 충족 | `selection.rs:403–405` `end − 1`로 당김. 시도 5: 커서 2879, 이후 `l`로 3839 |
| 5.7 표 `Shift` 행 「선택 넓히기」 | 부분 충족 | `selection.rs:169–171`(시도 4: 틈 → 클립 → 틈에서 빈 선택), `selection.rs:373–379, 418–434`(시도 6·7: 조작 뒤 넓히기가 선택을 줄임). 1회차 지적 2·4가 다른 경로로 남아 있다 |
| 5.7 표 `Alt`·`Alt+Shift` 행(대상은 선택) | 부분 충족 | `selection.rs:377–378` 선택을 `planned.moved`로 바꿔 `Shorten`에서 「그대로 둔」 노트가 선택에서 빠진다(시도 8). 계획 2 「줄이기는 길이가 한 스텝 이하인 노트는 그대로 둔다」와 어긋남 |
| 5.7 「같은 조작을 연속으로」(`<` 연타) | 충족 | 시도 5의 첫 단언 전까지 두 번째 `<`가 적용된다(`review_s3_t2` 시도 4 통과) |
| 5.7·5.8 `Shift+p` 「이전 클립으로」 | 충족 | `select.rs:121–127`, 시도 11, `pianoroll_select` 시험 9. 계획 3과 글자 그대로 |
| 5.8 `Esc` 「닫을 것이 없으면 아무 일도 없다」 + A13 시퀀스 대기 중 Esc | 부분 충족 | `keymap/mod.rs:229–234`. 소문자 `f20`은 거부(1회차 시도 7 통과), 대문자 `F20`은 통과해 Action 실행(시도 9). 결정 A13 위반 경로가 남아 있다 |
| KM-6 키맵 오류 보고 | 부분 충족 | `settings_window.rs` press-to-bind → `keymap/mod.rs:555 save_user`가 f20을 쓰고 성공을 돌려줌(시도 10). 상태줄에는 KM-6 오류가 뜨지만 편집기 메시지는 「걸었다」 |
| 5.11 「선택을 유지한 채 다음 트랙」 | 충족 | 거부 뒤 선택 유지(시도 1·2). 계획 2에 적음 |
| 7.5 `:e` | 충족 | `files.rs:300–320`, 시도 12. 틸드·기본 폴더 모두 설계대로 |
| PRE-1 미리듣기(실패 알림) | 충족 | `selection.rs:744–750`, 시도 13. 창마다 한 번, 두 번째 창 따로 |
| 00-conventions `expect` 이유 주석 | 충족 | `inspector.rs:311–312` |

「부분 충족」은 미충족으로 센다: 5.3 커서 한 칸, 5.7 `Shift` 행, 5.7 `Alt+Shift` 행(선택 갱신), A13(`F20`), KM-6(편집기 경로).

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 수정은 `keyscore-midi`를 건드리지 않았고 미리듣기 경로(`preview.rs`)도 바뀌지 않았다. 알림 추가는 UI 스레드의 `say` 한 번이다.
- 제품 코드의 `unwrap`/`expect`(수정 범위):
  - `inspector.rs:312` `expect("TEST_PITCH는 0–127 안의 상수")`: 이유 주석이 붙었다. 정당.
  - `selection.rs:370, 374` `planned.moved.first()`: `None`이면 아무 일도 하지 않는다. 패닉 경로 제거됨. 정당.
  - `selection.rs:407` `u64::try_from(tick.max(0)).unwrap_or(u64::MAX)`, `selection.rs:427–431` `unwrap_or(anchor.pitch)`: clamp 뒤라 도달하지 않는다. 정당.
  - `selection.rs:398` `map_or(0, …)`: 클립이 없으면 클립 시작을 0으로 본다. `apply_coalesced` 성공 직후라 클립은 있다. 정당하나 `clip_start`가 0으로 떨어지면 `abs_start`가 틀려 커서가 엉뚱한 곳으로 당겨질 수 있다(도달 불가라 지적하지 않음).
- 에러가 삼켜지는 곳:
  - `preview_cursor_note`: 이제 첫 실패를 알린다(제안 11 수락). 두 번째부터는 조용히 넘긴다(의도).
  - `open_ex_path`: HOME 미설정이면 `~/x`를 상대 경로 그대로 둔다(기본 폴더 기준 `…/~/x`). 메시지에 풀린 경로가 보이므로 사용자가 알 수 있다. 지적하지 않음.
  - `keymap::build`: `F20`은 오류 없이 걸린다(시도 9). 이것은 삼켜진 에러가 아니라 검사 누락이다(7절 5).
  - `settings_window::bind`: `save_user`의 `Ok`를 성공으로 보고하지만 그 뒤 `apply`가 낸 `build` 오류는 편집기 메시지에 반영되지 않는다(7절 6).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 1회차 지적 4가 다른 경로로 남아 있다. 선택 조작 뒤 `shift_anchor`는 기준 칸만 옮기고 커서는 두므로, 변위가 커서 쪽을 향하면(기준 칸 < 커서 틱에서 `Alt+l`, 기준 칸 < 커서 음높이에서 `Alt+k`) 기준 칸 ↔ 커서 직사각형이 줄어 다음 `Shift+이동`이 방금 옮긴 노트를 떨어뜨린다(시도 6: 4 → 2, 시도 7: 3 → 2). 5.7 「선택 넓히기」가 선택을 줄인다. 길은 둘 중 하나로 보인다: (a) 선택 조작 뒤 커서도 같은 변위만큼 옮긴다(커서가 선택을 따라감, 기준 칸과 함께 직사각형이 보존된다), (b) 조작 뒤의 `Shift+이동`은 직사각형을 다시 계산하지 않고 현재 선택에 새 직사각형의 노트를 합친다. 어느 쪽이든 계획 1·2에 한 줄로 적는다 | `crates/keyscore-app/src/selection.rs:373–379, 418–434` |
| 2 | 수정 | 1회차 지적 2가 다른 경로로 남아 있다. 기준 칸이 틈이고 커서가 클립을 지나 다시 틈으로 가면 `clip_at(anchor)`·`clip_at(after)`가 모두 `None`이라 직사각형이 노트를 덮는데도 선택이 빈다(시도 4). 1회차가 적은 두 번째 선택지(직사각형 `[lo, hi)`가 걸치는 클립을 고른다)나 「선택이 비어 있지 않으면 선택의 클립을 쓴다」로 닫힌다 | `selection.rs:169–171` |
| 3 | 수정 | 지적 3의 수정이 새 결함을 만들었다. `follow_cursor_note`가 커서를 `end − 1`로 당겨 스텝 격자(960의 배수)를 벗어나고, `move_cursor`의 `StepBack/Forward`는 `tick ∓ step`이라 그 뒤의 `h`/`l`도 영원히 격자 밖이다(시도 5: 2880 → `<` → 2879 → `l` → 3839). 5.3 「커서는 (틱, 음높이) 한 칸」. 노트 안의 마지막 스텝 칸(`start + floor((len − 1) / step) × step`, `clip_last_step`과 같은 식)으로 당기면 격자가 유지되고 연타도 된다 | `selection.rs:403–405`, `pianoroll/mod.rs:530–537` |
| 4 | 수정 | 조작 뒤 선택을 `planned.moved`로 바꾸는데 `moved`는 `targets`(바뀌는 노트)만 담는다. `Shorten`은 한 스텝 이하 노트를 `targets`에서 빼므로 그 노트가 선택에서 사라진다(시도 8: 선택 {C4 960, D4 1920} + `<` → 선택 {D4'}). 계획 2 「줄이기는 길이가 한 스텝 이하인 노트는 그대로 둔다」와 어긋나고 다음 `>`는 D4에만 간다. 바뀌지 않은 대상도 선택에 남긴다(`notes`에서 `moved`의 `from`이 아닌 것을 그대로 합친다) | `selection.rs:377–378`, `pianoroll/ops.rs:148–150` |
| 5 | 수정 | 1회차 지적 5가 다른 경로로 남아 있다. `build`는 키 토큰을 `k == "f20"`으로 비교하지만 GPUI `Keystroke::parse`는 이름 키를 `make_ascii_lowercase`로 바꾸므로 사용자 키맵의 `"F20"`은 검사를 지나 `f20`에 걸리고, `g` 뒤 `escape`가 그 Action을 실행한다(시도 9: 키맵 오류 0개, 뷰가 배치 뷰로). 결정 A13 위반 경로다. 수식키 교환이 켜져 있으면 `swap_alt_cmd`의 parse/unparse가 소문자로 바꿔 잡히므로 옵션에 따라 결과가 다르다. 토큰을 `Keystroke::parse`로 정규화해 `key == FLUSH_KEY`로(또는 `canonical_keys` 뒤에) 비교한다 | `crates/keyscore-app/src/keymap/mod.rs:229–234`, gpui-pre 0.3.8 `platform/keystroke.rs` `parse` |
| 6 | 수정 | 키맵 편집기 press-to-bind 경로는 예약 키를 거부하지 않는다. `finish_capture` → `bind` → `UserKeymap::bind` → `save_user`가 `f20`을 `keymap.json`에 쓰고 `Ok`를 돌려 편집기는 「`view::Toggle`에 f20을 걸었다」를 말한다. 그 뒤 `apply`의 `build`가 거부해 상태줄에는 KM-6 오류가 매번 뜨고 파일에는 쓸 수 없는 항목이 남는다(시도 10). `finish_capture`(또는 `UserKeymap::bind`)에서 예약 키를 거부하고 메시지를 내거나, `save_user`가 `apply` 뒤의 새 `build` 오류를 `Err`로 돌린다 | `crates/keyscore-app/src/settings_window.rs:500–580`(`finish_capture`·`bind`), `keymap/mod.rs:555–578`(`save_user`) |
| 7 | 수정 | 누락 시험: 위 1–6. `review_s3_t2_r2.rs`의 실패 7개가 그대로 쓸 수 있는 시험이다. 고친 뒤 이 파일을 PR에 두고 `brk2_` 접두 그대로 통과시킨다 | `crates/keyscore-app/tests/review_s3_t2_r2.rs` |
| 8 | 제안 | 1회차 시험 파일 `review_s3_t2.rs`가 커밋 `8eb0c9c` 한 번에 「수정 뒤 기대값」(시도 7 주석 「수정(리뷰 1 지적 5)」, 시도 2·4의 기대값)으로 들어와 「수정 전 실패 → 수정 후 통과」 이력이 저장소에 없다. 보고서의 「리뷰어 허용」을 확인할 길이 diff에 없다. 다음부터는 리뷰 시험을 실패 상태로 먼저 커밋하고 기대값 변경은 수정 커밋에 따로 둔다(01-protocol [5] 2의 「실패하면 그 테스트를 PR에 추가한다」의 취지) | `git log -- crates/keyscore-app/tests/review_s3_t2.rs` |
| 9 | 제안 | `Shift+l`로 클립 경계를 넘으면 선택은 기준 칸의 클립(A), 현재 클립은 B가 되어 바로 이은 `Alt` 조작이 「선택이 현재 클립에 없다」로 거부된다(시도 3). 지적 1의 결정과 일관되지만 사용자는 `Shift+l` 직후 `Alt+k`가 왜 안 되는지 메시지로만 안다. 계획 1에 「직사각형이 다른 클립에 걸치면 선택은 기준 칸의 클립이고 커서가 그 클립 밖이면 조작은 거부된다」를 한 줄 적고, 거부 메시지에 되돌아갈 키(`Shift+p`/`Cmd+←`)를 덧붙이면 충분하다 | `selection.rs:168–171, 326–335`, `docs/reports/s3-t2/plan.md` 1 |

차단은 없다. 모두 선택·커서(UI 상태)의 결함이고 문서(모델)는 바뀌지 않는다.

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

1회차 차단 1은 닫혔다(5.3 편집 불가, 시도 1·2·3). 그러나 1회차 지적 2·4·5가 각각 다른 경로(시도 4 / 6·7 / 9)로 남아 있고, 지적 3의 수정이 커서를 격자 밖으로 보내며(시도 5), 조작 뒤 선택 갱신이 `Shorten`에서 노트를 떨어뜨린다(시도 8). 수정 1–6을 고치고 `review_s3_t2_r2.rs`의 실패 시험이 통과하도록 한 뒤 재리뷰한다. 제안 8·9는 거절할 수 있으며 거절 이유를 보고서에 적는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(13개, 7개 깨짐)
- [x] REQ ID를 하나씩 대조했다(5절. 수정이 닿는 행만, 나머지는 1회차 판정 그대로)
- [x] `git diff`에 Task 범위 밖의 변경이 없다. `d2a466c..8cc5d26`의 11개 파일은 `files.rs`·`inspector.rs`·`keymap/mod.rs`·`pianoroll/select.rs`·`selection.rs`·`window.rs`(계획의 인터페이스 안), `review_s3_t2.rs`(리뷰 시험), 계획·보고서·리뷰 문서, `13-S3` 단계 파일의 이월 두 줄
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다. 수정 범위의 테스트 변경은 `review_s3_t2.rs` 추가(629줄)만이다
- [x] 새 의존성이 있으면 라이선스를 확인했다 — 새 의존성 없음(`Cargo.toml`·`Cargo.lock` 변경 없음)
- [x] 스타일 지적을 적지 않았다
