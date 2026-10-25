# Task 보고: S3-T2 커서·선택·수식키 체계

- PR: 로컬 브랜치 `s3/t2-cursor-selection-modifiers`
- 계획 파일: docs/reports/s3-t2/plan.md
- 계획 대비 달라진 점
  - `Shift+이동`의 기준 칸을 `Selection`이 아니라 `PianoRollState::anchor`에 둔다. 빈 칸을 여러 번 지나 노트까지 넓힐 때 선택이 비어도 기준 칸이 남아야 하기 때문이다. `Selection::set`은 (트랙, 클립, 노트) 세 인자다. 계획 파일을 고쳤다.
  - core의 `Note::with_start/with_pitch/with_length`를 공개했다(`pub(crate)`였다). 앱이 `transform::map_notes`의 클로저에서 노트 변형을 만들려면 필요하다. 각각 자기 검사(길이 0, 오버플로, 절대 피치)를 하고 규칙은 바뀌지 않는다.
  - `commands::ex::is_command_line`, `CommandPalette::set_query_text`, `PaletteEvent::Ex`, `ProjectWindow::open_popup_with`(하위 메뉴용), `NoteOp::ALL`·`moves_time`·`moves_pitch`를 더했다.
  - 테스트 노트(S2-T5c)의 보내기 경로를 `src/preview.rs`의 `sound_note`로 옮겨 미리듣기와 공유한다. 인스펙터의 동작·문구는 같다.

## 구현 요약

