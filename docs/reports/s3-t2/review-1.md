# 적대적 리뷰: S3-T2 커서·선택·수식키 체계 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 세션(구현 맥락 없음, 코드·계획·REQ·결정 문서만 읽음)
- 대상 커밋: `d2a466c` (`git diff main...HEAD`)
- 리뷰 테스트: `crates/keyscore-app/tests/review_s3_t2.rs`(13개 시도, 실행 결과 6 통과 / 7 실패. 실패가 「깨짐」이다)

## 1. 코드만 읽고 적은 「내가 검사할 것」

테스트 파일(`pianoroll_select.rs`, `command_line.rs`, `transform_map.rs`)을 열기 전에 `selection.rs`, `select.rs`, `ops.rs`, `preview.rs`, `window.rs`, `keymap/mod.rs`, `command_palette.rs`, `ex.rs`, `files.rs`, `settings*.rs`, 키맵 JSON, core 변경을 읽고 적었다.

1. 선택이 가리키는 클립과 커서가 든 클립이 다를 때 `Alt` 조작: 클립 A에서 `shift-l`로 고른 뒤 `l`로 클립 B(또는 빈 틈)에 들어가 `alt-k`를 치면 흐리게 보이는 A의 노트가 바뀌는가(REQ 5.3 「앞뒤 클립의 노트는 … 편집은 되지 않는다」). 같은 구조로 트랙을 바꾼 뒤(`Tab`) 보이지 않는 선택이 바뀌는가.
2. 기준 칸(anchor)이 어느 클립에도 들지 않을 때(클립 사이 빈 틈에서 `shift-l`을 시작): `clip_at(anchor.tick)`이 `None`이라 커서가 클립 안으로 들어가도 영원히 아무것도 선택되지 않는가.
3. 선택 없이 커서 노트에 `Shorten`(`<`)을 연타: 커서 틱이 노트 끝 뒤로 밀려 두 번째 `<`가 「커서 위에 노트가 없다」로 실패하는가(5.7 「같은 조작을 연속으로」 + 「선택이 없으면 커서 위의 노트 하나」).
4. `Shift+p`가 클립 중간에서 「현재 클립의 시작」으로 가는가(`prev_clip_start`는 `start < tick`). REQ 5.8은 「이전 클립으로」다.
5. 되돌리기 묶음: 메뉴·팔레트로 실행한 `cmd-z`(키 입력 아님)는 `seal`을 거치지 않는데 core `History::undo`가 스스로 `sealed`를 켜는가. 실패한 조작(겹침 거부)이 끼어도 묶음이 유지되는가. 입력란 글자가 묶음을 끊는가. 다른 창의 키가 이 창의 묶음을 끊지 않는가.
6. 선택 가지치기와 FSM: 선택 노트를 모두 지우는 문서 변경(undo) 뒤 `Normal > PianoRoll > Selection` 컨텍스트가 남아 `escape`가 `select::Clear`로 가고 아무 일도 안 하는 상태가 있는가. 선택의 트랙이 지워졌을 때(`track::Delete`) 선택이 비는가.
7. `n`/`p`: 화음(같은 틱 세 음)을 음높이 순으로 한 음씩 지나는가. 꼬리 노트(시작은 클립 안, 끝은 클립 밖)를 지나는가. 클립 사이 틈에서 `n`이 다음 클립 첫 노트로 가는가. 다른 트랙 노트는 보지 않는가.
8. `cmd-a`: 입력란(템포·트랙 이름) 포커스에서 글자 전체 선택으로 가고 피아노롤 선택이 생기지 않는가. 배치 뷰에서 아무 일도 없는가.
9. 시퀀스 대기 중 `escape`(vim `g` 뒤): 선택이 유지되고 카운트가 비는가. 설정 창(프로젝트 창 아님)에서는 가로채지 않는가. `f20`을 사용자 키맵이 걸면 그 Action이 실행되는가(보고서가 인정한 한계인지 확인). 대기가 없을 때의 `escape`는 그대로 `select::Clear`인가.
10. `:` 명령줄 파서: `:w a b.ksq`(공백 포함 이름), `:e ~/x.ksq`(틸드), `:set Input`(대소문자), `:q!`, `:wq`, `: w`(접두 뒤 공백), `::`. `:e 상대경로`가 문서 폴더 기준인가. `:w 이름`에 확장자가 없으면 `.ksq`.
11. 미리듣기(`preview_cursor_note`): 엔진 없음·다른 창 재생 중·이 창 재생 중·정지 중의 분기. 정지 중 이름 포트 트랙에서 이동마다 `prepare`+`swap`하는 비용. 일시정지(`Paused`)에서 주인을 놓는지(놓으면 「정지 중」 분기로 스냅샷을 바꾼다).
12. 마크: 트랙이 지워진 뒤 `JumpMark` 알림, 마크 이름 검증(`A` → `a`, 두 글자 거부), 우클릭 메뉴가 커서를 옮기고 묶음을 끊는가, 메뉴가 뜬 뒤 포커스.
13. 설정 `preview` 읽기: `length_ms` 0·10001·실수·문자열, `on_move` 문자열, 모르는 키 보존, `SetPreviewLength(-1)`.
14. 새 Action 4개의 명령 표·팔레트·`ALL_ACTION_NAMES`·메뉴 자리(UI-11). `palette::OpenCommandLine`이 보기 메뉴에 있는가.
15. `Alt+Shift+l` 클립 연장이 다음 클립과 겹치면 전부 거부(전부 아니면 전무)인가. `Alt+Ctrl+j/k`가 (예약)으로 바인딩 없음인가. 옥타브 올림에서 한 노트만 127을 넘어도 전무인가.
16. REQ 문구 수정(5.8 `:` 행, 5.8.1 `g` 행·`;` 행·`.` `,` 행·충돌 메모)이 결정 A2·A10·A11·A12와 글자 그대로 맞는가.
17. 제품 코드의 `unwrap`/`expect`: `inspector.rs`의 `Pitch::new(TEST_PITCH).expect("가운데 C")`가 새로 들어왔다.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 선택의 클립·트랙 ≠ 커서의 클립·트랙에서 `Alt` 조작 | 없음 | 보고서 「리뷰어에게」가 「의도」라고 적고 시험은 없다. 3절 시도 1·2에서 깨짐 |
| 2 기준 칸이 빈 틈 | 없음 | 시험 10이 「클립 밖에서 `b`」만 본다. 시도 3에서 깨짐 |
| 3 커서 노트 `Shorten` 연타 | 없음 | 시험 7은 `alt-k`·`alt-shift-l`·`alt-l`만 연타한다(길이를 늘리는 쪽은 커서가 노트 안에 남는다). 시도 4에서 깨짐 |
| 4 `Shift+p` 클립 중간 | 없음 | 시험 9는 클립 B의 시작(15360)에서만 `shift-p`를 친다. 시도 5에서 확인 |
| 5 메뉴 경로 undo 뒤 묶음 | 부분(core `history_coalesce_breaks_on_other_key_seal_or_undo`) | 앱 경로는 시도 9로 확인, 못 깨뜨림 |
| 6 undo로 선택이 모두 사라질 때 FSM, 트랙 삭제 | 부분(시험 17: undo) | 트랙 삭제는 시도 8로 확인, 못 깨뜨림 |
| 7 `n`/`p` 화음·꼬리·다른 트랙 | 부분(시험 2: 화음·클립 경계) | 꼬리 노트는 시도 11로 확인, 못 깨뜨림 |
| 8 입력란 `cmd-a` | 간접(`review_s2_t5b::vim_digits_in_tempo_field`가 빈 프로젝트에서 `cmd-a`) | 노트가 있는 프로젝트에서 시도 6으로 확인, 못 깨뜨림 |
| 9 시퀀스 대기 중 Esc, `f20` | 부분(시험 13) | 사용자 키맵 `f20`은 시도 7에서 깨짐(보고서가 한계로 인정) |
| 10 ex 파서 가장자리 | 부분(시험 5) | 시도 10으로 확인. 틸드는 풀지 않는다(제안) |
| 11 미리듣기 분기 | 부분(시험 11: 정지 중·가상 포트) | 이름 포트·다른 창 재생 중 분기는 시험 없음(`inspector.rs` 시험이 같은 함수를 PRE-3으로 덮는다). 비용은 코드로만 봄 |
| 12 마크 | 있음(시험 16) | 지워진 트랙은 코드로 확인(`cursor_jump_mark`가 `track(mark.track)` 검사) |
| 13 설정 `preview` | 있음(시험 19) | 범위·형식·모르는 키 모두 본다 |
| 14 Action 4개 표·팔레트·메뉴 | 있음(시험 14, 기존 `commands` 12개 통과 확인) | `palette::OpenCommandLine`은 보기 메뉴 54번 |
| 15 전무 규칙·예약 칸 | 있음(시험 4, 6) | |
| 16 REQ 문구 | 부분(시험 12가 `vim.json`만) | REQ diff를 결정 문서와 손으로 대조, 5절 |
| 17 `expect` | 해당 없음 | 6절 |

