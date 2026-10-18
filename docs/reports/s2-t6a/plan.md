# Task 계획: S2-T6a 설정 창과 키맵 편집기

- 브랜치: `s2/t6-settings-keymap`
- 관련 REQ: KM-2, KM-3, KM-4, KM-6, KM-8, KM-9, KM-11, KM-12, 7.3, 7.5(`Cmd+,`), 7.6(설정 > 키맵), 11절 컴포넌트 표, P1
- 선행 Task: S2-T2(키맵 엔진, `keymap::conflict`), S2-T4(명령 표), S2-T5c(입력란 규칙)
- 관련 스파이크: 없음
- 결정: A13(Esc = 통상의 취소, 대화상자 `dialog::Cancel` 신규)

## 나눔

단계 파일의 S2-T6은 키맵 편집기, `?` 오버레이, 설정 탭 넷(입력·MIDI 출력·메트로놈·AI)을 함께 적었다. 리뷰 단위를 줄이려고 셋으로 나눈다. REQ 범위와 단계 게이트는 그대로다.
- S2-T6a(이 Task): 설정 창 틀과 키맵 편집기(KM-3·4·8·9, 프리셋 전환, 수식키 교환). 단계 게이트 둘(키보드만으로 편집기 끝까지, 가져온 키맵 충돌을 적용 전에 잡기)을 맡는다.
- S2-T6b: `?` 키맵 오버레이(KM-10, UI-6, `help::CloseOverlay`).
- S2-T6c: 설정 > 입력·MIDI 출력·메트로놈·AI 탭, 이월 줄(undo 메모리 상한 결정 요청, 패널 너비, 바탕 레이어 `alt-cmd-i`, 창 최소 크기).

## 끝의 정의 (내 말로)

1. 설정 창
   - `settings::Open(구역)`(`Cmd+,`)과 `keymap::Edit`(앱 메뉴 「키맵 편집」)이 설정 창을 연다. 이미 열려 있으면 그 창을 앞으로 가져오고 구역만 바꾼다. 창은 앱에 하나다.
   - 왼쪽에 구역 목록(입력, MIDI 출력, 메트로놈, 키맵, AI). 구역 이동은 클릭 또는 `ctrl-tab`/`ctrl-shift-tab`(새 Action `settings::NextSection`/`PrevSection`). 키맵 밖 구역은 이 Task에서 「S2-T6c에서 채운다」 자리표시자다. `구역` 없이 열면 키맵이다(이 Task 동안).
   - `cmd-w`로 닫는다(표준 레이어의 창 닫기).
2. 키맵 편집기(KM-3)
   - 위에 프리셋 선택(기본/vim, `keymap::SelectPreset`), 수식키 교환 토글(`keymap::SwapModifiers`), 내보내기·가져오기 버튼, 검색란.
   - 목록: Action마다 한 줄(명령 표의 제목, 이름, 지금 걸린 키와 그 컨텍스트). 「지금 걸린 키」는 레이어를 겹친 뒤의 실효 바인딩(같은 (컨텍스트, 키)는 뒤 레이어가 이기고, `null`은 지운다)이다.
   - 목록에 포커스가 있을 때(컨텍스트 `Keymap > Editing`): `up`/`down`(새 `keymap::SelectPrev`/`SelectNext`), `/`(새 `keymap::FocusSearch`), `enter`(`StartCapture`), `backspace`(`ClearBinding`), `r`(`RestoreDefault`).
   - 검색란: 이름·제목·설명에 들어 있는 글로 거른다. `enter`나 `down`으로 목록으로 돌아간다. 입력란 규칙(Esc는 검색어를 비우고 목록으로)은 S2-T5c와 같다.
