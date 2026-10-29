# Task 계획: S3-T3e 퀀타이즈 설정 시트

- 브랜치: `s3/t3e-quantize-sheet`
- 관련 REQ: 5.8 `q`/`Shift+q`, 5.8.1 vim `Q`, EDT-2(그리드 1/4–1/64, 셋잇단, 강도 0–100%, 스윙 0–75%), 7.6 「퀀타이즈 실행·설정」(메뉴 편집 > 퀀타이즈, 설정 시트), 5.11 「키맵 편집 · 설정 · 대화상자」의 `Tab`/`Shift+Tab`, P2·A13(`Esc`는 통상의 취소), 7절 「모달 다이얼로그는 OS 네이티브 외에 두지 않는다」, UI-1, UI-11
- 선행 Task: S3-T3b(설정 `quantize`와 `q`)
- 관련 결정: `docs/decisions/2026-10-26-s3-t3b-note-edit-details.md` 3-A(앱 설정 `quantize`, 모든 프로젝트에 같다)와 「후속」
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

프로젝트 창에서 `Shift+q`(vim 프리셋도 같은 키 `Q`), 메뉴 편집 > 퀀타이즈 설정, 팔레트의 「퀀타이즈 설정」이 퀀타이즈 설정 시트를 연다. 키보드만으로 네 값(그리드·셋잇단·강도·스윙)을 바꾸고 `Enter`로 저장하거나 `Esc`로 버린다. 저장한 값은 앱 설정 `quantize`(`settings.json`)에 들어가고 다음 `q`가 그 값을 쓴다. 규칙은 아래가 전부이고 시험이 고정한다.

### 1. 시트의 모양과 자리

- 시트는 프로젝트 창 안에 그리는 패널이다(별도 창이나 OS 모달이 아니다, REQ 7절). 팔레트·`?` 오버레이처럼 모드 키 컨텍스트 겹 바깥(창 루트의 자식, `deferred`)에 그린다. 그래서 시트가 포커스를 가진 동안 문서 키(`q`, `x`, `hjkl` 등)는 동작하지 않는다.
- 키 컨텍스트는 바깥 `Dialog`, 안쪽 `QuantizeSheet`(경로 `Dialog > QuantizeSheet`). 포커스는 안쪽 요소가 가진다. `Dialog`의 기존 바인딩(`enter` 확인, `escape` 취소, `tab`/`shift-tab` 다음·이전 필드)을 그대로 쓴다.
- 줄은 위에서부터 그리드, 셋잇단, 강도, 스윙. 각 줄은 「이름: 값」과 줄이는 버튼(‹)·늘리는 버튼(›)을 보인다. 포커스 줄은 강조한다. 아래에 「확인」「취소」 버튼과 키 안내 한 줄.
- 창마다 시트 하나. 다른 창의 시트와 상관없다(설정은 앱 전역이라 마지막에 확인한 값이 남는다).

### 2. 열기

- `edit::QuantizeSettings`의 처리기는 창 루트에 단다. 그래서 키(Normal 컨텍스트의 `shift-q`)·메뉴·팔레트 어느 쪽으로 와도, 어느 모드에서 와도 연다.
- 열 때 초안은 지금 앱 설정 `quantize`의 사본이고 포커스 줄은 그리드다.
- 팝업은 하나만 띄운다: 열면 팔레트·`?` 오버레이·팝업 메뉴를 닫는다. 반대로 팔레트·오버레이·팝업 메뉴를 열면 시트는 취소로 닫힌다(초안을 버린다).
- 이미 열려 있으면 초안과 포커스 줄을 그대로 두고 시트로 포커스만 돌린다.

### 3. 키(시트 안)