## 3. 깨뜨리기 시도

모두 `crates/keyscore-app/tests/review_s3_t2.rs`에 있고 `cargo test -p keyscore-app --test review_s3_t2`로 실행했다(CoreMIDI를 쓰지 않는다). 「깨짐」은 단언이 실패한 것이다.

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 클립 A에서 `shift-l`로 D4 선택 → `shift-n`으로 클립 B → `alt-k`. A의 D4가 그대로인가(5.3) | 깨짐: A의 D4가 63으로 바뀌었다. 현재 클립은 B이고 장면은 선택을 그리지 않는데(흐린 노트) 편집이 됐다 | `brk_alt_op_with_selection_in_other_clip_edits_dimmed_notes` |
| 2 | 같은 선택 뒤 `cmd-down`으로 트랙 U → `alt-k`. 트랙 T의 D4가 그대로인가 | 깨짐: 보이지 않는 트랙 T의 D4가 63으로 바뀌었다. 현재 트랙 U는 그대로 | `brk_alt_op_with_selection_on_other_track_edits_invisible_notes` |
| 3 | 클립 A(0–1마디)·B(2–3마디, 시작에 C4). 틈(1마디+2박)에서 `shift-l` ×3으로 B 안(2마디+1박)까지. 직사각형 `[5760, 9600) × 60`에 B의 C4가 든다 | 깨짐: 선택이 빈다. `extend`가 `clip_at(anchor.tick)`으로 클립을 고르는데 기준 칸이 틈이라 `None` → 영원히 선택 없음 | `brk_shift_extend_started_in_gap_never_selects` |
| 4 | 노트 `[0, 3840)`, 커서 3000, 선택 없이 `alt-shift-h` 두 번 | 깨짐: 첫 번째로 `[0, 2880)`이 되고 커서(3000)가 노트 밖이라 두 번째는 「커서 위에 노트가 없다」. 5.7 연타가 성립하지 않는다 | `brk_shorten_repeat_on_cursor_note_fails_when_cursor_falls_off` |
| 5 | 클립 B 중간(4마디+480)에서 `shift-p` | 깨짐(REQ 글자 기준): B의 시작(15360)으로 간다. 「이전 클립」 A(0)가 아니다. S3-T1 `Ctrl+h` 읽기(마디 중간이면 그 마디 시작)와 같은 읽기이므로 7절에서는 제안으로 둔다 | `brk_shift_p_from_clip_middle_goes_to_current_clip_start` |
| 6 | `shift-l`로 D4 선택 → `alt-k`(선택은 63 노트) → `shift-l`로 더 넓히기 | 깨짐: 선택이 빈다. 기준 칸(음높이 62)↔커서 직사각형에 방금 옮긴 63 노트가 들지 않아 「넓히기」가 선택을 지웠다 | `brk_extend_after_alt_op_drops_moved_note_from_selection` |
| 7 | 사용자 `keymap.json`에 `Normal` `f20`=`view::Toggle`, vim 프리셋, `g` 뒤 `escape` | 깨짐: 뷰가 배치 뷰로 바뀐다. Esc가 시퀀스만 취소해야 하는데(A13) 사용자 Action이 실행됐다. 보고서 「알려진 한계」와 일치 | `brk_user_keymap_f20_runs_on_escape_during_pending_sequence` |
| 8 | 트랙 T의 D4 선택 → `track::Delete`로 T 삭제 → 선택·FSM·`escape` | 못 깨뜨림: 선택이 비고 FSM도 선택 없음, Esc는 no-op | `brk_track_delete_clears_selection_pointing_to_it` |
| 9 | `alt-shift-l` ×2, `l`, `alt-shift-l` ×2 → 메뉴 경로 `edit::Undo`(키 아님) → 다시 고르고 `alt-shift-l` | 못 깨뜨림: core `History::undo`가 봉인해 새 단위(undo 2단위). 부수 관찰: undo 뒤 선택이 비어 다시 골라야 했다(7절 제안 9) | `brk_undo_via_menu_then_same_op_starts_new_unit` |
| 10 | ex 파서: `: w`, `:w a b.ksq`, `:e ~/x.ksq`, `:set Input`, `:W`, `:q!`, `:q x`, `::`, `:`, `w`, `is_command_line` | 못 깨뜨림(모두 설계대로). 틸드는 파서가 풀지 않아 `:e ~/x.ksq`는 열기에서 `<문서 폴더>/~/x.ksq`가 된다(제안) | `brk_ex_parse_edge_cases` |
| 11 | 분할로 만든 꼬리 노트(시작은 클립 안, 끝은 밖)와 다음 클립을 `n`이 지나는가 | 못 깨뜨림 | `brk_next_note_walks_tail_note_and_only_this_track` |
| 12 | 템포 입력란에 포커스 두고 `cmd-a` → 노트 선택이 생기지 않고 글자 전체가 골라지는가(`backspace` 뒤 빈 값) | 못 깨뜨림 | `brk_cmd_a_in_tempo_field_selects_text_not_notes` |
| 13 | 배치 뷰에서 `cmd-a` | 못 깨뜨림: 선택 없음, 컨텍스트 `Normal > Arrangement` | `brk_cmd_a_in_arrangement_view_does_nothing` |

