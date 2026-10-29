# 적대적 리뷰: S3-T3e 퀀타이즈 설정 시트 (리뷰 1회차)

- 리뷰어 세션: 구현자와 다른 세션(구현 컨텍스트를 물려받지 않은 리뷰 전용 에이전트)
- 대상 커밋: c03081f (`s3/t3e-quantize-sheet`)

## 1. 코드만 읽고 적은 「내가 검사할 것」

시험 파일 `crates/keyscore-app/tests/quantize_sheet.rs`를 열기 전에 `git diff main...s3/t3e-quantize-sheet`의 제품 코드만 읽고 적었다.

1. 처리기가 창 루트에 있다. 시트 포커스의 디스패치 경로는 `QuantizeSheet → Dialog → 루트`라서 루트의 모든 처리기(모드·피아노롤·트랜스포트·undo)가 전역 바인딩·메뉴로 닿는다. 그 가운데 포커스를 옮기는 것(`transport::SetTempo` 인자 없음 = 템포 필드 포커스, 메뉴 경로)이 시트를 닫지 않고 포커스만 가져가면, 시트가 보이는데 키는 다른 곳으로 가는 상태가 생기는가.
2. 그 상태에서 필드를 `Enter`로 끝내면 포커스는 문서로 가는데 시트는 여전히 열려 있는가. 그때 `q`·`x` 같은 문서 키가 시트 뒤에서 문서를 바꾸는가(계획 1절 「시트가 포커스를 가진 동안 문서 키는 동작하지 않는다」의 전제가 깨진다).
3. 전역 `ctrl-tab`(`mode::Cycle`)이 시트가 열린 채 모드를 바꾸는가, 그때 포커스가 시트에 남는가(`apply_mode_event`는 입력란 포커스일 때만 포커스를 옮긴다).
4. 「팝업은 하나」: 시트를 열면 `palette`·`overlay`·`popup`을 비운다. 상태줄 모드 메뉴는 `popup`(PopupKind::Mode)이라 포함된다. 팔레트·오버레이·팝업을 열면 시트를 비운다. 팔레트를 연 채 메뉴로 시트를 열 때 팔레트가 정말 닫히는가.
5. 확인 버튼 클릭: gpui-kit `Button`은 포커스 핸들을 가진다(마우스 다운 때는 포커스하지 않는다고 주석). 버튼으로 닫은 뒤 포커스가 문서로 돌아오는가(`close_quantize_sheet`의 `contains_focused || focused().is_none()` 조건).
6. 값 범위 끝: 그리드 1/4에서 `←`, 1/64에서 `→`가 멈추는가. 강도 95에서 `Shift+→`가 100에서 멈추는가, 스윙 70에서 `Shift+→`가 75에서 멈추는가, 5에서 `Shift+←`가 0인가. 셋잇단은 켬에서 `→`가 그대로 켬인가.
7. 확인한 값이 `settings.json`에 들어가고, 같은 값이면 쓰지 않는가. 취소는 설정 파일을 건드리지 않는가.
8. 창 둘에 시트 둘: 한쪽 확인이 다른 쪽 초안에 영향이 없는가(계획대로 마지막 확인이 남는가).
9. 열린 채 `edit::QuantizeSettings`를 다시 보내면 초안이 그대로인가. 다른 창에서 설정이 바뀐 뒤 다시 보내도 낡은 초안이 남는가(계획이 허용하는 동작인지 확인).
10. vim 프리셋: `shift-q`가 `Normal` 위 vim 레이어에서 가려지지 않는가. 사용자 키맵으로 `QuantizeSheet`의 새 Action을 다른 키에 걸면 동작하는가(UI-1).
11. `Esc`는 `Dialog`의 `dialog::Cancel`이 먹는가. vim 프리셋의 `Normal > Operator`의 `escape`(operator::Cancel)처럼 다른 컨텍스트의 `escape`가 끼어들지 않는가(시트는 모드 키 컨텍스트 밖이라 끼어들지 않아야 한다).
12. 배경 클릭은 취소이고 아래로 클릭이 닿지 않는가. 오른쪽 클릭(배경이 왼쪽 버튼만 처리)이 아래 피아노롤에 닿는가.
13. 시트가 열린 동안 전역 `cmd-z`가 문서를 되돌리는가(계획 3절은 전역 바인딩이 동작한다고 적었다. 동작이 계획과 같은지 확인).

## 2. 실제 테스트와의 차이

