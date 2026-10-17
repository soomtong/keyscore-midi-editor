# Task 계획: S2-T2 키맵 엔진

- 브랜치: `s2/t2-keymap`
- 관련 REQ: 7.3 KM-1·2·5·6·7·11·12(KM-12는 단계 파일의 「Cmd 표준 단축키의 프리셋 불변」으로 읽는다, 7.5), 5.7·5.8·5.8.1(프리셋), UI-1
- 선행 Task: S2-T1
- 관련 결정: `2026-10-17-key-sequence-timeout.md`(A: 시퀀스 대기 1 s 고정)
- 입력 자료: `docs/reports/s2-t2/action-catalog.md`(REQ에서 뽑은 Action 285개, 충돌·모호 40건)

## 끝의 정의 (내 말로)

1. Action을 한 곳에 정의한다
   - 카탈로그의 Action 285개를 `src/actions.rs`에 `actions!`(인자 없는 것)와 `#[derive(Action)]`(인자 있는 것)로 정의한다.
   - 문서 주석은 한 줄 설명이고, 팔레트(S2-T4)가 쓴다.
   - 아직 동작이 없는 Action은 처리기가 없어 아무 일도 하지 않는다. 각 단계가 처리기를 단다.
2. 키맵 파일 형식(KM-1, 7.3 예시)
   - GPUI 키맵 JSON 배열 `[{ "context": "...", "bindings": { "keys": "ns::Name" } }]`을 그대로 읽는다.
   - 값은 세 가지를 받는다.
     - `"ns::Name"`
     - `"ns::Name(arg)"`: 7.3 예시 표기. 인자 하나를 JSON 문자열로 넘긴다.
     - `["ns::Name", {인자}]`: GPUI 표기.
   - 값 `null`은 아래 레이어의 바인딩을 지운다.
   - 주석(`//`)과 끝 쉼표를 허용한다.
   - 오류(JSON 문법, 모르는 Action, 잘못된 키 표기, 잘못된 인자, 잘못된 컨텍스트)는 파일의 줄·열과 함께 모두 보고한다. 오류 항목만 빼고 나머지는 적용한다.
3. 레이어(KM-2, 7.5)
   - 순서: 표준(`Cmd` 단축키, 프리셋 불변) → 프리셋(`default` 또는 `vim`) → 사용자 오버라이드(`keymap.json`).
   - 뒤 레이어가 이긴다. 다만 표준 레이어의 `cmd-` 키는 프리셋이 덮지 못하게 프리셋 파일에서 거부한다. 사용자 레이어는 덮을 수 있다.
   - 프리셋을 바꿔도 사용자 레이어는 그대로다. 프리셋 선택은 설정 `keymap_preset`에 저장한다.
4. 충돌 감지(KM-4의 엔진 부분, 편집 UI는 S2-T6)
   - 같은 컨텍스트·같은 키 시퀀스에 다른 Action이 둘이면 충돌이다(같은 레이어 안. 레이어 사이 덮어쓰기는 충돌이 아니다).
   - 같은 컨텍스트에서 한 시퀀스가 다른 시퀀스의 접두이면 접두 충돌이다.
   - 함수 `conflicts(&LayerSet) -> Vec<Conflict>`와 위치 정보를 준다.
5. 컨텍스트 우선순위
   - GPUI 규칙을 그대로 쓰고 문서화·테스트한다. 키 컨텍스트 트리에서 더 깊은 노드에 맞는 바인딩이 이긴다. 그래서 `Normal > Arrangement`가 `Normal`보다 우선한다. 깊이가 같으면 뒤 레이어·뒤 항목이 이긴다.
6. 멀티키 시퀀스와 카운트 접두(KM-5)
   - 시퀀스(`g g`)는 GPUI에 맡긴다. 대기는 1 s 고정이다(결정 A).
   - 카운트 접두
     - vim 프리셋에서 `1`–`9`(그리고 카운트가 시작된 뒤의 `0`)는 `count::Digit(n)`이다.
     - 창마다 카운트를 쌓고, 다음 Action 처리기가 `take_count()`로 꺼낸다.
     - 엔진은 숫자 Action과 저장소만 준다. 각 Action의 반복 적용은 그 Action의 단계가 한다.
7. 물리 키 매칭(KM-7)
   - GPUI macOS는 바인딩을 `Keystroke.key`로 맞춘다. 이 값은 현재 자판 레이아웃(한글 입력 중에도 ABC/U.S.)과 keyCode로 만든다. 입력된 글자는 `key_char`에 따로 담긴다.
   - 그래서 한/영 전환과 무관하게 같은 자리 키가 같은 Action이다. 테스트로 `key = "h", key_char = "ㅗ"` 입력이 `h` 바인딩을 부르는지 확인한다.
   - 문자 기준이 필요한 곳(음이름 텍스트 입력, 팔레트 입력, 가사 모드)은 텍스트 입력 요소(IME)가 받는다. 엔진은 그 컨텍스트에 바인딩을 두지 않는 것으로 예외를 둔다. 실제 가사 모드는 S6이다.
8. 수식키 교환(KM-11, S): 프리셋을 바인딩할 때 `alt`↔`cmd`를 바꾸는 옵션 `swap_alt_cmd`. 표준 레이어(`Cmd` 단축키)는 바꾸지 않는다.
9. 적용과 핫리로드(KM-6)
   - 부트 때 레이어를 읽어 `cx.bind_keys`로 건다. 그 뒤에 메뉴를 다시 건다(메뉴는 단축키를 굽는다).
   - `~/Library/Application Support/KeyScore/keymap.json`을 1초 간격으로 살핀다(수정 시각·크기). 바뀌면 다시 읽어 바인딩을 갈아 끼운다.
   - 파싱 오류는 전역 `KeymapStatus`에 줄 번호와 함께 남긴다. 상태줄 표시는 S2-T5다. 오류가 있어도 나머지는 적용한다.