실행하지 않은 것(적지 않음): 설정 창에서의 시퀀스 대기 중 Esc(설정 창에는 시퀀스가 없어 만들 수 없었다), 다른 창 재생 중의 미리듣기 분기(CoreMIDI 두 창 시험을 새로 만들지 않았다).

구현자 시험 재실행: `command_line` 7 통과, `transform_map` 4 통과, `pianoroll_select` 12 통과(CoreMIDI 둘 제외) + `cursor_move_onto_note_sends_preview_on_track_port_channel` 단독 통과, `commands` 12 통과. 리뷰 테스트 파일은 `cargo fmt --check`와 `cargo clippy --test review_s3_t2 -D warnings`를 통과한다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 아니오 | 앱은 `Edit`만 만들고 `transform::map_notes` → `plan`이 `bar_aligned_length`로 연장하며 겹치면 `ClipOverlap`(시험 4 확인). 클립 길이를 앱이 직접 쓰는 곳 없음 |
| 노트는 클립 안(꼬리는 시작만) | 아니오 | `map_notes`가 `needed_end`를 편집 직전 길이로 판정(S0-T7 규칙 그대로). `Note::with_start/with_pitch/with_length`를 공개했지만 자기 검사(길이 0, 오버플로, 절대 피치)는 그대로고 클립 소속은 `ReplaceNotes` 적용 때 core가 검사 |
| 틱 ≥ 0 | 아니오 | `shift_start`가 `rel < 0`을 `NoteBeforeClipStart`로 거부. 커서는 `move_cursor`가 `saturating_sub`. `apply_note_op`의 커서 따라가기도 `.max(0)` |
| 채널 1–16 | 해당 없음 | 이 Task는 채널을 만지지 않는다. 미리듣기는 `track.channel()`을 그대로 넘긴다 |
| 음높이 0–127 | 아니오 | `plan_note_op`가 조옮김 전 전체를 검사해 하나라도 밖이면 `PitchOutOfRange`(접기 없음) |
| 선택의 노트는 실제 노트 | 부분적으로 예(해롭지 않음) | `Selection::set`·`ProjectWindow::set_selection`이 `pub`이라 시험·다른 코드가 클립에 없는 노트를 넣을 수 있다. 그 선택으로 `Alt` 조작을 하면 `map_notes`가 `NoteNotFound`로 거부하고 문서는 그대로다. 문서 변경 때 `retain_existing`이 걸러 낸다. 다만 선택이 「현재 클립의 노트」라는 보장은 없다(3절 시도 1·2) |
| 선택 유무 = FSM `Selection` 컨텍스트 | 아니오 | `sync_selection_mode`가 한곳. 트랙 삭제·undo·뷰 전환 모두 확인(시도 8, 시험 10·17) |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.7 표 (없음) 행 h/l/j/k | 충족 | S3-T1 처리기, 시험 `modifier_table_5_7_every_cell` 56칸 |
| 5.7 표 `Shift` 행 | 부분 충족 | `selection.rs:160 extend`. 기준 칸이 클립 밖이면 영원히 선택 없음(시도 3), 조작 뒤 넓히기가 선택을 지움(시도 6) |
| 5.7 표 `Ctrl` 행 | 충족 | S3-T1 `move_cursor` BarBack/BarForward/OctaveDown/Up, 표 시험 |
| 5.7 표 `Ctrl+Shift` 행 | 충족 | `extend(BarBack/BarForward/OctaveUp/Down)`, `Ctrl+h` 읽기와 같음(계획 1) |
| 5.7 표 `Alt` 행 | 부분 충족 | `selection.rs:308 apply_note_op`. 대상이 현재 클립·트랙 밖의 노트일 수 있다(시도 1·2, 5.3 위반) |
| 5.7 표 `Alt+Shift` 행 | 부분 충족 | 같은 이유 + 커서 노트 `Shorten` 연타 불가(시도 4). 「한 스텝은 Step 모드의 현재 길이」는 `step_ticks` 960 고정(S3-T4 이월, 계획 2에 명시) |
| 5.7 표 `Alt+Ctrl` 행 | 충족 | `NudgeBarEarlier/Later`, `j/k` (예약) 바인딩 없음(표 시험 `Nothing`) |
| 5.7 선택: `Shift+이동`, 선택 없으면 커서 노트, `Cmd+A`, `b`, `Shift+b`, `Esc` | 충족(단독 동작) | `selection.rs:278–304`, 시험 7·10 |
| 5.7 노트 단위 이동 `n` `p` `Shift+n` `Shift+p` `Cmd+←→↑↓` | 충족(`Shift+p` 읽기는 제안 7) | `selection.rs:440–500`, 시험 9 |
| 5.7 별칭 `+` `-` `>` `<` | 충족 | `default.json:100,103,114,117` |
| 5.7 되돌리기 묶음 | 충족 | `document.rs apply_coalesced/seal`, `window.rs observe_keystrokes`, 시험 8, 시도 9 |
| 5.7 길이 직접 지정 | 범위 밖(S3-T3) | 계획 「범위 밖」 |
| 5.3 「앞뒤 클립의 노트는 흐리게 보이고 편집은 되지 않는다」 | 미충족 | 시도 1: 커서가 클립 B에 있을 때 `alt-k`가 흐린 A의 노트를 바꿈. `selection.rs:308–333` 선택 분기에 현재 클립·트랙 검사 없음 |
| 5.3 「선택 → 동작」, 이동 시 미리듣기(기본 켬), `Esc` 선택 해제 | 충족 | `preview_cursor_note`(`selection.rs:675`), `settings.rs PreviewSettings::default`, 시험 11 |
| 5.8 `Esc` 「취소: 선택 해제 … 닫을 것이 없으면 아무 일도 없다」 | 충족 | 시험 10(두 번째 Esc no-op), 시도 8 |
| 5.8 `n`/`p`, `Shift+n`/`Shift+p`, `Cmd+화살표`, `[` `]` | 충족 | 시험 9, S3-T1 |
| 5.8 `Cmd+Shift+P` / `:` 행(ex 명령) | 충족 | `window.rs open_command_line/run_ex`, `ex.rs`, 시험 15. REQ 문구는 결정 A12와 일치 |
| 5.8 선택·편집 표 `Shift+이동`, `Cmd+A`/`b`/`Shift+b`, `Alt+이동`/`Alt+Shift+이동`, `+ - > <` | 5.7과 같음 | 위 |
| 5.8 `Space`/`Shift+Space` 커서부터 재생 | 충족(피아노롤) | `transport.rs:130–143`, 시험 18. 배치 뷰 커서는 S4 이월(계획) |
| 5.8.1 `g a`(A2) | 충족 | `vim.json:15`, 시험 12 |
| 5.8.1 `enter` 커서 노트 듣기(A8) | 충족 | `vim.json` `Normal > PianoRoll` `enter`, `note_audition`, 시험 12 |
| 5.8.1 `gb` `gB`(A10) | 충족 | `vim.json:16–17`, 시험 12 |
| 5.8.1 `f` `F` `;` `,`(A11) | 충족 | `vim.json:18–19`, `find_same_pitch`, 시험 12 |
| 5.8.1 `w`/`b` 다음·이전 노트, `{` `}` 클립, `gg`/`G` | 충족(S2-T2 바인딩, S3-T2 처리기) | `vim.json`, `cursor_next_note` 등 |
| 5.8.1 마크 `m a`, `' a` | 충족 | `cursor_set_mark/jump_mark`, 시험 16 |
| 5.8.1 `:` 명령줄 | 충족 | 위 `:` 행 |
| A13 `Normal > Operator` `escape`=`operator::Cancel` | 충족(컨텍스트는 S3-T3) | `vim.json`, `operator_cancel`, 시험 14. `review_s2_t3_r3` 기대값 갱신은 결정 표의 줄과 일치 |
| A13 시퀀스 대기 중 Esc는 시퀀스만 취소 | 부분 충족 | `keymap/mod.rs:333–345`. 기본·vim 프리셋에서는 맞음(시험 13). 사용자 키맵이 `f20`을 걸면 그 Action이 실행(시도 7) |
| PRE-1 미리듣기: 해당 트랙의 포트·채널, 길이 설정(기본 200 ms) | 충족 | `preview.rs sound_note`, `settings.rs:194–240`, 시험 11(채널·벨로시티·200 ms·50 ms) |
| 7.5 `Cmd+A` 전체 선택(피아노롤은 클립 안 노트 전부) | 충족 | `select_all`, 시험 10, 시도 12·13 |
| 7.5 vim 대응 `:new` `:e` `:w` `:w name` `:q` `:set` | 충족 | `ex.rs:73–87`, `run_ex`, 시험 15. `:w name`은 이름 제안 시트(계획 「범위 밖」에 명시) |
| 7.4 「마우스 … 같은 선택 상태를 공유」 | 범위 밖(S3-T6) | 계획. `Selection` 타입은 공유 가능 |
| UI-11 마크의 눈에 보이는 자리(결정 a) | 충족(피아노롤) | `open_piano_roll_menu` 하위 메뉴 둘, 시험 16. 배치 뷰는 S4-T1 이월 기록(`14-S4` 파일) |
| UI-11 새 Action 4개 | 충족 | `commands/table.rs:1063,1120,1431,1443`, `commands` 시험 통과 |
| KM-5 vim 카운트 비움 | 충족 | `operator_cancel` → `clear_count`, 시험 14 |
| EDT-1 undo | 충족 | `apply_coalesced`가 `Document::apply(…, Some(CoalesceKey))` |
| REQ 문구(결정 「후속」): 5.8 `:` 행, 5.8.1 `g` 행 `gb` `gB`, `;` 행, `.` `,` 행, 충돌 메모 `gv`→`ga`·`b`/`B` | 충족 | `docs/REQ.md` diff가 결정 A2·A10·A11·A12 권고와 글자 그대로 맞음. `.` `,` 행의 유래 열에 `,`를 더한 것도 A11과 일관 |

