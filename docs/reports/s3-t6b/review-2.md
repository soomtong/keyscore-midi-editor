# 적대적 리뷰: S3-T6b 드래그 이동·조절 (리뷰 2회차)

- 리뷰어 세션: 구현자와 다른 세션(적대적 리뷰 전용 서브 에이전트, 구현 컨텍스트를 물려받지 않음. 1회차 리뷰어와도 다른 세션)
- 대상 커밋: `73f1608`(브랜치 `s3/t6b-mouse-drag`). 1회차 이후 수정 커밋 `623e9fc`, `62d9d9b`, `2a0cee3`, `56b0af1`
- 리뷰 시험: `crates/keyscore-app/tests/review_s3_t6b_r2.rs`(`CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t6b_r2`). 9개 중 2개가 결함을 보이며 실패한 그대로 둔다

## 0. 1회차 지적의 수정 확인

| 1회차 지적 | 확인 | 근거 |
| --- | --- | --- |
| 1 차단: `shift_note`가 가사·오프셋·`extra`를 버림 | 고쳐짐 | `ops.rs` `shift_note`가 `with_length` → `with_start` → `with_pitch`로 원래 노트를 고친다. `review_s3_t6b` 8개 통과, 본체 이동·`Alt` 복제·끝 조절 모두 가사·오프셋 유지(`review2_move_copy_and_resize_keep_lyric_and_offset` 통과). 순서도 맞다: `with_length`의 넘침 검사는 옛 시작 + 새 길이, `with_start`는 새 시작 + 새 길이로 하므로 최종 노트의 `start + length` 넘침을 정확히 잡는다. `with_pitch`가 절대 피치 범위를 다시 검사한다. 다만 그 검사가 되살아나면서 마우스 쪽 자르기와 어긋나는 새 결함이 생겼다(지적 1) |
| 2 수정: 수식키를 쥔 `Esc`가 드래그를 취소하지 않음 | 고쳐짐 | `pianoroll_mouse.rs:31-46`. `alt-escape`·`shift-escape`(1회차 시험), `cmd-escape`로 `Cmd+러버밴드` 취소·선택 복원(`review2_cmd_escape_during_cmd_band_restores_selection`) 통과. 조작이 없을 때는 가로채지 않는다(`review2_escape_after_drag_reaches_normal_binding` 통과). 기본·vim 키맵에 수식키 붙은 `escape` 바인딩은 없어 막히는 바인딩도 없다 |
| 3 제안: 대체 격자선이 노트를 늘림 | 고쳐짐(대체 경로만) | `mouse.rs` `drag_shift`의 `.min(end)`·`.max(start)`. 1회차 시험 통과. 가장 가까운 격자선 스냅 자체는 여전히 안쪽 끌기를 바깥으로 보낼 수 있다(지적 3, 제안) |
| 4 제안: 문서 주석 위치 | 고쳐짐 | `window.rs` `swallow_root_tab`·`watch_modifiers` 주석이 각자 자리에 있다 |

구현자가 1회차 시험 둘을 고친 것(한 마디 앞당김의 거부 단언, 제안 3 기대값)은 보고서에 적힌 대로이고 타당하다. 틱 960의 노트를 한 마디(3840) 앞당기면 클립 시작 앞이라 main에서도 거부된다.

## 1. 코드만 읽고 적은 「내가 검사할 것」

시험 파일을 열기 전에 수정 커밋 diff와 `ops.rs`·`mouse.rs`·`pianoroll_mouse.rs`만 읽고 적었다.