| 내가 검사하려던 것 | 실제 테스트 존재 여부 | 비고 |
| --- | --- | --- |
| 1·2. 루트 처리기가 포커스를 가져가 시트가 고아가 되는 경로 | 없음 | 리뷰 시험 `set_tempo_dispatch_while_sheet_open_leaves_orphan_sheet`. 지금 사용자 경로는 없다(제안 1) |
| 3. `ctrl-tab`이 열린 시트에서 모드를 바꾸고 포커스가 남는가 | 없음 | 리뷰 시험 `ctrl_tab_while_sheet_open_keeps_focus_in_sheet` |
| 4. 팝업은 하나(팔레트·오버레이·모드 메뉴) | 팔레트·오버레이만(`palette_and_overlay_close_sheet_and_sheet_closes_them`) | 모드 메뉴(`popup`)는 리뷰 시험 `mode_menu_and_sheet_are_exclusive` |
| 5. 버튼·배경 클릭 뒤 포커스 복귀 | 없음(`buttons_and_backdrop_click`은 닫힘·설정만 본다) | 리뷰 시험 `mouse_close_returns_focus_to_document`(제안 2) |
| 6. 중간 값에서 큰 걸음의 끝 멈춤 | 일부(끝까지 1·10씩 밟아 가는 경로만) | 리뷰 시험 `large_steps_clamp_from_middle_values`(95→100, 70→75, 5→0, 3→0) |
| 7. 설정 파일 쓰기, 취소·같은 값은 쓰지 않음 | 쓰기만(`enter_writes_settings_file`) | 리뷰 시험 `cancel_and_unchanged_confirm_do_not_write_file` |
| 8. 창 둘 | 없음 | 리뷰 시험 `two_windows_last_confirm_wins` |
| 9. 열린 채 재요청 시 초안 유지 | 있음(`reopen_while_open_keeps_draft`) | 다른 창 확인 뒤 낡은 초안이 남는 것은 계획 1절 「마지막에 확인한 값이 남는다」와 맞다 |
| 10. vim 프리셋·사용자 키맵 | vim `shift-q`만(`vim_preset_shift_q_opens_sheet`) | vim에서 시트 안 키, 사용자 `keymap.json`의 `QuantizeSheet` 재배정은 리뷰 시험 `vim_preset_sheet_keys`, `user_keymap_rebinds_sheet_actions` |
| 11. `Esc`가 `dialog::Cancel` | 있음(`escape_discards_changes`, 키맵 단위 시험) | vim 프리셋은 리뷰 시험 9 |
| 12. 배경 클릭이 아래에 닿지 않음 | 있음(`track-add` 클릭) | 오른쪽 클릭은 헤드리스 좌표 지정이 번거로워 실행하지 않았다(아래 3절에 적지 않음) |
| 13. 열린 채 `cmd-z` | 없음 | 리뷰 시험 `cmd_z_while_sheet_open_undoes_document`. 계획 3절대로 문서가 되돌려진다 |

## 3. 깨뜨리기 시도

| # | 시도 | 결과(깨짐/못 깨뜨림) | 추가한 테스트 |
| --- | --- | --- | --- |
| 1 | 시트에서 그리드를 바꾼 뒤 `transport::SetTempo(None)` 디스패치 | 깨짐(디스패치 한정). 시트는 열린 채 포커스가 템포 필드로 간다. `Esc`는 필드만 취소하고 포커스를 문서로 보낸다. 시트가 보이는 채 `q`가 문서를 퀀타이즈했다(`[(240,..),(960,..)]`). 다만 사용자가 이 디스패치를 낼 경로가 지금 없다: `SetTempo`는 메뉴 항목이 아니고(`commands/table.rs` `Surface::Transport`), 키 `t`는 `Normal` 컨텍스트, 팔레트·명령줄은 열 때 시트를 닫는다 | `crates/keyscore-app/tests/review_s3_t3e.rs::set_tempo_dispatch_while_sheet_open_leaves_orphan_sheet`(동작 기록, 단언 없음) |
| 2 | 시트에서 `ctrl-tab` | 못 깨뜨림. 모드는 Normal→Step으로 바뀌고 시트·포커스·초안은 남는다. `enter`로 저장, 포커스 문서 | `ctrl_tab_while_sheet_open_keeps_focus_in_sheet` |
| 3 | 시트에서 `mode::ShowMenu`(메뉴 막대), 모드 메뉴가 열린 채 `edit::QuantizeSettings` | 못 깨뜨림. 서로 닫고 초안은 버린다 | `mode_menu_and_sheet_are_exclusive` |
| 4 | 「확인」「취소」 버튼, 배경(`track-add` 자리) 클릭 뒤 포커스 | 못 깨뜨림. 세 경우 모두 포커스가 문서로 돌아온다 | `mouse_close_returns_focus_to_document` |
| 5 | 1/4에서 `←`·`Shift+←`, 셋잇단 켬에서 `Shift+→`·`→`, 강도 95·스윙 70에서 `Shift+→`, 1/64에서 `→`·`Shift+→`, 강도 5·스윙 3에서 `Shift+←` | 못 깨뜨림. 1/4, 켬, 100, 75, 1/64, 0, 0 | `large_steps_clamp_from_middle_values` |
| 6 | 설정 파일을 지운 뒤 바꾸고 `Esc`, 바꿨다 되돌리고 `Enter`, 디바운스 시간(5 s) 흘림 | 못 깨뜨림. 파일을 다시 쓰지 않는다 | `cancel_and_unchanged_confirm_do_not_write_file` |
| 7 | 창 둘에 시트 둘. A 확인(1/32) 뒤 B 초안(1/8) 유지, B 확인 → 1/8 | 못 깨뜨림 | `two_windows_last_confirm_wins` |
| 8 | vim 프리셋에서 시트 안 `→`·`tab`·`↓`·`↑`·`escape`·`enter` | 못 깨뜨림. vim `Normal > Operator`의 `escape`가 끼어들지 않는다 | `vim_preset_sheet_keys` |
| 9 | 사용자 `keymap.json`에 `QuantizeSheet`의 `j`/`k`/`l`/`h`/`shift-l` 재배정 | 못 깨뜨림. 문서 `hjkl`보다 시트 바인딩이 먼저 먹고 값·줄이 맞다(강도 50→49→59) | `user_keymap_rebinds_sheet_actions` |
| 10 | 시트에서 `cmd-z` | 계획대로. 시트·포커스는 남고 문서는 되돌려진다(61→60) | `cmd_z_while_sheet_open_undoes_document` |
| 11 | 시트에서 그리드 1/8로 바꾼 채 `edit::Quantize`(메뉴 편집 > 퀀타이즈) | 계획대로. 초안이 아니라 저장된 1/16으로 퀀타이즈하고 시트·초안은 남는다 | `menu_quantize_while_sheet_open_uses_saved_settings` |