| 키 | Action | 동작 |
| --- | --- | --- |
| `Tab`, `↓` | `window::FocusNext` | 다음 줄. 스윙 다음은 그리드(돈다) |
| `Shift+Tab`, `↑` | `window::FocusPrev` | 이전 줄. 그리드 이전은 스윙 |
| `→` | `dialog::Increase` | 값을 한 칸 늘린다 |
| `←` | `dialog::Decrease` | 값을 한 칸 줄인다 |
| `Shift+→` | `dialog::IncreaseLarge` | 강도·스윙은 10 늘린다. 그 밖은 `→`와 같다 |
| `Shift+←` | `dialog::DecreaseLarge` | 강도·스윙은 10 줄인다. 그 밖은 `←`와 같다 |
| `Enter` | `dialog::Confirm` | 초안을 앱 설정에 넣고 닫는다 |
| `Esc` | `dialog::Cancel` | 초안을 버리고 닫는다 |

- 「한 칸」: 그리드는 1/4 → 1/8 → 1/16 → 1/32 → 1/64 순서에서 다음(늘리기)·이전(줄이기). 셋잇단은 늘리기 = 켬, 줄이기 = 끔. 강도는 0–100, 스윙은 0–75에서 1씩. 모든 값은 끝에서 멈춘다(돌지 않는다).
- 새 Action 네 개(`dialog::Increase`·`Decrease`·`IncreaseLarge`·`DecreaseLarge`)는 `QuantizeSheet` 컨텍스트에만 건다. 명령 표에는 `Surface::Dialog`의 「다른 곳」 명령으로 둔다(대화상자 안에서만 뜻이 있다, `dialog::Confirm`과 같다). 사용자 키맵으로 바꿀 수 있다(UI-1).
- 위 표에 없는 키는 시트 안에서 아무 일도 하지 않는다. 전역 바인딩(메뉴 키 등가물 `cmd-…`, 문맥 없는 바인딩)은 GPUI 규칙대로 그대로 동작한다.

### 4. 확인·취소·닫기

- 확인: `AppSettings::update`로 `quantize`를 초안으로 바꾼다(값이 같으면 쓰지 않는다, 기존 동작). 상태줄에 「퀀타이즈 설정: 1/16, 셋잇단 끔, 강도 100%, 스윙 0%」를 남긴다. 저장은 기존 설정 저장 규칙(디바운스, 종료 때 flush)을 따른다.
- 취소: 앱 설정을 건드리지 않는다. 메시지는 남기지 않는다.
- 마우스: 「확인」「취소」 버튼은 키와 같다. 줄을 누르면 그 줄로 포커스 줄이 옮는다. ‹ › 버튼은 그 줄의 줄이기·늘리기다. 시트 바깥(배경)을 누르면 취소다(오버레이와 같다). 배경은 아래 요소에 클릭이 닿지 않게 막는다.
- 닫히면 포커스는 문서로 돌아간다. 확인·취소는 문서·선택·커서·undo 기록을 바꾸지 않는다. `.` 반복의 마지막 편집도 바꾸지 않는다.
- 창의 문서가 바뀌는 흐름(팔레트·오버레이를 닫는 그 자리, `window.rs`)에서 시트도 닫는다(취소).

## 공개 인터페이스 초안

~~~rust
// crates/keyscore-app/src/actions.rs, dialog 모듈에 더함
dialog::Increase, dialog::Decrease, dialog::IncreaseLarge, dialog::DecreaseLarge

// crates/keyscore-app/src/quantize_sheet.rs
pub enum SheetRow { Grid, Triplet, Strength, Swing }

impl ProjectWindow {
    /// 열린 시트의 (포커스 줄, 초안). 닫혀 있으면 None. 시험·접근성용.
    pub fn quantize_sheet(&self) -> Option<(SheetRow, QuantizeSettings)>;
}
~~~

## 테스트 목록