「부분 충족」은 미충족으로 센다: 5.7 `Shift`·`Alt`·`Alt+Shift` 행, 5.3 편집 불가, A13 `f20`.

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 이 Task는 `keyscore-midi` 스케줄러 스레드 코드를 건드리지 않는다. 미리듣기는 `Scheduler::preview`(명령 큐)와 `send_now`만 쓴다. 단, UI 스레드에서 정지 중 이름 포트 트랙의 미리듣기가 커서 이동마다 `engine.prepare`(프로젝트 전체 스냅샷) + `collect_garbage` + `swap`을 한다(`preview.rs:115–132`). OS 키 반복(초당 수십 회)으로 노트 위를 지나면 그만큼 스냅샷을 만든다. 보고서는 뱅크·프로그램 반복 전송만 한계로 적었다(제안 10).
- 제품 코드의 `unwrap`/`expect`(diff에 더해진 것):
  - `inspector.rs:311` `Pitch::new(TEST_PITCH).expect("가운데 C")`: `TEST_PITCH`는 `const 60`이라 실패하지 않는다. S2-T5c는 `map_err(InvalidMessage)`였는데 `expect`로 바뀌었다. 상수라 정당하나 `00-conventions`의 `expect` 금지 취지(이유 주석)에 맞춰 「상수 60은 0–127 안」을 주석으로 적거나 `Pitch`를 `const`로 만들 수 있다(제안 12).
  - `ops.rs:58` `unwrap_or_default()`: `NoteOp::ALL.position(self)`는 항상 `Some`. 정당.
  - `select.rs:41` `bar_start(bar).unwrap_or(Tick::ZERO)`, `ops.rs:121–122` `map_or`: `bar_start`는 u64 범위 밖에서만 `None`. 정당.
  - `selection.rs:356` `planned.moved[0]`: `plan_note_op`가 `targets.is_empty()`면 `Ok(None)`을 돌려 여기 오지 않는다. 정당하나 인덱스 대신 `first()`가 안전하다(스타일이 아니라 패닉 경로 제거).