실행: `CARGO_TARGET_DIR=target/review cargo test -p keyscore-app --test review_s3_t3e`(11 통과), `--test quantize_sheet`(14 통과). 리뷰 시험 파일은 `rustfmt --check`, `cargo clippy -p keyscore-app --test review_s3_t3e -- -D warnings` 통과.

## 4. 불변식 점검

| 불변식 | 공개 API로 깨뜨릴 수 있는가 | 경로 |
| --- | --- | --- |
| 앱 설정 `quantize`와 초안이 범위 안 | 아니오 | `stepped`가 그리드는 `GRIDS` 색인을 `min`/`saturating_sub`, 강도·스윙은 `clamp(0, max)`(`quantize_sheet.rs:76-99`). 설정 파일 읽기도 범위 밖을 버린다(`settings.rs` `QuantizeSettings::read`). 시도 5 |
| 시트가 문서·선택·커서·undo·마지막 편집을 바꾸지 않는다 | 시트 자체로는 아니오 | 시트 처리기는 초안·줄·포커스만 만진다. 전역 `cmd-z`·메뉴 Action은 시트가 열린 채 문서를 바꾸지만 계획 3절이 허용한 것이다(시도 10·11) |
| 팝업은 하나(시트·팔레트·오버레이·팝업 메뉴) | 아니오 | 시트 열기가 셋을 비우고(`quantize_sheet.rs:155-157`), 팔레트·팝업·오버레이 열기가 시트를 비운다(`window.rs:675`, `window.rs:773`, `overlay.rs:235`). 시도 3 |
| 시트가 열려 있으면 포커스는 시트에 있다(계획 1절의 전제) | 디스패치로는 예, 사용자 경로로는 못 찾음 | 시도 1. 루트 처리기 `transport_set_tempo`(`window.rs:858`)가 시트를 닫지 않고 필드에 포커스 |

## 5. REQ 대조