시험 파일 `crates/keyscore-app/tests/quantize_sheet.rs`(헤드리스 창, 키 입력).

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `shift_q_opens_sheet_with_current_settings` | UI | `shift-q`가 시트를 열고 초안 = 지금 설정, 포커스 줄 그리드, 포커스가 시트 안(`Dialog > QuantizeSheet`) |
| 2 | `menu_action_opens_sheet_in_step_mode` | UI | Step 모드에서 `edit::QuantizeSettings` 디스패치(메뉴·팔레트 경로)로 열린다 |
| 3 | `vim_preset_shift_q_opens_sheet` | UI | vim 프리셋에서도 `shift-q` |
| 4 | `tab_and_arrows_move_rows_and_wrap` | UI | `tab`·`down` 다음 줄, `shift-tab`·`up` 이전 줄, 양 끝에서 돈다 |
| 5 | `arrows_change_each_value_and_stop_at_ends` | UI | 그리드 순서와 양 끝, 셋잇단 켬·끔, 강도·스윙 ±1·±10과 0·100·75 끝 |
| 6 | `enter_saves_and_q_uses_new_values` | UI | 바꾸고 `enter` → 설정 값, 시트 닫힘, 포커스 문서, 상태줄 메시지, 다음 `q`가 새 그리드·셋잇단·강도로 퀀타이즈(게이트) |
| 7 | `enter_writes_settings_file` | UI | 확인 뒤 `AppSettings::flush` → `settings.json`의 `quantize` |
| 8 | `escape_discards_changes` | UI | 바꾸고 `escape` → 설정 그대로, 닫힘, 포커스 문서 |
| 9 | `document_keys_do_not_reach_document_while_open` | UI | 시트에서 `q`·`x`·`l`·`u`가 문서·커서·undo를 바꾸지 않는다 |
| 10 | `sheet_does_not_touch_undo_or_repeat` | UI | 확인·취소 뒤 undo 길이 그대로, `.`가 그 전 편집을 반복 |
| 11 | `palette_and_overlay_close_sheet_and_sheet_closes_them` | UI | 시트에서 팔레트를 열면 시트 닫힘(초안 버림), 팔레트·오버레이가 열린 채 시트를 열면 그쪽이 닫힘 |
| 12 | `reopen_while_open_keeps_draft` | UI | 열린 채 `edit::QuantizeSettings`를 다시 디스패치하면 초안·줄이 그대로 |
| 13 | `buttons_and_backdrop_click` | UI | ‹ › 버튼, 「확인」 버튼, 배경 클릭 취소 |
| 14 | `sheet_actions_are_in_command_table_and_keymap` | 단위 | 새 Action 네 개가 명령 표(`Surface::Dialog`)·Action 목록·기본 키맵 `QuantizeSheet`에 있다(기존 명령 표 시험이 함께 본다) |

## 불변식

- 앱 설정 `quantize`의 값은 늘 범위 안이다(그리드 다섯 값, 강도 0–100, 스윙 0–75). 시트의 초안도 같은 범위에서만 움직인다(끝에서 멈춤).
- 시트가 문서·선택·커서·undo·마지막 편집을 바꾸지 않는다.
- 팝업은 하나: 시트·팔레트·오버레이·팝업 메뉴 가운데 하나만 열려 있다.

## 범위 밖

- 설정 창(`Cmd+,`)에 퀀타이즈 구역을 두는 것. 7.6은 메뉴와 설정 시트만 요구한다.
- 숫자를 글자로 입력하는 강도·스윙 입력란. 키보드만으로 바꾸는 요구는 화살표로 채운다. 필요하면 후속.
- 녹음 뒤 자동 퀀타이즈(S5).
- 복제 간격 격자 결정(`2026-10-26-duplicate-offset-grid.md`)은 아직 결정 대기다.

## 리스크·결정 요청

- 시트 안의 키 배정(화살표로 값, `Shift`로 10씩, 셋잇단은 → 켬·← 끔)은 REQ에 없는 UI 세부다. REQ의 뜻(키보드로 설정 시트를 쓴다, `Tab` 다음 필드, `Esc` 취소)을 바꾸지 않는 읽기라서 결정 요청 없이 이 계획에 적고 시험으로 고정한다. 사람이 다르게 원하면 키맵으로 바꿀 수 있다.
- REQ 7절은 OS 네이티브 외의 모달 다이얼로그를 두지 않는다. 시트는 창 안 패널(팔레트와 같은 자리)이라 이 규칙과 맞는다고 읽는다.