- `crates/keyscore-app/src/pianoroll/select.rs`(순수): `region_notes`(기준↔커서 직사각형, 시간 `[min, max + step)` 겹침, 음높이 양끝 포함), `bar_notes`, `note_under`, `next_note`/`prev_note`((절대 시작, 음높이) 순, 클립 경계를 넘음), `next_same_pitch`/`prev_same_pitch`, `next_clip_start`/`prev_clip_start`, `clip_last_step`.
- `crates/keyscore-app/src/pianoroll/ops.rs`(순수): `NoteOp` 10개와 `plan_note_op`. `transform::map_notes`로 `ReplaceNotes`를 만들어 S0-T7 규칙(마디 단위 연장, 다음 클립 겹침·시작 앞 거부, 꼬리)을 그대로 따른다. 음높이 밖은 전부 거부(접기 없음), 한 스텝 이하 줄이기는 그 노트를 건너뛰고 모두 건너뛰면 `Ok(None)`. 조작마다 다른 `coalesce_key`.
- `crates/keyscore-app/src/pianoroll/mod.rs`: `Selection`(트랙·클립·노트 집합, `contains(clip, note)`, `retain_existing`), `Mark`, `FindDirection`, 상태에 `anchor`·`marks`·`last_find`. `scene.rs`는 선택의 클립이 현재 클립일 때만 `selected`.
- `crates/keyscore-app/src/selection.rs`: 창 처리기. `Shift+이동` 8개(`extend`: 커서를 같은 규칙으로 옮기고 기준 칸이 든 클립에서 직사각형 선택), `b`/`Shift+b`/`Cmd+A`/`Esc`, `Alt` 조작 10개(`apply_note_op`: 선택 없으면 커서 노트, `apply_coalesced`, 성공 뒤 선택 갱신 또는 커서 따라가기), `n`/`p`/`Shift+n`/`Shift+p`/`Cmd+←→`, vim `f`/`F`/`;`/`,`, 마크 설정·점프(트랙 전환 포함), 우클릭 메뉴(`PopupMenu::submenu` 둘, 26항목씩), `note::Audition`, `operator::Cancel`. `cursor_moved`가 뷰포트 따라가기·미리듣기·기준 칸 지우기를 한곳에서 한다. `sync_selection_mode`가 모드 FSM의 `Selection` 컨텍스트를 실제 선택에 맞추고, `selection_document_changed`가 문서 변경 뒤 없는 노트를 뺀다.
- `crates/keyscore-app/src/preview.rs`: `sound_note(track, pitch, velocity, length)` → `NoteSend`. 이 창 재생 중이면 그 스냅샷 포트, 다른 창 재생 중이면 기본 포트만, 정지 중이면 스냅샷을 넣어 포트를 열고 뱅크·프로그램을 보낸다. `inspector.rs`의 테스트 노트가 이것을 쓴다.
- `crates/keyscore-app/src/window.rs`: `set_selection`, `apply_selection_event`, `context_menu_open`, `open_popup_with`, `PopupKind::PianoRollMenu`, 키 입력 관찰(`observe_keystrokes`: 이 창의 키 입력이 `Alt` 조작 Action이 아니면 `seal`), 문서 변경 구독에 선택 가지치기, `apply_mode_event`가 FSM 선택 해제 때 실제 선택도 비움, `:` 명령줄(`open_command_line`, `run_ex`). `pianoroll_window.rs`: 클릭은 `seal` + 커서, 우클릭은 마크 메뉴. `element.rs`: 우클릭은 뗄 때(누를 때는 창의 겹 바깥 누름 처리기가 포커스를 문서로 돌려 메뉴가 포커스를 잃는다).
- `crates/keyscore-app/src/document.rs`: `apply_coalesced(edits, key)`(`core::Document::apply(…, Some(CoalesceKey))`), `seal`.
- `crates/keyscore-app/src/commands/ex.rs`: `parse`(`:w [이름]` `:q` `:e [경로]` `:new` `:set [구역]`, 모르는 명령·인자는 `ExError`), `label`. `command_palette.rs`: `:` 접두면 Action 줄 대신 ex 줄 하나(`palette-row-ex`, 모르는 명령은 꺼진 줄), 확정은 컴포넌트가 질의를 비우기 전의 값(`query_text`)으로. `files.rs`: `file_save_as_named`(이름 제안, 확장자 없으면 `.ksq`), `open_ex_path`(문서 폴더 기준, `defer`로 연다).
- `crates/keyscore-app/src/keymap/mod.rs`: 가로채기에서 「대기 중 + 수식키 없는 `escape` + 프로젝트 창」이면 전파를 멈추고 카운트를 비우고, `Window::defer`로 바인딩 없는 키 `f20`을 디스패치해 GPUI가 대기를 시간 초과처럼 비운다.
- `crates/keyscore-app/src/settings.rs`: `PreviewSettings { on_move: true, length_ms: 200 }`(`preview` 객체, 1–10000 ms, 모르는 키 보존). `settings_tabs.rs`·`settings_window.rs`: 설정 > 입력에 「이동 시 미리듣기」(켬·끔)와 「미리듣기 길이」 선택 상자, 전역 처리기 `settings::TogglePreviewOnMove`·`SetPreviewLength`.
- `crates/keyscore-app/src/transport.rs`: `Shift+Space`는 피아노롤 뷰에서 피아노롤 커서 틱부터.
- `crates/keyscore-app/src/actions.rs`·`commands/table.rs`: 새 Action 4개(`operator::Cancel`, `palette::OpenCommandLine`(보기 메뉴 「명령줄」), `settings::TogglePreviewOnMove`, `settings::SetPreviewLength`(변형 100·200·300·500)). 301개.
- `assets/keymaps/default.json`: `:`=`palette::OpenCommandLine`. `vim.json`: `g a`, `g b`, `g shift-b`, `;`, `,`, `Normal > PianoRoll` `enter`=`note::Audition`, `Normal > Operator` `escape`=`operator::Cancel`. 결정 대기 주석은 남은 항목만 「결정됨 … 반영 단계가 남은 것」으로.
- `crates/keyscore-core`: `transform::map_notes` 공개, `Note::with_start/with_pitch/with_length` 공개.
- `docs/REQ.md`(결정 문서 「후속」, 개발 단계 수정 승인): 5.8 `:` 행, 5.8.1 `g` 행(`gb` `gB`)·`;` 행·`.` `,` 행, 충돌 메모 `gv`→`ga`.
- `docs/decisions/2026-10-25-playback-end-of-song.md`: 곡 끝 동작 결정 요청(이 Task는 구현하지 않는다).