3. press-to-bind(KM-3·8)
   - `enter`로 캡처를 시작하면 다음 키들을 받는다. 마지막 키 뒤 1 s(KM-5 시퀀스 대기)가 지나면 그 시퀀스로 확정한다. 수식키 없는 `escape`는 캡처를 취소한다(KM-8). 캡처 중의 키는 바인딩으로 디스패치되지 않는다.
   - 확정하면 그 Action의 바인딩 컨텍스트에서 그 Action의 키를 새 시퀀스로 바꾼다. 컨텍스트는 그 Action의 지금(없으면 프리셋의) 첫 바인딩의 컨텍스트, 없으면 전역이다.
   - 표준 단축키(`Cmd` 7.5)를 덮으면 경고를 보인다(KM-12).
4. 충돌(KM-4)
   - 확정 전에 같은 컨텍스트의 실효 바인딩과 비교한다. 같은 시퀀스가 다른 Action에 걸려 있거나 접두 관계(`g`와 `g g`)면 충돌 시트를 연다.
   - 시트: 「새 바인딩 유지(그쪽 해제)」(`enter`, 새 `dialog::Confirm`)와 「취소」(`escape`, 새 `dialog::Cancel`, A13). 클릭도 된다.
5. 지우기·복원(KM-3)
   - `backspace`: 그 Action의 실효 키를 모두 지운다(사용자 항목은 빼고, 프리셋·표준 항목은 사용자 레이어에 `null`).
   - `r`: 그 Action에 관한 사용자 항목(그 Action으로의 바인딩, 그 Action의 프리셋 키를 지우는 `null`)을 모두 빼 프리셋 기본값으로 되돌린다.
6. 사용자 파일(KM-6)
   - 편집은 `keymap.json`을 고쳐 쓴다(같은 파일을 엔진과 외부 편집기가 함께 쓴다, 7.3 도입). 쓰기 전 내용은 `keymap.json.bak`으로 남긴다(편집기가 다시 쓰면 주석·순서가 사라진다). 임시 파일 + rename.
   - 쓴 뒤 곧바로 다시 건다(`keymap::apply`). 파일이 문법 오류로 읽히지 않는 상태면 편집기는 고쳐 쓰지 않고 알린다(사용자의 손 편집을 덮지 않는다).
7. 내보내기·가져오기(KM-9)
   - 내보내기: 저장 위치를 묻고 지금 사용자 레이어를 JSON 파일 하나로 쓴다.
   - 가져오기: 파일을 고르면 읽고(문법 오류면 알리고 중단), 같은 레이어 안의 충돌(`keymap::conflict`)과 표준 단축키 덮기를 먼저 보인다. 충돌이 있으면 시트: 「충돌마다 파일에서 뒤 항목을 남기고 적용」(`dialog::Confirm`) 또는 「취소」(`dialog::Cancel`). 적용은 사용자 레이어를 가져온 내용으로 바꾼다(`.bak` 남김).
8. 설정 창의 키는 모드 키 컨텍스트와 무관하다(다른 창). 창의 루트 컨텍스트는 `Settings`, 키맵 구역은 `Keymap`, 목록은 `Keymap > Editing`, 시트는 `Dialog`다.

## 공개 인터페이스 초안