10. 프리셋 파일: `assets/keymaps/default.json`, `vim.json`(바이너리에 `include_str!`)
    - 카탈로그에서 충돌이 없는 바인딩만 넣는다.
    - 충돌 13건의 키는 결정이 날 때까지 비워 두고, 결정 요청 `docs/decisions/2026-10-17-keymap-conflicts.md`에 모은다.

## 공개 인터페이스 초안

~~~rust
pub mod keymap {
    pub enum Preset { Default, Vim }
    pub struct Binding { /* keys: Vec<Keystroke 문자열>, context: Option<String>, action: ActionRef, origin: Origin(레이어, 줄, 열) */ }
    pub enum ActionRef { Named { name: String, args: Option<serde_json::Value> }, Unbind }
    pub struct KeymapError { /* line, column, message */ }
    pub fn parse(text: &str, layer: Layer) -> (Vec<Binding>, Vec<KeymapError>);
    pub struct LayerSet { /* standard, preset, user */ }
    pub fn conflicts(bindings: &[Binding]) -> Vec<Conflict>;
    pub fn to_key_bindings(layers: &LayerSet, options: &KeymapOptions, cx: &App) -> (Vec<KeyBinding>, Vec<KeymapError>);
    pub fn install(cx: &mut App);  // 부트·리로드·프리셋 전환
    pub struct KeymapStatus { /* errors: Vec<KeymapError> */ }  // Global
    pub fn take_count(window: &mut Window, cx: &mut App) -> Option<u32>;
}
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `req_example_keymap_snapshot` | 스냅샷(insta) | REQ 7.3 예시 JSON → 바인딩 목록(컨텍스트, 키, Action, 인자). 오류 없음 |
| 2 | `parse_errors_report_line_and_column` | 단위 | JSON 문법 오류, 모르는 Action, 잘못된 키(`ctl-x`), 잘못된 인자, 잘못된 컨텍스트에서 줄·열. 나머지 항목은 적용 |
| 3 | `same_physical_key_fires_same_action_in_korean_input` | UI | `key: h`·`key_char: ㅗ`인 입력이 `h` 바인딩 Action을 부른다(KM-7) |
| 4 | `conflicts_and_prefix_conflicts_are_detected` | 단위 | 같은 컨텍스트·같은 키에 두 Action, `g`와 `g g`. 다른 컨텍스트·레이어 덮어쓰기는 충돌 아님 |
| 5 | `more_specific_context_wins` | UI | `Normal > Arrangement`의 `enter`가 `Normal`의 `enter`를 이긴다. 깊이가 같으면 뒤 레이어 |
| 6 | `preset_switch_keeps_user_overrides` | UI | 사용자 오버라이드가 default·vim 전환 뒤에도 이긴다 |
| 7 | `cmd_shortcuts_are_preset_invariant` | 단위·UI | 프리셋 파일의 `cmd-` 표준 키 재정의는 거부되고, 두 프리셋에서 `cmd-s` 등이 같다 |
| 8 | `multi_key_sequence_and_count_prefix` | UI | vim `g g`가 Action을 부르고, `3 j`에서 `take_count()`가 3 |
| 9 | `user_keymap_hot_reloads_and_reports_errors` | UI | 파일을 고치면 1 s 주기 안에 새 바인딩. 오류 파일은 오류를 남기고 나머지 적용 |
| 10 | `swap_alt_cmd_option_rewrites_preset_bindings` | 단위 | KM-11 |
| 11 | `presets_have_no_conflicts_and_only_known_actions` | 단위 | 두 프리셋 파일이 오류·충돌 없이 읽힌다 |
| 12 | `every_catalog_action_is_registered` | 단위 | 카탈로그의 Action 이름 전부가 GPUI 레지스트리에 있다 |

## 불변식

- 바인딩의 출처(레이어·줄·열)는 늘 남는다. 오류와 충돌을 사람에게 가리킬 수 있어야 한다.
- 키맵 오류는 앱을 멈추지 않는다. 오류 항목만 빠진다.
- 표준 레이어의 `Cmd` 단축키는 프리셋으로 바뀌지 않는다.

## 범위 밖

- 키맵 편집 화면, press-to-bind, 충돌 시트, 내보내기·가져오기(KM-3·4 UI·8·9, S2-T6)
- `?` 오버레이(S2-T6), 팔레트·메뉴 7개(S2-T4), 모드 FSM과 컨텍스트 전환(S2-T3), 각 Action의 실제 동작(S3 이후)
- 충돌 13건의 해소: 결정 요청으로 미루고 해당 기능 단계에서 반영한다.

## 리스크·결정 요청

- 결정 요청 `docs/decisions/2026-10-17-keymap-conflicts.md`: 카탈로그 충돌 13건(A1–A13)과, 키 표기에 걸리는 모호 몇 건(B25 기호 키 표기 등)을 모은다. S2-T2를 막지 않는다. 충돌 키는 프리셋에서 비워 둔다.
- KM-7을 실제 한글 입력기에서 확인하는 것은 수동이다(S2 출구).
- 의존: `schemars`(GPUI가 이미 쓰는 1.x, MIT)를 앱에 직접 더한다.