1. `with_pitch`가 오프셋을 더한 절대 피치를 검사하게 되었는데, 마우스 `drag_shift`의 음높이 자르기(`semitones.clamp(-low, 127 - high)`)는 오프셋 없는 음높이만 본다. 오프셋 있는 노트를 가장자리 너머로 끌면 「가장자리에서 멈춤」(결정 6절 5)이 아니라 전체 거부가 되고, 거부 고스트를 그리는 `shifted(..).unwrap_or_default()`도 같은 오류로 비어 고스트가 사라질 것이다.
2. `with_length` → `with_start` → `with_pitch` 순서에서 중간 단계가 정당한 결과를 거부하는 경우(넘침 검사 순서).
3. 수식키와 무관한 `Esc` 가로채기가 조작이 없을 때 키를 흘려보내는가, 러버밴드(`Cmd+드래그`) 중 `cmd-escape`도 선택을 되돌리는가.
4. 키맵에 「Dragging」 컨텍스트의 `escape` → `edit::CancelDrag`가 있는데 드래그 취소는 하드코딩된 가로채기다. `edit::CancelDrag`에 처리기가 있는가(KM-1 바인딩 단위, KM-3 재바인딩).
5. 끄는 중 키 편집(`Alt+k`)이 문서를 바꾸면 드래그가 취소되고 뗌이 낡은 계획을 적용하지 않는가.
6. vim 프리셋의 시퀀스 대기(`g`) 중 `Esc`: 키맵 가로채기(시퀀스만 취소)와 마우스 가로채기의 순서 때문에 드래그를 취소할 길이 막히지 않는가.
7. 새 대체 격자선 규칙 뒤에도 가장 가까운 격자선 스냅이 안쪽으로 끈 끝을 바깥 격자선으로 보내 노트를 늘이는가(제안 3의 명시 목적 「안쪽으로 끌었는데 길어지지 않게」와의 관계).
8. 리뷰 초점: 키와 마우스가 같은 계산(`plan_shift`·`plan_shift_copy`·`shifted` → `shift_note`)을 부르는가가 수정 뒤에도 유지되는가.

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1 오프셋 노트의 가장자리 자르기 | 없음 | 리뷰 시험으로 깨짐 확인 |
| 2 `with_*` 순서 | 없음 | 코드로 확인(0절). 결과 노트 기준 검사와 같다 |
| 3 수식키 `Esc`의 통과·러버밴드 | 부분 | 1회차 시험은 드래그만. 리뷰 시험으로 확인, 못 깨뜨림 |
| 4 `edit::CancelDrag` | 없음 | 리뷰 시험으로 깨짐 확인 |
| 5 끄는 중 키 편집 | 없음(드래그 쪽) | `selection_document_changed`가 취소. 리뷰 시험으로 확인, 못 깨뜨림 |
| 6 vim 시퀀스 대기 중 `Esc` | 없음 | 리뷰 시험으로 확인, 못 깨뜨림(첫 `Esc`가 드래그를 취소한다) |
| 7 가장 가까운 스냅의 바깥 이동 | 없음 | 리뷰 시험으로 기록(통과) |
| 8 같은 계산 | 있음 | `note_ops_equal_plan_shift_of_their_shift`. 수정 커밋은 `shift_note` 한 곳만 고쳐 공유가 유지된다 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 음높이 115 + 오프셋 +12반음(절대 127) 노트에 `drag_shift`(한 박 오른쪽, 3반음 위) 결과를 `plan_shift`로 계획 | 깨짐: `drag_shift`가 음높이 Δ를 3으로 두고(115 + 3 ≤ 127) `plan_shift`가 `AbsolutePitchOutOfRange`로 거부. 시간 Δ까지 버려진다 | `review2_drag_shift_with_pitch_offset_is_accepted_by_plan_shift` |
| 2 | 같은 노트를 화면에서 끎 | 깨짐: 고스트가 비고(`[]`, 거부 색 고스트도 없음) 놓으면 편집 없이 「노트 옮기기: 음높이 + 피치 오프셋이 MIDI 2.0 범위 … 밖이다」 | `review2_body_drag_offset_note_stops_at_top_edge` |
| 3 | 가사·오프셋 노트를 본체 이동 → `Alt` 복제 → 사본 끝 조절 | 못 깨뜨림: 셋 모두 가사·오프셋 유지 | `review2_move_copy_and_resize_keep_lyric_and_offset` |
| 4 | `Cmd+러버밴드` 중 `cmd-escape` | 못 깨뜨림: 누르기 전 선택 `[N1]`로 복원, 뗌도 무효 | `review2_cmd_escape_during_cmd_band_restores_selection` |
| 5 | 드래그를 마친 뒤 `escape` | 못 깨뜨림: 가로채지 않고 선택 해제(`select::Clear`)에 닿는다 | `review2_escape_after_drag_reaches_normal_binding` |
| 6 | 끄는 중 `edit::CancelDrag` 디스패치(키맵 「Dragging」의 `escape` Action) | 깨짐: 처리기가 없어 조작이 살아 있고, 놓으면 이동이 적용된다 | `review2_cancel_drag_action_cancels_drag` |
| 7 | 끄는 중 `alt-k` | 못 깨뜨림: 조옮김 하나만 적용(undo +1), 드래그 취소, 뗌 무효 | `review2_key_edit_during_drag_cancels_drag` |
| 8 | vim 프리셋, 끄는 중 `g` 뒤 `escape` | 못 깨뜨림: 첫 `escape`가 드래그를 취소한다(마우스 가로채기가 먼저 돈다) | `review2_vim_pending_sequence_then_escape_cancels_drag` |
| 9 | `[0, 300)`의 오른쪽 끝을 10틱 왼쪽으로, `[10, 110)`의 왼쪽 끝을 50틱 오른쪽으로(8분 격자) | 결정 글대로(가장 가까운 격자선): 끝 Δ +180, 시작 Δ −10. 결함이 아니라 기록 | `review2_inward_edge_drags_can_still_lengthen_by_nearest_snap`(통과) |