## 테스트 결과

~~~
cargo test -p keyscore-core --test transform_map: 4 passed(proptest 2: map_notes = shift / transpose)
cargo test -p keyscore-app --test pianoroll_select: 14 passed
cargo test -p keyscore-app --test command_line: 7 passed
영향받는 기존 시험: commands 12, review_s2_t4 15, settings_tabs 8, keymap_engine 11, keymap_overlay 4, modes_ui 8,
  inspector 11, pianoroll 13, review_s3_t1 19, review_s3_t1_r2 10, count_non_key_paths 2, carry_overs 3, window 12,
  status_bar 2 모두 통과
cargo test --workspace --no-fail-fast: 1940 passed, 3 failed, 2 ignored(1시간판 측정용). 실패 3은 의도한 변경이 깬
  기존 리뷰 시험으로 기대값을 고쳤다(아래). 고친 뒤 세 바이너리 재실행 9·17·13 passed
scripts/bench-check.py: pianoroll/scene_10k_notes 26.4 µs(기준 29.7 µs, -11%), 나머지 기준 안(ksq/read +9.0%는 소음 범위)
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `region_notes_picks_notes_overlapping_cells_between_anchor_and_cursor` | 통과(뼈대에서 실패) |
| 2 | `next_prev_note_walk_track_notes_in_start_pitch_order` | 통과(뼈대에서 실패) |
| 3 | `bar_notes_and_clip_last_step` | 통과(뼈대에서 실패. 기대값 둘을 고쳤다: 1000틱 클립의 마지막 칸은 960, 2000틱은 1920) |
| 4 | `plan_note_op_rules` | 통과(뼈대에서 실패. 「하나라도 밖이면 전부 거부」 입력을 127+1로 고쳤다) |
| 5 | `ex_parse_maps_w_q_e_new_set_and_rejects_unknown` | 통과(뼈대에서 실패) |
| 6 | `modifier_table_5_7_every_cell` | 통과(뼈대에서 실패). 7행 × 4열 × {hjkl, 화살표} 56칸. 단계 파일 게이트 |
| 7 | `alt_operations_without_selection_apply_to_cursor_note` | 통과(뼈대에서 실패). 단계 파일 게이트 |
| 8 | `repeated_same_operation_is_one_undo_and_other_key_breaks_group` | 통과(뼈대에서 실패). 리뷰 초점 |
| 9 | `n_p_shift_n_shift_p_cmd_arrows_move_cursor` | 통과(뼈대에서 실패) |
| 10 | `b_shift_b_cmd_a_select_and_escape_clears` | 통과(뼈대에서 실패) |
| 11 | `cursor_move_onto_note_sends_preview_on_track_port_channel` | 통과(뼈대에서 실패. 첫 구현 뒤 시험의 커서 순서를 고쳤다: `l`은 음높이를 바꾸지 않는다) |
| 12 | `vim_preset_keys_from_decisions_a2_a8_a10_a11` | 통과(뼈대에서 실패) |
| 13 | `escape_during_pending_sequence_only_cancels_sequence` | 통과(뼈대에서 실패) |
| 14 | `operator_cancel_clears_count` | 통과(뼈대에서 실패) |
| 15 | `colon_opens_palette_with_ex_prefix_and_runs_commands` | 통과(뼈대에서 실패. 두 곳을 고쳤다: 팔레트의 Esc는 글자가 있으면 먼저 질의를 비우는 컴포넌트 규칙(S2-T4)에 맞춰 두 번, `:e` 결과 경로는 정규화 비교) |
| 16 | `marks_set_and_jump_and_context_menu_opens` | 통과(뼈대에서 실패. 첫 구현은 우클릭을 누를 때 열어 메뉴가 포커스를 잃었다 → 뗄 때로) |
| 17 | `selection_follows_document_changes` | 통과(뼈대에서 실패) |
| 18 | `play_from_cursor_starts_at_piano_roll_cursor` | 통과(뼈대에서 실패) |
| 19 | `preview_settings_round_trip_and_defaults`, `preview_settings_have_controls_and_actions` | 통과(뼈대에서 실패) |
| 20 | `scene_marks_selected_only_in_selected_clip` | 통과. 뼈대 커밋에서 이미 통과했다: `Selection` 타입 변경이 `scene.rs`의 `contains(id, note)` 수정을 강제해 뼈대에 들어갔다. 이 시험이 잡는 것은 `Selection::contains`의 클립 판정이다 |
| 21 | `action_counts_match_palette_and_menus`(기존) | 통과(새 Action 4개) |
| 22 | `map_notes_matches_shift`, `map_notes_matches_transpose_inside_range`(core, proptest) + `map_notes_rejects_duplicate_picks`, `map_notes_extends_clip_by_bars_when_length_grows` | 통과(뼈대에서 실패) |