~~~rust
// keyscore_app::keymap::user (사용자 레이어 편집, GPUI 무관)
pub struct UserKeymap { /* 사용자 항목 Vec<Binding> */ }
impl UserKeymap {
    pub fn parse(text: &str) -> Result<Self, Vec<KeymapError>>;
    pub fn to_json(&self) -> String;
    pub fn bind(&mut self, action: &str, context: Option<&str>, keys: &str, effective: &[Effective]);
    pub fn clear(&mut self, action: &str, effective: &[Effective]);
    pub fn restore(&mut self, action: &str, preset: &[Binding]);
}
pub struct Effective { pub context: Option<String>, pub keys: String, pub action: String, pub layer: Layer }
pub fn effective(layers: &LayerSet) -> Vec<Effective>;
pub fn binding_conflicts(effective: &[Effective], context: Option<&str>, keys: &str, action: &str) -> Vec<Effective>;
// keyscore_app::settings_window
pub fn open(cx: &mut App, section: Option<SettingsSection>);
pub fn export_to(cx: &mut App, path: &Path) -> Result<(), String>;
pub fn import_from(cx: &mut App, path: &Path) -> Result<ImportCheck, String>;
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `user_keymap_round_trips_and_edits` | 단위 | 사용자 레이어 읽기·쓰기 왕복, `bind`(같은 컨텍스트의 옛 키 대체), `clear`(프리셋 키는 `null`), `restore` |
| 2 | `effective_bindings_layer_and_unbind` | 단위 | 실효 바인딩: 뒤 레이어가 이기고 `null`은 지운다. 충돌 찾기(같은 키, 접두 양방향) |
| 3 | `settings_open_and_sections` | UI | `Cmd+,`가 설정 창을 연다(하나만), `keymap::Edit`이 키맵 구역, `ctrl-tab`으로 구역 이동 |
| 4 | `keymap_editor_keyboard_only_flow` | UI(단계 게이트) | 키보드만: `/`로 검색 → 거른 결과 → `enter`로 목록 → `enter` 캡처 → 키 → 1 s → 파일에 기록·엔진에 걸림(그 키가 Action을 낸다) → `backspace` 지우기 → `r` 복원 |
| 5 | `capture_escape_cancels_and_keys_do_not_dispatch` | UI | 캡처 중 키가 바인딩으로 디스패치되지 않고, `escape`가 취소한다(KM-8) |
| 6 | `binding_conflict_sheet_asks_which_to_keep` | UI | 같은 컨텍스트의 같은 키 → 시트. `escape` 취소면 그대로, `enter`면 새 바인딩이 이기고 옛 Action은 그 키를 잃는다. 접두 충돌도 시트 |
| 7 | `preset_switch_keeps_user_overrides` | UI | 프리셋 선택이 엔진·설정에 반영되고 사용자 항목은 남는다(KM-2). 수식키 교환 토글 |
| 8 | `import_conflicts_are_caught_before_apply` | UI(단계 게이트) | 충돌 있는 파일 가져오기 → 시트 → 취소면 바뀌지 않음, 적용이면 뒤 항목만 남음. 문법 오류 파일은 거부 |
| 9 | `export_writes_user_layer` | UI | 내보내기 경로에 사용자 레이어 JSON. 다시 읽으면 같다 |
| 10 | `editor_refuses_to_overwrite_broken_user_file` | UI | 손으로 깨뜨린 `keymap.json`이면 편집기가 쓰지 않고 알린다 |

## 불변식

- 편집기는 사용자 레이어만 고친다. 표준·프리셋 파일은 바꾸지 않는다.
- 편집기가 쓴 파일은 엔진이 오류 없이 읽는다(왕복).
- 쓰기는 원자적이다(임시 파일 + rename).

## 범위 밖

- `?` 오버레이(S2-T6b), 다른 설정 구역(S2-T6c)
- 바탕 레이어(gpui-component) 바인딩 편집: 목록에 보이지 않는다. 보여 줄지는 S2-T6c 이월 줄과 함께 정한다.
- 결정 대기 키(A1–A12, B24, B25): 편집기는 그 키를 특별히 다루지 않는다.

## 리스크·결정 요청

- 「Enter 후 키를 직접 눌러」의 시퀀스 끝을 1 s 대기로 정한 것은 KM-5의 시퀀스 대기와 같게 맞춘 세부다.
- 바인딩 대상 컨텍스트(그 Action의 첫 바인딩 컨텍스트)는 REQ가 정하지 않은 세부다. 같은 Action을 다른 컨텍스트에 따로 거는 일은 JSON 손 편집으로 한다.
- 편집기가 파일을 다시 쓰면 주석이 사라진다. `.bak`으로 남긴다.