기존 `review_s3_t6b`(8), `mouse_drag`(18), `mouse_drag_pure`(15), `mouse_select`(26), `pianoroll_select`(14)는 `target/review`에서 모두 통과했다. 지시대로 MIDI 시험(`mouse_drag_preview` 등)과 전체 시험은 돌리지 않았다. 리뷰 시험 파일은 `rustfmt`·`clippy -D warnings`를 통과한다.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 같은 트랙 클립 겹침 없음 | 못 깨뜨림 | `map_notes`·`insert_notes` 겹침 검사 그대로 |
| 노트는 클립 안 | 못 깨뜨림 | `shift_note`의 클립 시작 검사, 끝은 `map_notes` 연장 |
| 절대 피치 범위(결정 2026-10-12 B) | 못 깨뜨림(수정으로 회복) | `with_pitch`가 검사한다. 오히려 마우스 자르기가 이 범위를 몰라 거부가 난다(지적 1) |
| 노트 필드 보존(LYR-1, IO-1) | 못 깨뜨림(수정으로 회복) | `shift_note`가 원래 노트를 고친다 |
| 드래그 한 번 = undo 하나, 끄는 중 문서 불변 | 못 깨뜨림 | 끄는 중 키 편집은 드래그를 취소한다(시도 7) |
| 키·마우스 같은 계산 | 유지 | `plan_note_op` = `plan_shift(op.shift(step))`, 마우스는 `plan_shift`·`plan_shift_copy`, 노트 계산은 `shift_note` 하나 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 7.4 머리말 「같은 Action을 호출」 | 충족 | `ops.rs` `plan_note_op` → `plan_shift`, `pianoroll_mouse.rs` `plan_move`·`finish_move` |
| 7.4 표 「드래그 중 `Esc` 취소, 원위치」 | 충족(키 `Esc`) | `pianoroll_mouse.rs:31-46`. 수식키 무관 |
| KM-1·KM-3 (바인딩 = 컨텍스트·키·Action, 재바인딩) | 미충족 | `assets/keymaps/default.json:180-185` 「Dragging」 `escape` → `edit::CancelDrag`, 처리기 없음(지적 2) |
| MS-1 고스트·undo 한 단위 | 부분 충족(미충족으로 셈) | 오프셋 노트를 가장자리 너머로 끌면 고스트가 사라진다(`pianoroll_mouse.rs:436`의 `shifted(..).unwrap_or_default()`, 지적 1) |
| MS-2 상대 위치 유지, 끝 같은 양 | 충족 | 기존 시험 |
| MS-3 스냅·`Cmd` 자유·설정 단위 | 충족 | `mouse.rs` `snap_nearest`, 대체 격자선 수정 |
| MS-4 미리듣기 | 코드상 충족(MIDI 시험 미실행) | `update_move`. 지적 1 상황에서는 거부된 음높이(절대 범위 밖)를 들려준다(`grabbed_ghost_pitch`) |
| MS-6 끝 영역 | 충족 | `grip_at` |
| SEQ-3 (음높이 + 피치 오프셋) · 결정 6절 5 「모든 노트가 범위 안에 있게 자른다」 | 미충족 | `mouse.rs:295` 자르기가 오프셋을 보지 않는다(지적 1) |
| LYR-1, 5.7 `Alt` 조작 회귀 | 충족(회복) | `ops.rs` `shift_note` |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음. 미리듣기는 기존 `preview_note` 경로.
- 제품 코드의 `unwrap`/`expect`: 수정 커밋에 새로 없음.
- 에러가 삼켜지는 곳: `plan_move`의 거부 고스트 `shifted(..).unwrap_or_default()`. 1회차는 「`drag_shift`가 미리 잘라 실제로는 닿지 않는다」고 보았으나, 지적 1 수정으로 `shift_note`가 절대 피치 범위 오류를 낼 수 있게 되면서 닿는다. 그때 고스트가 통째로 사라진다(결정 6절 9 「거부되면 거부 색으로 그린다」와 어긋남).

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 수정 | 피치 오프셋이 있는 노트를 음높이 가장자리 너머로 끌면 「가장자리에서 멈춤」(결정 6절 5)이 아니라 드래그 전체가 거부된다. `drag_shift`의 음높이 자르기는 `note.pitch()`만 보고(0–127), 1회차 지적 1 수정으로 되살아난 `with_pitch`는 음높이 + 오프셋의 절대 범위(0 이상 128반음 미만)를 검사한다. 그래서 시간 Δ까지 함께 버려지고, 거부 고스트를 그리는 `shifted(..).unwrap_or_default()`도 같은 오류로 비어 고스트가 사라지며(결정 6절 9), 미리듣기는 범위 밖 음높이를 들려준다. 자르기의 상한·하한을 오프셋을 더한 절대 음높이로 계산해야 한다(예: 오프셋이 양수면 상한 `127 − ceil(pitch + offset − 127)`에 해당하는 반음 수). 재현: `review2_drag_shift_with_pitch_offset_is_accepted_by_plan_shift`, `review2_body_drag_offset_note_stops_at_top_edge`(둘 다 실패) | `crates/keyscore-app/src/pianoroll/mouse.rs:290-295`(`Grip::Body`의 `low`·`high`), `crates/keyscore-app/src/pianoroll_mouse.rs:436` |
| 2 | 수정 | 키맵(`default.json` 「Dragging」)과 Action 목록(`commands/table.rs` `edit::CancelDrag` 「마우스 드래그 취소, 원위치」)은 드래그 취소를 `edit::CancelDrag` Action으로 적는데, 이 Action에는 처리기가 없고 「Dragging」 컨텍스트도 세워지지 않는다. 실제 취소는 하드코딩된 `escape` 가로채기다. 키맵 편집(KM-3)에서 `edit::CancelDrag`를 다른 키에 묶어도 드래그가 취소되지 않고, `escape` 바인딩을 지워도 `Esc`가 계속 취소한다(KM-1 「바인딩 단위는 (컨텍스트, 키, Action)」). 드래그 동작을 처음 들인 이 Task에서 Action 처리기를 붙이고 끄는 동안 「Dragging」 컨텍스트를 세우거나, 하드코딩을 유지할 거라면 키맵·Action 목록과의 관계를 결정 문서에 적어야 한다. 재현: `review2_cancel_drag_action_cancels_drag`(실패) | `crates/keyscore-app/src/pianoroll_mouse.rs:31-46`, `assets/keymaps/default.json:180-185`, `crates/keyscore-app/src/actions.rs:347` |
| 3 | 제안 | 결정 6절 3이 대체 격자선 규칙의 목적을 「안쪽으로 끌었는데 격자 밖의 짧은 노트가 길어지지 않게」로 적었지만, 가장 가까운 격자선 스냅 자체가 안쪽으로 끈 끝을 바깥 격자선으로 보낸다(`[0, 300)`의 끝을 10틱 안쪽으로 끌면 480으로 늘어남, `[10, 110)`의 시작을 50틱 안쪽으로 끌면 0으로 늘어남). 가장 가까운 격자선은 결정 글 그대로라 결함은 아니다. 문구를 「대체 격자선은 원래 끝을 넘지 않는다」 정도로 좁히거나, 끝 조절의 스냅을 끄는 방향 쪽 격자선으로 할지 사람이 판단할 일이다. 기록: `review2_inward_edge_drags_can_still_lengthen_by_nearest_snap`(통과) | `docs/decisions/2026-10-31-s3-t6-mouse-editing.md` 6절 3, `crates/keyscore-app/src/pianoroll/mouse.rs` `drag_shift` |

## 8. 판정

- [ ] 병합 가능
- [x] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

1회차 지적 1–4는 모두 고쳐졌고 리뷰 초점(마우스 전용 편집 경로 없음)도 유지된다. 단계 게이트 셋(다중 선택 상대 위치, 드래그 한 번 = undo 하나, `Esc` 취소)은 통과한다. 다만 지적 1은 1회차 수정이 되살린 절대 피치 검사와 마우스 자르기가 어긋나 생긴 새 결함(결정 6절 5·9 위반)이고, 지적 2는 드래그 취소가 키맵 Action과 이어지지 않는 KM-1 위반이다. 지적 2를 이 Task 범위 밖으로 볼 근거가 있으면(예: 키맵 컨텍스트 체계를 다른 Task가 맡음) 결정 요청으로 돌리고 그 사실을 보고서에 적으면 된다. 지적 1을 고치고 리뷰 시험 둘이 통과하면 재리뷰한다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(수정 커밋 넷은 `ops.rs`·`pianoroll_mouse.rs`·`mouse.rs`·`window.rs`·결정·보고·1회차 리뷰 시험만 고침)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다(1회차 리뷰 시험 둘의 기대 정정은 보고서에 적혀 있고 타당)
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