계획에 있었지만 쓰지 않은 테스트와 이유: 없음. 기존 시험 변경 5건(지운 시험·`#[ignore]`는 없다):
- `review_s2_t4::colon_typed_in_open_palette_goes_to_query`: `:`가 접두를 채우므로(A12) 기대 질의가 `:a:b`.
- `settings_tabs::settings_sections_keyboard_reachable`, `review_s2_t6c::brk_calibrate_by_space`: 입력 구역의 Tab 순서에 미리듣기 컨트롤 둘이 들어간다.
- `review_s2_t3_r3::vim_layer_does_not_change_any_escape_binding`: vim 전용 컨텍스트 `Normal > Operator`의 `escape`=`operator::Cancel`(A13 결정 표)이 더해진다. 기본 바인딩을 가리거나 바꾸는 것은 여전히 없다.
- `review_s2_t4_r3::palette_does_not_show_shadowed_key_in_vim`: vim의 `view::Toggle` 키가 `g a`(A2)로 생겨 팔레트가 「G A」를 보인다. 가려진 `v`가 보이지 않는 요점은 그대로다.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings(워크스페이스, 모든 타깃)
- [x] test: `cargo test --workspace --no-fail-fast` 1940 passed, 3 failed(기대값 갱신으로 해결, 재실행 통과), 2 ignored
- [x] deny: advisories·bans·licenses·sources ok
- [x] `scripts/check-deps.sh` ok, `scripts/check-deps.test.sh` 16 passed, `scripts/bench-check.test.sh` 8 passed(의존성 변경 없음)
- [x] 단계별 추가 게이트: 5.7 표 테이블 시험(6), 선택 없을 때 `Alt` 조작(7). `scripts/bench-check.py` 통과(`pianoroll/scene_10k_notes` -11%)

## 알려진 한계

