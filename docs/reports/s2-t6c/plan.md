# Task 계획: S2-T6c 설정 > 입력·메트로놈·AI

- 브랜치: `s2/t6c-settings-tabs`
- 관련 REQ: MET-1, P3, UI-10(키 레이아웃 선택 자리), UI-11, 7.6(메트로놈 포트·채널·노트, 레이턴시 보정, 키 레이아웃, AI 설정), AI-1(자리만)
- 선행 Task: S2-T6a(설정 창), S2-T5a(엔진·메트로놈 Action), S2-T5d(포트 감시)
- 관련 스파이크: 없음
- 나눔: 단계 파일의 S2-T6c를 2026-10-19에 다시 나눴다. MIDI 출력 탭은 S2-T6d, 이월 줄은 S2-T6e다.

## 끝의 정의 (내 말로)

1. 설정 파일(`settings.json`)에 두 객체를 더한다. 모르는 키는 보존하고, 잘못된 값은 그 필드만 기본값으로 두고 경고한다(기존 `layout`·`keymap`과 같은 태도).
   - `input`: `key_layout`(`"piano"` 기본, `"solfege"`).
   - `metronome`: `port`(`"virtual"` 기본 또는 `{"named": 이름}`, `.ksq` 트랙 포트와 같은 모양), `channel`(1–16, 기본 10), `accent`(0–127, 기본 37), `beat`(0–127, 기본 42), `length_ms`(1–10000, 기본 50).
2. 설정 > 입력
   - 키 레이아웃 선택(건반형/계이름형). 고르면 `settings::SetKeyLayout`을 낸다. 처리기는 앱 전역이라 팔레트에서도 같은 값이 바뀐다. 실제 음 키 해석은 S3(Step 입력)이 이 값을 읽는다.
   - 「보정 마법사」 버튼. `settings::Calibrate`를 낸다. 마법사는 S5-T2가 만든다. 지금은 설정 창을 입력 구역으로 열고 「보정 마법사는 녹음 단계(S5)에서 들어온다」를 알린다.
3. 설정 > 메트로놈(MET-1)
   - 포트(기본 가상 포트 + 지금 보이는 CoreMIDI 목적지 + 목록에 없는 지금 값), 채널(1–16), 강박 노트, 약박 노트(0–127, 표기는 번호와 GM 타악기 이름(35–81만, 예 「37 · Side Stick」)), 길이(ms). 모두 키보드로 고를 수 있는 선택 상자(`Select`)다. 길이는 목록(10–500 ms 사이 흔한 값)에서 고르고, 파일에 손으로 넣은 다른 값은 목록에 덧붙여 보인다.
   - 바꾸면 곧바로 설정에 남고(1 s 디바운스 저장) 엔진에 반영된다. 재생 중이면 지금 재생하는 창이 스냅샷을 다시 만들어 클릭 포트·채널·노트·길이가 바로 바뀐다.
   - 재생 시작·재생 중 편집 때 엔진은 메트로놈 포트를 트랙 포트와 같은 규칙으로 연다(같은 이름은 고유 ID 최소, 없으면 소리 없음). 포트를 찾지 못하면 클릭은 나지 않고 상태줄에 「메트로놈 포트 「X」가 없어 클릭이 나지 않는다」를 알린다. 버튼 상태(켬)는 그대로 둔다.
   - 포트 감시가 그 이름의 소멸·복귀를 알리면 트랙 포트와 같이 다시 준비한다.
4. 설정 > AI: 비활성 자리. 엔드포인트·모델 두 자리·키·템플릿 폴더·취향 지시 항목 이름을 비활성으로 보이고 「S7에서 켠다」를 적는다(7.6 대응표의 자리).
5. 키보드: 설정 창의 구역 안 컨트롤은 `Tab`/`Shift+Tab`으로 옮겨 다니고(REQ 5.11 표 「키맵 편집 · 설정 · 대화상자: 다음 필드」), 선택 상자는 `Enter`/`space`로 열고 화살표로 고른다. 구역을 바꾸면 그 구역 첫 컨트롤에 포커스가 간다.

## 공개 인터페이스 초안

