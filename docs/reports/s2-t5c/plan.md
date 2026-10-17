# Task 계획: S2-T5c 트랙 인스펙터와 상태줄

- 브랜치: `s2/t5c-inspector-status`(S2-T5를 나눈 셋째 Task)
- 관련 REQ: UI-5, UI-11, UI-12, SEQ-7, PRE-3, KM-6, KM-14, 7.1(화면 구성), 7.6(인스펙터·상태줄 행), 11절 컴포넌트 표(`Select` GM 이름 검색)
- 선행 Task: S2-T5b(현재 트랙, 트랜스포트 입력란 규칙, 포트 메뉴), S2-T5a(엔진), S2-T2(키맵 오류 수집)
- 관련 스파이크: 없음
- 이월 받음: 인스펙터 입력란은 모드 키 컨텍스트 겹 바깥(S2-T5b 리뷰 1 지적 1), 창 비활성화와 Blur(S2-T5b 리뷰 3 제안 5)

## 나눔

단계 파일의 S2-T5c는 인스펙터·상태줄과 포트 감시(OUT-3)를 함께 적었다. 포트 감시는 CoreMIDI 알림 스레드·재연결·재등록·연 포트 정리까지 엔진 쪽 일이 커서 S2-T5d로 나눈다(T5a·T5b를 나눈 것과 같은 이유). 단계 게이트 「포트 소멸·복귀가 상태줄에 보임」은 S2-T5d가 맡는다. 이 Task의 상태줄은 그 표시가 들어갈 자리(포트 이름 옆)를 둔다.

## 끝의 정의 (내 말로)

1. 키 컨텍스트 자리
   - 모드 키 컨텍스트 겹(포커스 핸들 포함)은 가운데 주 뷰만 감싼다. 트랙 목록·인스펙터·아래 패널·상태줄은 겹 바깥이다. 키 디스패치는 포커스 자리에서 시작하므로 모드 키는 그대로이고, 인스펙터 입력란에 친 글자는 모드 바인딩으로 빠지지 않는다.
2. 트랙 인스펙터(UI-12, 7.1 오른쪽, `Cmd+I` 토글은 이미 있음). 현재 트랙을 다룬다. 트랙이 없으면 「트랙이 없다」만 보인다.
   - 이름: 입력란. Enter로 확정(`TrackProp::Name`), Esc·포커스 잃음은 취소. 트랜스포트 입력란과 같은 규칙(고친 필드만 확정, 같은 값은 편집 없음, 문서가 바뀌면 고치지 않은 필드는 따라감).
   - 색: 12색 메뉴(색 견본 + 이름).
   - 포트: 트랜스포트와 같은 포트 메뉴(가상 포트 + OS 목적지).
   - 채널: 1–16 메뉴(10은 「드럼」).
   - 뱅크: 입력란(0–16383, 비우면 없음).
   - 프로그램: gpui-component `Select`(검색 가능). 「없음」 + GM 128개(`1 Acoustic Grand Piano` … `128 Gunshot`). 검색어는 번호·이름 어디든 맞는다.
   - 기본 벨로시티: 입력란(1–127).
   - 가사 언어: 메뉴(없음, ko, en, ja, 지금 값이 그 밖이면 그 값도).
   - 뮤트·솔로: 토글 버튼(`track::ToggleMute`/`ToggleSolo`를 실행 함수로).
   - 테스트: 버튼(`midi::SendTestNote`를 실행 함수로).
   - 편집 하나가 undo 한 단위다. 메뉴·드롭다운에서 지금 값을 다시 고르면 편집하지 않는다.
   - 버튼·메뉴를 쓴 뒤 포커스는 문서로 돌아간다(트랜스포트 바와 같다).
3. 인스펙터 Action 처리기(UI-11: 팔레트·키맵에서도 같은 일)
   - `track::Rename(name)`, `SetColor(color)`(팔레트 이름 `red` … 또는 0–11), `SetPort(name)`(빈 문자열·`virtual`은 가상 포트), `SetProgram{bank, program}`(program 1–128 GM 번호, 0은 없음; bank 0–16383, 음수는 없음), `SetDefaultVelocity(1–127)`, `SetLyricLang(lang)`(빈 문자열은 없음). 잘못된 값은 편집하지 않고 메시지.
   - `midi::SendTestNote`: 현재 트랙의 포트·채널로 가운데 C(60)를 트랙 기본 벨로시티, 미리듣기 길이(200 ms)로 보낸다(PRE-3). 재생 중이 아니면 먼저 그 트랙의 뱅크·프로그램을 보낸다. 이 창이 재생 중이면 지금 스냅샷의 포트로 보낸다. 다른 창이 재생 중이고 이름 포트면 보내지 않고 알린다. 포트가 없으면 알린다. 보냈으면 「테스트 노트를 보냈다: 포트 · ch N. 신디에서 들리는지 확인한다」를 남긴다.