| 요구사항 ID | 충족 / 부분 충족 / 미충족 | 근거(파일:줄) |
| --- | --- | --- |
| 5.8 `Shift+q` 퀀타이즈 설정 | 충족 | `assets/keymaps/default.json:25`, `window.rs`의 `open_quantize_sheet` 처리기, 시험 `shift_q_opens_sheet_with_current_settings` |
| 5.8.1 vim `Shift+q`(퀀타이즈 설정) | 충족 | vim 프리셋이 `shift-q`를 가리지 않는다. `vim_preset_shift_q_opens_sheet`, 리뷰 `vim_preset_sheet_keys` |
| EDT-2 그리드 1/4–1/64·셋잇단·강도 0–100·스윙 0–75 | 충족 | `quantize_sheet.rs:76-99`, 시도 5 |
| 7.6 「퀀타이즈 실행·설정」(메뉴 편집 > 퀀타이즈, 설정 시트) | 충족 | `commands/table.rs:566-573`(편집 메뉴의 퀀타이즈·퀀타이즈 설정) |
| 5.11 대화상자의 `Tab`/`Shift+Tab` | 충족 | `Dialog` 바인딩 재사용(`default.json:528-536`), `tab_and_arrows_move_rows_and_wrap` |
| P2·A13 `Esc` = 통상의 취소 | 충족 | `dialog::Cancel` → `close_quantize_sheet(false)`, `escape_discards_changes` |
| 7절 「모달 다이얼로그는 OS 네이티브 외에 두지 않는다」 | 충족(계획의 읽기) | 창 안 패널. 다만 배경이 클릭을 막고 키를 가두는 점은 사실상 모달이라 읽기가 갈릴 수 있다(제안 3) |
| UI-1 사용자 JSON 키맵 재정의 | 충족 | 리뷰 `user_keymap_rebinds_sheet_actions` |
| UI-11 보이는 UI | 충족 | 메뉴 항목, 시트의 ‹ › 「확인」「취소」 버튼(`quantize_sheet.rs:243-263`, 335-357) |

## 6. 실시간·에러 경로

- 스케줄러 경로의 할당·락·로그: 없음(UI 스레드만 바뀜).
- 제품 코드의 `unwrap`/`expect`: `quantize_sheet.rs:83` `expect("0..=max 안")` — 바로 앞 `clamp(0, max)`라 `u8` 변환이 실패할 수 없다. `quantize_sheet.rs:88` `unwrap_or(2)` — `Grid`의 다섯 변형이 모두 `GRIDS`에 있어 닿지 않는 대체값.
- 에러가 삼켜지는 곳: 새로 생긴 곳 없음. 저장 실패는 기존 `AppSettings::flush`의 `eprintln`을 따른다.

## 7. 지적 사항

| # | 심각도(차단/수정/제안) | 내용 | 파일:줄 |
| --- | --- | --- | --- |
| 1 | 제안 | 시트가 포커스를 가진 동안에도 루트 처리기 전체가 디스패치 경로에 있다. `transport::SetTempo(None)`처럼 포커스를 옮기는 처리기가 오면 시트는 열린 채 포커스를 잃고, 그 뒤 문서 키가 시트 뒤에서 문서를 바꾼다(시도 1). 지금 사용자 경로는 없지만 이후 Task가 이런 Action을 메뉴에 올리면 바로 드러난다. 시트의 포커스 핸들에 `on_blur`(또는 창의 포커스 변경 관찰)로 「시트 밖으로 포커스가 나가면 취소로 닫기」를 걸면 경로마다 닫기를 더하지 않아도 된다 | `crates/keyscore-app/src/quantize_sheet.rs:149-166`, `crates/keyscore-app/src/window.rs:858`, `crates/keyscore-app/src/transport_bar.rs:111-117` |
| 2 | 제안 | 계획 4절 「닫히면 포커스는 문서로」를 마우스 경로(「확인」「취소」 버튼, 배경)에서 보는 시험이 구현자 시험에 없다. 동작은 맞고(시도 4) 리뷰 시험이 고정하므로 제안으로 둔다 | `crates/keyscore-app/tests/quantize_sheet.rs` `buttons_and_backdrop_click` |
| 3 | 제안 | 배경이 클릭을 막고 시트가 키를 가두는 형태는 REQ 7절 「모달 다이얼로그」와 읽기가 갈릴 수 있다. 7.6이 「설정 시트」를 명시하므로 미충족은 아니라고 본다. 계획 「리스크」에만 있는 이 읽기를 `docs/decisions/`에 한 줄 남겨 두면 이후 대화상자 Task가 같은 판단을 반복하지 않는다 | `docs/reports/s3-t3e/plan.md:103` |

## 8. 판정

- [x] 병합 가능
- [ ] 수정 후 재리뷰
- [ ] 범위 재정의 (이유: )

차단·수정 지적이 없다. 제안 1–3은 구현자가 받거나 거절 이유를 보고서에 적는다.

## 체크리스트 (판정 전에 전부 확인)

- [x] 테스트를 읽기 전에 1번을 썼다
- [x] 깨뜨리는 테스트를 실제로 실행했다
- [x] REQ ID를 하나씩 대조했다
- [x] `git diff`에 Task 범위 밖의 변경이 없다(`settings.rs`의 `GRIDS` 공개 범위, `window.rs`·`overlay.rs`의 팝업 상호 배제는 계획 2절의 범위)
- [x] 기존 테스트가 지워지거나 `#[ignore]`되지 않았다
- [x] 새 의존성이 있으면 라이선스를 확인했다(새 의존성 없음)
- [x] 스타일 지적을 적지 않았다