~~~rust
// keyscore_app::settings
pub struct InputSettings { pub key_layout: KeyLayout }
pub struct MetronomeSettings {
    pub port: OutputPort, pub channel: u8, pub accent: u8, pub beat: u8, pub length_ms: u32,
}
impl MetronomeSettings { pub fn config(&self, port_index: u16) -> keyscore_midi::MetronomeConfig; }
pub struct Settings { /* … */ pub input: InputSettings, pub metronome: MetronomeSettings }

// keyscore_app::engine
pub(crate) struct Prepared { /* … */ pub(crate) metronome: Option<u16> }

// keyscore_app::settings_window (시험 API)
impl SettingsWindow {
    pub fn set_metronome(&mut self, edit: impl FnOnce(&mut MetronomeSettings), cx: &mut Context<Self>);
}
// keyscore_app::window::ProjectWindow
pub fn metronome_port_missing(&self) -> bool;
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `settings_input_and_metronome_round_trip` | 단위 | 두 객체 읽기·쓰기 왕복, 모르는 키 보존, 범위 밖·잘못된 형식은 필드만 기본값 + 경고 |
| 2 | `set_key_layout_updates_settings_and_input_tab` | UI | 팔레트 경로(`settings::SetKeyLayout`)와 입력 탭 선택이 같은 설정을 바꾸고 저장된다 |
| 3 | `calibrate_opens_input_section_with_notice` | UI | `settings::Calibrate`가 설정 창을 입력 구역으로 열고 알린다 |
| 4 | `metronome_settings_change_clicks` | 루프백 | 채널 9·노트 60/61로 바꾼 뒤 재생하면 그 채널·노트로 클릭이 온다 |
| 5 | `metronome_change_applies_while_playing` | 루프백 | 재생 중 노트를 바꾸면 다음 클릭부터 새 노트 |
| 6 | `metronome_named_port_routes_and_missing_is_reported` | 루프백 | 이름 포트(테스트가 만든 가상 목적지)로 보내면 그 포트에서 받고 기본 포트에는 오지 않는다. 없는 이름이면 클릭 없음 + 상태 메시지 |
| 7 | `settings_sections_keyboard_reachable` | UI | 입력·메트로놈 구역에서 `Tab`으로 컨트롤을 차례로 지나고, 선택 상자를 키보드로 열어 값을 바꾼다 |
| 8 | `ai_section_is_disabled_placeholder` | UI | AI 구역에 항목 이름이 비활성으로 보이고 아무것도 바꾸지 못한다 |

## 불변식

- 설정 파일의 모르는 키(최상위·`input`·`metronome` 안)는 쓸 때 그대로 남는다.
- 엔진에 넘기는 메트로놈 값은 항상 범위 안이다(읽을 때 고친다, 선택 상자는 범위 안 값만 낸다).
- 클릭은 메트로놈 포트가 이 스냅샷에 있을 때만 켠다(없는 포트 번호로 클릭을 보내지 않는다).

## 범위 밖

- MIDI 출력 탭(가상 포트 추가·이름, 포트별 클럭, 테스트 노트, 리스너 표시): S2-T6d.
- 이월 줄(undo 상한 알림, `alt-cmd-i`, 패널 너비, 창 최소 크기, 오버레이 인자 라벨): S2-T6e.
- 보정 마법사 자체(S5-T2), 키 레이아웃에 따른 음 키 해석(S3), AI 설정 내용(S7).
- Sequence 녹음·카운트인의 강제 클릭(MET-1 뒤 절, S5).
- 클릭 벨로시티: REQ에 설정 항목이 없어 S1-T5의 고정값(127/100)을 둔다.

## 리스크·결정 요청

- 길이 목록 값(10, 20, 30, 50, 80, 100, 150, 200, 300, 500 ms)은 UI 세부다. 파일에는 1–10000 ms 어떤 값이든 들어가고 화면은 그 값을 보인다.
- 메트로놈 포트가 없을 때 버튼을 켠 채 두는 것은 트랙 포트가 없을 때와 같은 태도다(포트가 돌아오면 다시 준비해 바로 울린다).