4. 상태줄(UI-5, 7.1 아래)
   - 왼쪽: 모드 표시(누르면 모드 메뉴, 이미 있음).
   - 가운데: 현재 트랙 「트랙 이름 · 포트 이름 chN」(트랙이 없으면 비움).
   - 오른쪽: 키맵 오류가 있으면 그것(KM-6, `사용자 키맵 3:5: …`, 경고색), 없으면 마지막 메시지.
   - 키맵 파일을 다시 읽으면(핫리로드) 열린 창의 상태줄이 바로 바뀐다.
   - 길이 코드·옥타브(UI-5)는 Step 입력(S5)과 함께 넣는다.
5. 창 비활성화(이월): 고치던 입력란에서 창이 비활성이 되면 입력을 지킨다(macOS 관례). 창 안의 다른 곳으로 포커스가 옮겨 갈 때만 취소한다. 트랜스포트 입력란에도 같게 적용한다.

## 공개 인터페이스 초안

~~~rust
// keyscore_app::gm
pub const GM_PROGRAMS: [&str; 128];
pub fn program_label(program: Option<Program>) -> String; // "1 Acoustic Grand Piano", "없음"
// keyscore_app::tracks
pub fn parse_bank(text: &str) -> Result<Option<Bank>, String>;
pub fn parse_velocity(text: &str) -> Result<Velocity, String>;
pub fn parse_color(text: &str) -> Result<TrackColor, String>;
pub fn status_track_text(project: &Project, track: TrackId, virtual_name: &str) -> String;
// ProjectWindow (테스트용 읽기)
pub fn status_text(&self, cx: &App) -> String; // 상태줄 오른쪽에 보이는 글
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `gm_program_names_and_parsers` | 단위 | GM 이름 128개, 번호 표기, 뱅크·벨로시티·색 파싱 경계 |
| 2 | `inspector_shows_current_track` | UI | 현재 트랙을 바꾸면 인스펙터 값이 바뀐다. 트랙이 없으면 빈 안내 |
| 3 | `inspector_name_field_edits_are_undo_units` | UI | 이름·뱅크·벨로시티 입력 Enter가 각각 undo 한 단위. 같은 값은 편집 없음, 잘못된 값은 메시지 |
| 4 | `inspector_typing_does_not_reach_mode_keys` | UI | 이름 입력란에 `space`·`m`·숫자를 쳐도 재생·뮤트·카운트가 일어나지 않는다(두 프리셋) |
| 5 | `inspector_menus_edit_track` | UI | 색·채널·가사 언어 메뉴가 각각 undo 한 단위. 같은 값은 편집 없음. 메뉴 뒤 모드 키가 산다 |
| 6 | `inspector_program_select_searches_gm_names` | UI | 프로그램 `Select`에서 `piano`를 치고 고르면 프로그램이 바뀐다(undo 한 단위) |
| 7 | `inspector_actions_validate_arguments` | UI | `track::Rename`·`SetColor`·`SetPort`·`SetProgram`·`SetDefaultVelocity`·`SetLyricLang`의 정상·경계·잘못된 값 |
| 8 | `inspector_mute_solo_test_buttons_run_actions` | UI | 뮤트·솔로 버튼, 테스트 버튼(엔진 없으면 메시지) |
| 9 | `send_test_note_reaches_track_port` | UI + 루프백 | 이름 포트 트랙의 테스트 노트가 그 목적지에 프로그램 → Note On → Note Off로 닿는다. 재생 중에도 닿는다 |
| 10 | `status_bar_shows_track_and_messages` | UI | 가운데 「트랙 · 포트 chN」, 오른쪽 마지막 메시지. 트랙·포트·채널 편집을 따라간다 |
| 11 | `status_bar_shows_keymap_errors` | UI | 사용자 키맵에 문법 오류를 쓰면 상태줄에 줄·칸과 함께 보이고, 고치면 사라진다 |
| 12 | `inactive_window_keeps_field_text` | UI | 고치던 이름·템포 입력란에서 창이 비활성이 되었다 돌아와도 글자가 남고 Enter로 확정된다 |

## 불변식

- 인스펙터의 모든 편집은 `ProjectWindow::edit_document` 한 번 = undo 한 단위다.
- 인스펙터는 늘 현재 트랙을 보인다(현재 트랙은 문서에 있는 트랙이거나 없음).
- 버튼은 실행 함수를 거친다(결정 C).

## 범위 밖

- 포트 감시·재연결·재등록·연 포트 정리, 상태줄의 포트 소멸·복귀 표시(S2-T5d)
- 상태줄의 길이 코드·옥타브·박 표시(UI-5 일부, MET-2: S5 Step 입력·메트로놈 표시와 함께)
- 포트 리스너 표시(PRE-2: 온보딩과 함께 S2-T7 이후), 설정 > MIDI 출력의 테스트 버튼(설정 화면 Task)
- 클립·노트 인스펙터(REQ에 없다), Fluent 문자열 외부화(국제화 Task)

## 리스크·결정 요청

- `track::SetProgram`의 인자 의미(GM 번호 1–128, 0은 없음, 뱅크 음수는 없음)와 `SetColor`의 이름은 Action 카탈로그가 정하지 않은 세부다. 화면에 보이는 번호(GM 1–128)와 같게 골랐다.
- 창 비활성화 때 입력을 지키는 것은 이월 줄이 「정한다」고 적은 세부로, macOS 관례를 따른다.