- 선택 영역의 기하(직사각형, 겹침 기준)와 「선택 없이 커서 노트에 적용하면 커서가 따라간다」는 REQ에 없는 세부로 계획에 적어 두었다. 리뷰어가 다르게 읽으면 고친다.
- 되돌리기 묶음은 「이 창의 키 입력 가운데 같은 조작의 Action이 아닌 것」이 끊는다. 메뉴·팔레트로 실행한 같은 조작은 끊지 않는다(계획대로). 다른 창에서 친 키는 보지 않는다.
- 시퀀스 대기 중 `Esc`는 GPUI 공개 API에 대기를 비우는 것이 없어 바인딩 없는 키 `f20`을 디스패치해 비운다. 사용자 키맵이 `f20`을 걸면 그 Action이 실행된다. GPUI 스냅샷이 바뀌면 다시 본다.
- 미리듣기는 정지 중 이름 포트 트랙에서 이동마다 스냅샷을 넣어 포트를 열고, 기본 포트 트랙에서는 뱅크·프로그램을 매번 먼저 보낸다(테스트 노트와 같은 경로). 이동이 잦으면 MIDI 메시지가 그만큼 나간다. 실제 신디에서 거슬리면 「같은 트랙이면 뱅크·프로그램을 다시 보내지 않기」를 더한다.
- `:w 이름`은 그 이름을 제안하는 「다른 이름으로 저장」 시트다(바로 쓰지 않는다). `:e 경로`의 상대 경로는 문서 폴더(없으면 현재 폴더) 기준이다. `:ai` `:test`는 모르는 명령이다(S7·S8).
- 팔레트 안에서 `:`를 치면 글자다(질의 `::`). 컴포넌트의 Esc는 글자가 있으면 먼저 비운다(S2 출구 「팔레트 첫 Esc」 수동 항목, 이 Task 범위 밖).
- 배치 뷰의 우클릭 마크 메뉴·`Cmd+A` 클립 전체·배치 뷰 커서부터 재생은 S4-T1 이월. 마크·`Shift+n` 등은 창에 하나인 피아노롤 커서를 움직인다.
- 곡 끝 동작은 결정 요청(`2026-10-25-playback-end-of-song.md`)으로 남겼다.
- 실제 창에서 눈으로 확인하지 못했다(S3-T1과 같은 이유). 사람이 `cargo run -p keyscore-app --release`로 픽스처를 열어 `shift-l`로 노트가 강조되는지, `alt-k`로 올라가는지, 우클릭 메뉴가 뜨는지, `:set`이 설정 창을 여는지 본다.

## 리뷰어에게

- `selection.rs` `extend`: 기준 칸이 든 클립을 고르는 `clip_at(anchor.tick)`과, 커서가 클립 밖으로 나갔을 때(선택이 비어도 기준 칸은 남음)의 동작을 깨뜨려 보라. 기준 칸 생명주기(수식키 없는 이동·`b`류·`Esc`·뷰 전환이 지움)는 `cursor_moved(keep_anchor)`·`select_in_current_clip`·`select_clear`·`apply_mode_event`에 흩어져 있다.
- `apply_note_op`의 커서 따라가기(`moves_time`은 `to.start − from.start`만큼 커서 틱을 옮김)와, 선택이 다른 트랙·클립을 가리킬 때(현재 트랙이 바뀐 뒤 `Alt+k`) 그 선택이 적용되는 것이 의도다. 다르게 읽히면 지적해 달라.
- 되돌리기 묶음을 끊는 `observe_keystrokes`는 처리기 뒤에 돈다. 같은 키가 `note::Lengthen`을 내지만 처리기가 거부해(클립 겹침) 편집이 없을 때도 묶음은 유지된다(끊을 이유가 없다). 입력란에 글자를 칠 때도 `seal`이 돈다(문서는 바뀌지 않음).
- `keymap/mod.rs`의 Esc 가로채기: `cx.stop_propagation()` 뒤 GPUI가 `finish_dispatch_key_event`의 캡처 단계 리스너를 한 번 더 부를 수 있다(앱에 키 리스너는 설정 창 선택 상자뿐). `f20` 디스패치는 `Window::defer`라 같은 프레임의 뒤에 돈다.
- `preview.rs`는 S2-T5c 테스트 노트 코드를 옮긴 것이다. 동작 차이가 생겼는지 `inspector.rs` 시험(11개 통과)과 대조해 달라.
- `command_palette.rs`: `confirm`은 컴포넌트가 질의를 비우기 전의 값(`query_text`)을 읽는다. ex 줄은 `visible`에 없고 `items` 끝에 붙는다(`:` 접두면 `visible`은 비어 있어 줄은 하나다).
