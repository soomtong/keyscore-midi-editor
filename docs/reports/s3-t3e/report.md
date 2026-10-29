# Task 보고: S3-T3e 퀀타이즈 설정 시트

- PR: 브랜치 `s3/t3e-quantize-sheet`
- 계획 파일: docs/reports/s3-t3e/plan.md
- 결정: `docs/decisions/2026-10-26-s3-t3b-note-edit-details.md` 3-A(앱 설정 `quantize`)와 「후속」
- 계획 대비 달라진 점
  - 계획 4절의 「창의 문서가 바뀌는 흐름에서 시트도 닫는다」를 지웠다. 그런 흐름(창 안에서 문서를 바꿔 끼우는 길)이 없다. 열기·복구는 새 창을 만든다.
  - 시험 14(키맵)는 명령 표 시험(`commands.rs`)이 새 Action 네 개를 함께 보고, `quantize_sheet.rs`는 `Dialog > QuantizeSheet` 경로에서 키마다 GPUI가 고르는 첫 바인딩을 본다(`sheet_actions_are_bound_in_quantize_sheet_context`).
  - 시험이 문서 포커스를 보려고 공개 접근자 `ProjectWindow::document_focus_for_test`를 더했다.

## 구현 요약

- `crates/keyscore-app/src/quantize_sheet.rs`(새 파일): `SheetRow`, `QuantizeSheet`(포커스·줄·초안), `ProjectWindow::open_quantize_sheet`(`edit::QuantizeSettings` 처리기), `close_quantize_sheet(save)`, 줄 이동·값 바꾸기, 그리기(`deferred` 배경 + `Dialog` 패널 + `QuantizeSheet` 포커스 요소). 값 한 칸은 `stepped`(그리드 `GRIDS` 순서, 셋잇단 → 켬 ← 끔, 강도 0–100·스윙 0–75, 끝에서 멈춤).
- `crates/keyscore-app/src/window.rs`: 필드 `quantize_sheet`, 루트 처리기, 루트 자식으로 그리기. 팔레트·팝업 메뉴를 열면 시트를 버린다.
- `crates/keyscore-app/src/overlay.rs`: `?` 오버레이를 열면 시트를 버린다.
- `crates/keyscore-app/src/actions.rs`, `commands/table.rs`: `dialog::Increase`·`Decrease`·`IncreaseLarge`·`DecreaseLarge`(명령 표 `Surface::Dialog`의 「다른 곳」).
- `assets/keymaps/default.json`: `QuantizeSheet` 컨텍스트(`↑`/`↓` 줄, `←`/`→` 값, `Shift+←`/`Shift+→` 10씩). `Enter`·`Esc`·`Tab`은 기존 `Dialog` 바인딩.
- `crates/keyscore-app/src/settings.rs`: `GRIDS`를 `pub(crate)`로(시트가 같은 순서·표기를 쓴다).

## 테스트 결과

~~~
keyscore-app --test quantize_sheet: 14 passed
keyscore-app --test commands·keymap_engine·keymap_overlay·note_ops·keymap_source·keymap_user·command_line: 실패 없음(7·12·11·4·4·4·15 passed)
전체 cargo test --workspace --no-fail-fast: 아래 「자동 게이트」
~~~

| 계획의 테스트 # | 실제 테스트 이름 | 상태 |
| --- | --- | --- |
| 1 | `shift_q_opens_sheet_with_current_settings` | 통과 |
| 2 | `menu_action_opens_sheet_in_step_mode` | 통과 |
| 3 | `vim_preset_shift_q_opens_sheet` | 통과 |
| 4 | `tab_and_arrows_move_rows_and_wrap` | 통과 |
| 5 | `arrows_change_each_value_and_stop_at_ends` | 통과 |
| 6 | `enter_saves_and_q_uses_new_values` | 통과(게이트: 바꾼 값으로 `q`) |
| 7 | `enter_writes_settings_file` | 통과 |
| 8 | `escape_discards_changes` | 통과 |
| 9 | `document_keys_do_not_reach_document_while_open` | 통과 |
| 10 | `sheet_does_not_touch_undo_or_repeat` | 통과 |
| 11 | `palette_and_overlay_close_sheet_and_sheet_closes_them` | 통과 |
| 12 | `reopen_while_open_keeps_draft` | 통과 |
| 13 | `buttons_and_backdrop_click` | 통과 |
| 14 | `sheet_actions_are_bound_in_quantize_sheet_context` + 기존 `commands.rs` | 통과 |

계획에 있었지만 쓰지 않은 테스트: 없음.

## 자동 게이트

- [x] fmt
- [x] clippy -D warnings
- [ ] test(전체, `--no-fail-fast`): 진행 중. 결과를 이 줄에 적는다
- [x] deny
- [x] `scripts/check-deps.sh`
- [ ] `scripts/check-deps.test.sh`, `scripts/bench-check.test.sh`: 진행 중

## 알려진 한계

- 강도·스윙을 숫자로 직접 입력하는 입력란은 없다(화살표와 버튼만). 계획 「범위 밖」.
- 설정 창(`Cmd+,`)에는 퀀타이즈 구역이 없다. 7.6은 메뉴와 설정 시트만 요구한다.
- 두 창에서 시트를 함께 열면 늦게 확인한 쪽의 초안 전체가 남는다(설정은 앱 전역, 계획 1절).
- 실제 창에서의 모양(색·간격)은 사람이 확인할 항목이다(헤드리스 시험은 요소·포커스·동작만 본다).

## 리뷰어에게

- 시트는 루트의 자식이라 모드 키 컨텍스트 밖이다. 문맥 없는 전역 바인딩(`ctrl-tab` 모드 순환, 표준 `cmd-…`)은 시트가 열린 동안에도 동작한다. 모드가 바뀌어도 시트는 그대로 둔다. 메뉴 `cmd-…` 명령(예: `Cmd+Z`)이 시트 뒤의 문서를 바꿀 수 있다. 시트는 문서를 보지 않으므로 상태가 어긋나지는 않는다고 봤다.
- `close_quantize_sheet`의 포커스 복귀는 「포커스가 아직 시트에 있거나 아무 데도 없을 때만」이다(팔레트 닫기와 같은 규칙).
- 버튼(‹ › 확인 취소)은 gpui-component `Button`이다. 누른 뒤 포커스를 시트로 다시 돌린다.
