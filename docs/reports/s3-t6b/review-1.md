# 적대적 리뷰: S3-T6b 드래그 이동·조절 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트, 구현 컨텍스트를 물려받지 않음)
- 대상 커밋: `2f93053`(브랜치 `s3/t6b-mouse-drag`, main 대비 `7fa5247`..`2f93053` 네 커밋)
- 리뷰 시험: `crates/keyscore-app/tests/review_s3_t6b.rs`(`CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t6b`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

시험 파일을 열기 전에 `ops.rs`·`mouse.rs`·`pianoroll_mouse.rs`의 diff만 읽고 적었다.

1. 일반 변환 `shift_note`가 노트의 나머지 필드(가사 LYR-1, 피치 오프셋, `.ksq`의 모르는 키 `extra`)를 보존하는가. main의 `plan_note_op`은 `with_start`·`with_pitch`·`with_length`를 썼는데 새 코드는 `Note::new(start, length, pitch, velocity)`로 노트를 새로 만든다. 키 `Alt` 조작도 이 길로 바뀌었으므로 회귀일 수 있다.
2. 키 조작과 마우스가 같은 계산인가(리뷰 초점): `plan_note_op` == `plan_shift(op.shift(step))`, 마우스는 `plan_shift`·`plan_shift_copy`만 부르는가. 마우스 전용 편집 경로가 있는가.
3. 드래그 중 `Esc`: `Esc` 가로채기가 `modifiers.modified()`면 넘긴다. 이 Task의 드래그는 `Alt`·`Cmd`·`Shift`를 누른 채 하는 조작이 셋이라, 수식키를 쥔 채 `Esc`를 누르면 취소되지 않을 수 있다.
4. 키(`Cmd+A`, `Shift+이동`)로 만든 선택을 마우스로 끌 수 있는가(7.4 「`v`로 고른 뒤 마우스로 끌어도 된다」). 누름이 기준 칸을 지우고 커서를 옮기는데 그때 선택이 남는가.
5. 놓는 순간의 `Alt`(결정 6절 8): 끄는 중 `Alt`를 눌렀다 떼면 이동, 쥐고 놓으면 복제인가.
6. 끌었다가 누른 자리로 돌아오면 항등이고 편집·undo가 없는가(`Alt`를 쥐고 있어도).
7. 끝 조절의 「시작 뒤 첫 격자선」·「끝 앞 마지막 격자선」이 끄는 방향과 반대로 노트를 늘이는 경우.
8. 고스트와 적용이 같은 계산인가: 뗌이 `update_move`로 다시 계산한 뒤 `finish_move`가 같은 변환으로 계획을 다시 만든다. 캐시 열쇠(`key`)가 같으면 고스트를 다시 만들지 않는 점이 어긋남을 낳는가.
9. `map_notes` 콜백 순서에 기대는 `plan_shift`(같은 노트가 둘 이상인 클립).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 가사·오프셋 보존 | 없음 | 모든 기존 시험의 노트가 가사·오프셋 없음. 리뷰 시험으로 깨짐 확인 |
| 2 같은 계산 | 있음 | `note_ops_equal_plan_shift_of_their_shift`(열 가지 조작, 박자표 변화) |
| 3 수식키 쥔 `Esc` | 없음 | `escape_during_drag_cancels_and_release_does_nothing`은 수식키 없는 끌기만. 리뷰 시험으로 깨짐 확인 |
| 4 키 선택 뒤 끌기 | 없음 | 리뷰 시험으로 확인, 못 깨뜨림 |
| 5 놓는 순간의 `Alt` | 부분 | `alt_drag_duplicates_with_one_undo`는 쥐고 놓는 경우만. 리뷰 시험으로 떼고 놓는 경우 확인 |
| 6 제자리 복귀 | 없음 | 리뷰 시험으로 확인, 못 깨뜨림 |
| 7 끝 조절 역방향 | 부분 | `drag_shift_end_snaps_and_keeps_grabbed_length`는 시작 이하로 끈 경우만 |
| 8 고스트 = 적용 | 있음 | `ghost_shows_result_while_document_stays`, `cmd_during_drag_disables_snap` |
| 9 같은 노트 여럿 | 없음 | 아래 4절. 결과가 같은 노트끼리라 순서가 무관함을 코드로 확인 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 가사·피치 오프셋이 붙은 노트에 열 가지 키 `Alt` 조작(`plan_note_op`) | 깨짐: 첫 조작(`NudgeEarlier`)부터 가사가 `None` | `review_key_alt_ops_keep_lyric_and_pitch_offset` |
| 2 | 가사가 붙은 노트를 본체 드래그로 한 박 옮김 | 깨짐: 옮겨지지만 가사가 사라짐 | `review_body_drag_keeps_lyric` |
| 3 | `Alt` 복제 끌기 중 `alt-escape` | 깨짐: 조작이 살아 있고 놓으면 복제됨 | `review_escape_with_alt_held_cancels_copy_drag` |
| 4 | `Shift` 한 축 끌기 중 `shift-escape` | 깨짐: 놓으면 N2가 1920으로 옮겨짐, undo 하나 쌓임 | `review_escape_with_shift_held_cancels_axis_drag` |
| 5 | `Cmd+A`(키 선택) 뒤 본체를 한 행 위로 끌기 | 못 깨뜨림: 일곱 노트 모두 +1, undo 하나 | `review_key_selection_then_drag_moves_all` |
| 6 | 멀리 끌었다가 `Alt`를 쥔 채 누른 자리로 돌아와 놓음 | 못 깨뜨림: 고스트 없음, 편집·undo 없음, 선택 그대로 | `review_drag_back_to_origin_makes_no_edit` |
| 7 | 끄는 중 `Alt`를 눌렀다 떼고(수식키 사건만) 놓음 | 못 깨뜨림: 복제 없이 이동 | `review_alt_released_before_drop_moves` |
| 8 | 격자 밖 짧은 노트 `[10, 110)`의 오른쪽 끝을 20틱 왼쪽으로(8분 격자) | 결정 글대로 동작(끝 Δ +370, 끝 480으로 길어짐). 결함이 아니라 기록 | `review_end_grip_left_drag_on_short_offgrid_note_lengthens`(통과) |

기존 `mouse_drag`(18), `mouse_drag_pure`(15), `pianoroll_select`(14)는 `target/review`에서 모두 통과했다. 지시대로 MIDI 시험(`mouse_drag_preview`)과 전체 시험은 돌리지 않았다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 못 깨뜨림 | 연장은 `map_notes`·`insert_notes`의 겹침 검사를 그대로 탄다(`blocked_drag_shows_blocked_ghost_and_does_not_edit`) |
| 노트는 클립 안(꼬리 노트 규칙 포함) | 못 깨뜨림 | `shift_note`가 클립 시작 앞을 거부, 끝은 `map_notes`가 마디 단위 연장 |
| 음높이 0–127 | 못 깨뜨림 | `drag_shift`가 자르고 `shift_note`가 다시 거부 |
| 절대 피치 범위(음높이 + 오프셋, 결정 2026-10-12 B) | 지금은 오프셋을 버리므로 우회됨 | `shift_note`가 `with_pitch`(오프셋 재검사)를 쓰지 않는다. 지적 1을 고칠 때 이 검사도 살아나야 한다 |
| 노트 필드 보존(가사 LYR-1, `extra` IO-1 「모르는 키 보존」) | 깨짐 | `ops.rs` `shift_note`의 `Note::new(..)` |
| 드래그 한 번 = undo 하나, 끄는 중 문서 불변 | 못 깨뜨림 | `finish_move`의 `apply_edit(.., None)` 한 번 |
| `plan_shift`의 `map_notes` 순서 의존 | 못 깨뜨림 | `transform::plan`이 `notes` 순서대로 `f`를 부른다. 같은 값의 노트가 여럿이면 결과도 같다 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 7.4 머리말 「마우스 조작은 5.7의 키 조작과 같은 Action을 호출」 | 충족 | `ops.rs` `plan_note_op` → `plan_shift`, `pianoroll_mouse.rs` `plan_move`·`finish_move`가 같은 `plan_shift`·`plan_shift_copy` |
| 7.4 표 「드래그 중 `Esc` 취소, 원위치」 | 부분 충족(미충족으로 셈) | `pianoroll_mouse.rs:33-35` 수식키가 있으면 가로채지 않음. 지적 2 |
| MS-1 고스트·놓을 때 undo 한 단위 | 충족 | `plan_move`, `finish_move`, 시험 15·16·19 |
| MS-2 상대 위치 유지, 끝을 같은 양 | 충족 | `drag_selection_keeps_relative_positions_with_one_undo`, `right_edge_drag_moves_all_ends_by_same_amount` |
| MS-3 스냅, `Cmd` 자유, 설정 스냅 단위 | 충족 | `mouse.rs` `snap_nearest`·`snap_grid`, `settings.rs` `mouse_snap` |
| MS-4 미리듣기 | 코드상 충족(시험은 MIDI라 이번에 돌리지 않음) | `update_move`의 `sounded` 비교 |
| MS-6 끝 영역 6 px / 1/3 | 충족 | `mouse.rs` `grip_at` |
| 5.7 `Alt` 조작(회귀 확인) | 미충족(회귀) | 가사·오프셋을 잃는다. 지적 1 |
| LYR-1 가사(노트에 붙은 음절) | 미충족(회귀) | 지적 1 |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 미리듣기는 기존 `sound_note` 경로(`selection.rs` `preview_note`).
- 제품 코드의 `unwrap`/`expect`: 새 코드에 없음. `unwrap_or`·`unwrap_or_default`만 있다. `plan_move`의 거부 고스트가 `shifted(..).unwrap_or_default()`로 클립 시작 앞·음높이 밖 거부를 「고스트 없음」으로 삼키지만, `drag_shift`가 그 둘을 미리 자르므로 실제로는 닿지 않는다.
- 에러가 삼켜지는 곳: `finish_move`에서 `Ok(None)`은 조용히 끝(의도, 결정 6절 7). 거부는 상태줄에 알린다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 차단 | `shift_note`가 `Note::new(Tick::new(rel), length, pitch, note.velocity())`로 노트를 새로 만들어 가사(LYR-1)·피치 오프셋·`extra`(IO-1 모르는 키 보존)를 버린다. 키 `Alt` 조작(`Alt+h/l/j/k`, `+`/`-`, `>`/`<` …) 전부와 마우스 이동·조절·복제가 모두 이 길이라 main 대비 회귀이자 데이터 손실이다. 오프셋을 버리면서 `with_pitch`가 하던 절대 피치 범위 검사도 사라졌다. `with_start`·`with_length`·`with_pitch`로 원래 노트에서 만들어야 한다(오프셋이 있으면 음높이 변경이 `AbsolutePitchOutOfRange`로 거부될 수 있으니 `OpError` 변환도 확인). 재현: `review_key_alt_ops_keep_lyric_and_pitch_offset`, `review_body_drag_keeps_lyric`(둘 다 실패) | `crates/keyscore-app/src/pianoroll/ops.rs` `shift_note`(`let new = Note::new(..)` 줄) |
| 2 | 수정 | 끄는 중 수식키를 쥔 채 `Esc`를 누르면 취소되지 않는다. `Esc` 가로채기가 `event.keystroke.modifiers.modified()`면 넘기기 때문이다. 이 Task의 드래그 셋(`Alt` 복제, `Cmd` 자유, `Shift` 한 축)은 수식키를 쥐고 하는 조작이라 「드래그 중 `Esc` 취소, 원위치」(7.4 표, 결정 6절 11, 단계 게이트)가 그 경우에 지켜지지 않는다. `alt-escape`에서는 놓으면 복제가, `shift-escape`에서는 이동이 적용되고 undo가 쌓인다. 진행 중 조작이 있을 때는 수식키와 무관하게 `escape`를 받아야 한다. 재현: `review_escape_with_alt_held_cancels_copy_drag`, `review_escape_with_shift_held_cancels_axis_drag`(둘 다 실패) | `crates/keyscore-app/src/pianoroll_mouse.rs:31-35` |
| 3 | 제안 | 끝 조절에서 스냅한 끝이 시작 이하이면 「시작 뒤 첫 격자선」으로 가는 규칙 때문에, 격자 밖 짧은 노트(`[10, 110)`, 8분 격자)의 오른쪽 끝을 왼쪽으로 조금만 끌어도 노트가 480까지 길어진다(왼쪽 끝도 대칭). 결정 6절 3의 글자 그대로라 결함은 아니다. 「그런 격자선이 원래 끝을 넘으면 원래 끝을 유지」 같은 규칙이 끄는 방향과 맞다. 사람이 판단할 해석이다. 기록: `review_end_grip_left_drag_on_short_offgrid_note_lengthens`(통과) | `crates/keyscore-app/src/pianoroll/mouse.rs` `drag_shift`의 `Grip::End`·`Grip::Start` |
| 4 | 제안 | `watch_modifiers`를 `swallow_root_tab`의 문서 주석 한가운데에 넣어, `swallow_root_tab` 설명 다섯 줄이 `watch_modifiers`의 문서가 됐고 `swallow_root_tab`은 문서가 없다 | `crates/keyscore-app/src/window.rs` `watch_modifiers` 위 주석 |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

지적 1(차단)은 키 조작까지 퍼진 데이터 손실 회귀라 고쳐야 한다. 지적 2는 단계 게이트(`Esc` 취소)의 빈틈이다. 둘을 고치고 리뷰 시험 넷이 통과하면 재리뷰한다. 리뷰 초점(마우스가 키와 같은 계산을 부르는가)은 충족한다: 마우스 전용 편집 경로는 없고, 바로 그 공유 때문에 지적 1이 키 조작으로도 번졌다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`settings_tabs::sync`의 `selected_rows` 분리는 clippy 줄 수 상한 때문이라 보고서에 적혀 있다)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다(지적 4는 문서 주석이 다른 함수에 붙는 사실 오류라 제안으로 둠)