- 에러가 삼켜지는 곳:
  - `selection.rs:686` `let _ = self.sound_note(…)`: 계획 4에 명시(이동마다 상태줄을 덮지 않기 위해). 받아들임. 다만 「MIDI 엔진 없음」이 영구 상태라 사용자가 미리듣기가 왜 안 되는지 알 길이 없다. `note::Audition`은 알린다.
  - `settings_window.rs:171–178` `SetPreviewLength` 범위 밖·음수: 조용히 무시(알림 없음). 팔레트 변형은 모두 범위 안이라 사용자 경로에서는 닿지 않는다.
  - `selection.rs cursor_repeat_find`: `f`를 쓴 적이 없으면 조용히 no-op(계획 3에 명시).
  - `files.rs open_ex_path`: 실패는 상태줄에 알린다. 좋다.
  - `keymap/mod.rs:341` `Keystroke::parse(FLUSH_KEY)` 실패는 조용히 무시(상수라 실패하지 않음).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | 선택이 가리키는 (트랙, 클립)이 커서의 현재 트랙·클립과 다를 때 `Alt`·`Alt+Shift` 조작이 그 선택을 편집한다. 커서가 클립 B에 있으면 A의 노트는 흐리게 그려지고 선택도 그려지지 않는데(`scene.rs` `selected: !dim && …`) 편집이 되므로 REQ 5.3 「앞뒤 클립의 노트는 … 편집은 되지 않는다」 위반이고, 트랙을 바꾼 뒤에는 화면에 전혀 없는 노트가 바뀐다(데이터 손실 경로). 보고서 「리뷰어에게」가 이것을 의도라 적었으나 REQ와 어긋난다. 고치는 길은 둘 중 하나로 보인다: (a) 커서가 선택의 클립 밖으로 나가거나 트랙이 바뀌면 선택을 비운다(`cursor_moved`·`set_current_track`에서 `selection.clip != current_clip`이면 `set_selection(default)`), (b) `apply_note_op`에서 선택의 (트랙, 클립)이 현재와 다르면 거부하고 알린다. 어느 쪽이든 계획 1에 한 줄로 적는다. 시도 1·2 | `crates/keyscore-app/src/selection.rs:308–333`, `scene.rs:246` |
| 2 | 수정 | `Shift+이동`의 기준 칸이 어느 클립에도 들지 않으면(`clip_at(anchor.tick)` = `None`) 커서가 클립 안으로 들어가도 아무것도 선택되지 않고, 기준 칸은 수식키 없는 이동 전까지 남아 그 뒤의 모든 `Shift+이동`도 빈 선택이다. 클립을 커서(또는 직사각형이 걸치는 클립)에서 고르거나, 기준 칸이 클립 밖이면 커서 쪽 클립을 쓴다. 시도 3 | `selection.rs:160–185` |
| 3 | 수정 | 선택 없이 커서 노트에 `Shorten`을 적용하면 커서가 노트 끝 뒤에 남아 두 번째 `Shorten`이 「커서 위에 노트가 없다」로 실패한다. 5.7 「같은 조작을 연속으로」가 `<` 연타에서 성립하지 않는다. 시간·음높이처럼 길이 조작 뒤에도 커서를 노트 안으로(예: `min(cursor.tick, new.end − 1)`) 옮긴다. 시도 4 | `selection.rs:353–372`, `ops.rs:79–98`(`moves_time`/`moves_pitch`만 있음) |
| 4 | 수정 | 선택에 `Alt` 조작을 한 뒤 `Shift+이동`으로 더 넓히면 낡은 기준 칸↔커서 직사각형으로 선택을 다시 계산해, 방금 옮긴 노트(음높이·시간이 바뀜)가 선택에서 떨어진다. 「넓히기」가 선택을 비우는 결과다. 조작 뒤 기준 칸을 지우거나(그러면 다음 `Shift+이동`은 커서 칸이 기준), 기준 칸을 노트와 함께 옮긴다. 시도 6 | `selection.rs:160–185`, `selection.rs:373–376`(선택 갱신 뒤 anchor를 그대로 둠) |
| 5 | 수정 | 시퀀스 대기 중 Esc가 대기를 비우려 `f20`을 디스패치하는데 사용자 키맵이 `f20`을 걸면 그 Action이 실행된다(A13 위반). 보고서가 한계로 적었지만 고칠 수 있다: 사용자 레이어 파서가 `f20`(또는 전용으로 예약한 키)을 `KeymapError`로 거부하거나, 디스패치 전에 `window.bindings_for_input`으로 `f20` 바인딩이 없음을 확인하고 있으면 다른 예약 키를 쓴다. 시도 7 | `crates/keyscore-app/src/keymap/mod.rs:243, 333–345` |
| 6 | 수정 | 누락 시험: 위 1–5의 경우. `review_s3_t2.rs`의 실패 7개가 그대로 쓸 수 있는 시험이다(5번 `Shift+p`는 제안 7의 결정에 따라 기대값을 정한다). 고친 뒤 이 파일을 PR에 두고 `brk_` 접두 그대로 통과시키면 된다 | `crates/keyscore-app/tests/review_s3_t2.rs` |
| 7 | 제안 | `Shift+p`가 클립 중간에서 「현재 클립의 시작」으로 간다(`prev_clip_start`: `start < tick`). REQ 5.8은 「이전 클립으로」다. S3-T1 리뷰가 `Ctrl+h`를 「마디 중간이면 그 마디 시작」으로 읽고 계획에 고정한 것과 같은 읽기라 받아들일 수 있으나, 계획 3의 문장(「커서 틱 뒤·앞에서 시작하는 클립」)만으로는 이 읽기가 드러나지 않는다. `Ctrl+h`처럼 계획에 한 줄로 적고 시도 5의 기대값을 그에 맞춘다. REQ 글자대로 가려면 `prev_clip_start`를 「커서가 든 클립의 앞 클립」으로 바꾼다 | `pianoroll/select.rs:120–126`, `docs/reports/s3-t2/plan.md` 3 |
| 8 | 제안 | undo·redo로 선택 노트의 값이 바뀌면(`ReplaceNotes` 역적용) `retain_existing`이 그 노트를 떨어뜨려 선택이 빈다(시험 17이 이것을 기대값으로 고정). `Alt+k` ×5 → `Cmd+Z` 뒤 다시 조작하려면 다시 골라야 한다. `ReplaceNotes`의 짝(원래 ↔ 바뀐)을 알 수 있으므로 undo 뒤 선택을 원래 노트로 되살리는 편이 5.7 「선택 → 동작」 흐름에 맞다. S3-T3(편집)과 함께 정해도 된다 | `pianoroll/mod.rs:207–220`, `selection.rs:118–140` |
| 9 | 제안 | 정지 중 이름 포트 트랙의 미리듣기가 커서 이동마다 `engine.prepare`(프로젝트 전체 스냅샷) + `collect_garbage` + `swap`을 한다. 테스트 노트(사람이 버튼을 누름)에는 맞는 경로지만 OS 키 반복으로 노트 위를 지나면 초당 수십 번이다. 「같은 프로젝트 revision이면 스냅샷을 다시 만들지 않는다」 또는 포트만 여는 가벼운 경로를 둔다. 보고서의 「뱅크·프로그램 반복 전송」 한계도 같은 곳이다 | `crates/keyscore-app/src/preview.rs:107–134` |
| 10 | 제안 | `:e 경로`: 틸드(`~/x.ksq`)를 풀지 않고, 문서에 경로가 없으면 `current_dir()` 기준이라 Finder로 띄운 앱에서는 `/` 기준이 된다. 틸드는 홈으로 풀고, 경로 없는 문서는 `default_directory`(저장 시트가 쓰는 것)를 기준으로 한다 | `crates/keyscore-app/src/files.rs:300–322` |
| 11 | 제안 | `preview_cursor_note`가 `NoEngine`을 영구히 조용히 넘긴다. 첫 실패 한 번만 알리면(`notice_history_limit`처럼) 사용자가 미리듣기가 안 되는 이유를 안다 | `selection.rs:675–687` |
| 12 | 제안 | `inspector.rs:311` `expect("가운데 C")`: 상수라 안전하지만 이유 주석이 없다. S2-T5c의 `map_err`를 되돌리거나 「`TEST_PITCH`=60은 0–127 안」 주석을 단다. `selection.rs:356` `planned.moved[0]`은 `first()`로 패닉 경로를 없앨 수 있다 | `inspector.rs:311`, `selection.rs:356` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단 1(REQ 5.3 위반, 보이지 않는 노트 편집)과 수정 2–5를 고치고, `review_s3_t2.rs`의 실패 시험이 통과하도록 한 뒤 재리뷰한다. 제안 7–12는 거절할 수 있으며 거절 이유를 보고서에 적는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다(13개, 7개 깨짐)
- [x] REQ ID를 하나씩 대조했다(5절)
- [x] `git diff`에 Task 범위 밖의 변경이 없다. 42개 파일 모두 계획의 인터페이스·시험·문서 안이다. `inspector.rs`의 큰 삭제는 `preview.rs`로 옮긴 것(동작 동일, `inspector` 시험 통과를 보고서가 적음). `plan/stages/14-S4` 한 줄은 이월 기록
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다. 기존 시험 변경 5건은 기대값 갱신이고 모두 의도한 변경과 1:1로 맞는다(`review_s2_t4_r3`의 `None` → `Some("G A")`는 A2 결정의 결과이고 가려진 `v`가 보이지 않는 요점은 그대로)
- [x] 새 의존성이 있으면 라이선스를 확인했다 — 새 의존성 없음(`Cargo.toml`·`Cargo.lock` 변경 없음)
- [x] 스타일 지적을 적지 않았다
