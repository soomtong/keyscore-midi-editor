# Task 계획: S3-T5 건반 뷰

- 브랜치: `s3/t5-keyboard-view`
- 관련 REQ: 5.1(옥타브 `[` `]`), 5.4(「화면 하단에 2옥타브 건반」, 옥타브 버튼), 5.6(옥타브 숫자, 변화표), 5.13(드럼 트랙 라벨), 2 「조성 표시」, 7.1(하단 영역 「건반 뷰 (Play·Step 모드)」), P3
- 선행 Task: S3-T4a·T4b·T4c(Step 상태 `step.rs`, 처리기 `step_input.rs`), S3-T1(`pianoroll::scene::row_shade`), S2-T1(하단 영역 `view::ToggleBottomPanel`)
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

하단 영역(`bottom-panel`)이 펴져 있고 모드가 Step이면, 그 자리에 건반 뷰를 그린다(피아노롤·배치 뷰 모두). 그 밖의 모드에서는 지금의 자리표시(컨트롤러 레인, S6)를 그대로 둔다. 하단 영역을 접었으면 건반 뷰도 그리지 않는다(사용자가 고른 레이아웃이 앞선다).

건반 뷰의 규칙(REQ에 없는 세부는 여기서 글로 정하고 시험으로 고정한다):

1. 건반: 24개, Step의 현재 옥타브 `o`의 C부터 `o+1`의 B까지(`C{o}` = `(o+1)*12`, C4 = 60). 음높이가 127을 넘는 건반은 「없음」으로 그리고(흐린 빈 자리) 라벨을 달지 않는다.
2. 키 라벨: 그 건반의 음을 Step에서 쓰는 키 입력. 옥타브 숫자 없이 닿는 키가 있으면 그것을, 없으면 옥타브 숫자를 붙인 입력을 보인다.
   - 건반형(P3 (a)): 첫 16건반(`C{o}`–`D#{o+1}`)에 `A W S E D F T G Y H U J K O L P`(키캡 글자, 대문자). 나머지 8건반(`E{o+1}`–`B{o+1}`)은 첫 옥타브의 같은 키 뒤에 옥타브 숫자 `o+1`(`D5`, `T5`). 건반형도 음 뒤 숫자가 옥타브다(REQ 5.6, 리뷰 1 지적 1). 이 입력도 옥타브를 바꾼다.
   - 계이름형(P3 (b)): 첫 옥타브 흰건반 `c d e f g a b`, 검은건반 `c# d# f# g# a#`(REQ 5.6 예 `f#`의 표기). 둘째 옥타브는 같은 글자 뒤에 옥타브 숫자 `o+1`(`c5`, `c#5`). 이 입력은 옥타브를 `o+1`로 바꾼다(REQ 5.6 스티키 규칙) — 라벨은 「이 음을 쓰는 입력」이지 「옥타브를 지키는 입력」이 아니다. `o+1`이 9를 넘으면(옥타브 9) 둘째 옥타브는 이미 1의 「없음」이다.
3. 음 이름: 멜로디 트랙은 건반마다 과학적 음이름(`C4`, `C#4`)을 모델에 두고, 그리기는 C 건반에만 보인다(좁은 검은건반에 글이 넘치지 않게). 드럼 트랙(채널 10, `Track::is_drum`)은 그 건반 음높이의 GM 라벨(`keyscore_text::drum::label_of`, 예 36 → `bd`)을 모든 건반에 보이고 표에 없는 음은 `n<번호>`(REQ 5.13 「표에 없는 노트 번호는 `n<번호>`로 보인다」, 리뷰 1 지적 2).
   - 에이전트 해석: 지금 Step은 드럼 트랙에서도 음 키를 멜로디 음높이로 쓴다(라벨 키 매핑·킷 페이지는 S6-T4). 건반 뷰는 「그 키가 지금 실제로 쓰는 음」의 라벨을 보인다. 단계 파일 S3-T5의 「드럼 트랙에서는 라벨 표시」를 이렇게 읽고, REQ 5.13 표의 키 배치(`A` = `bd`, `S` = `sd` …)로 바꾸는 일은 S6-T4 이월 줄로 넘긴다. 표시가 실제 입력과 어긋나는 상태(라벨 `bd`인데 C4가 써짐)를 만들지 않기 위해서다.
4. 조성 강조: 프로젝트 조성이 있으면 조성 안 음(장조·자연 단조 음계, 피아노롤 `row_shade`와 같은 함수)을 밝게, 밖 음을 어둡게 그린다. 흰·검은건반 구분은 그대로 두고 명암만 더한다(조성 밖 흰건반은 회색 쪽, 조성 밖 검은건반은 더 어둡게). 조성이 없으면 명암을 더하지 않는다. 「끌 수 있음」 설정은 피아노롤에도 아직 없다(아래 범위 밖).
5. 옥타브 버튼: 건반 왼쪽에 `−`(`step::OctaveDown`), 현재 옥타브 글 `oct4`, `+`(`step::OctaveUp`). 버튼은 키와 같은 Action을 `dispatch::run_action`으로 낸다(키 경로와 같은 처리기: 조합 확정·화음 닫기·범위 밖 메시지). 0·9에서도 눌리며 처리기가 범위 밖 메시지를 낸다(키 `[` `]`와 같은 결과).
6. 다시 그림: 옥타브·레이아웃(설정)·조성·현재 트랙(드럼 여부)이 바뀌면 다음 그림에 반영한다. 건반 뷰는 모델 함수(`keyboard_view::keys`)를 그릴 때마다 부르는 순수 계산이라 따로 기억하는 상태가 없다.
7. 그리기: 24건반은 GPUI `div` 사각형(절대 위치)으로 그린다. 요소 수가 고정 24개라 피아노롤처럼 인스턴싱 Element가 필요하지 않다. 건반마다 `id` `keyboard-key-<0..23>`, 접근성 라벨 `"<음 이름> <키 라벨>"`(예 `C4 A`, 라벨 없으면 음 이름만, 없음이면 `없음`), 뷰 전체 `keyboard-view`, 버튼 `keyboard-octave-down`·`keyboard-octave-up`, 옥타브 글 `keyboard-octave`.

## 공개 인터페이스 초안

~~~rust
// crates/keyscore-app/src/keyboard_view.rs (pub mod)
/// 건반 하나.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyCap {
    /// 127을 넘으면 `None`(없음).
    pub pitch: Option<Pitch>,
    pub black: bool,
    /// Step에서 이 음을 쓰는 키 입력(규칙 2).
    pub key: Option<String>,
    /// 음 이름 또는 드럼 라벨(규칙 3).
    pub name: Option<String>,
    /// 조성 밖이면 `true`(규칙 4). 조성이 없으면 늘 `false`.
    pub out_of_key: bool,
}

pub const KEY_COUNT: usize = 24;

/// 건반 뷰의 24건반.
#[must_use]
pub fn keys(layout: KeyLayout, octave: u8, key: Key, drum: bool) -> Vec<KeyCap>;

/// 시험 스냅샷용 한 줄씩 글(`C4 white A in`).
#[must_use]
pub fn describe(keys: &[KeyCap]) -> String;

// crates/keyscore-app/src/window.rs 또는 step_input.rs
impl ProjectWindow {
    /// 지금 그리는 건반 뷰(Step이 아니거나 하단 영역이 접혔으면 `None`). 시험·접근성용.
    pub fn keyboard_view(&self, cx: &App) -> Option<Vec<KeyCap>>;
}
~~~

`pitch_name`(`selection.rs`)은 `pub(crate)`로 열어 함께 쓴다.

## 테스트 목록

순수 시험 `crates/keyscore-app/tests/keyboard_view.rs`(insta 스냅샷 포함), UI 시험 `crates/keyscore-app/tests/keyboard_view_ui.rs`(헤드리스, MIDI 없음).

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `labels_by_layout_and_octave_snapshot` | 스냅샷 | 건반형 oct4·oct3·oct0, 계이름형 oct4·oct8의 24건반 글(게이트: 옥타브·레이아웃에 따른 라벨) |
| 2 | `labels_at_octave_nine_mark_keys_above_127_absent` | 단위 | oct9: `C9`–`G9`만 있고 나머지 없음·라벨 없음, 계이름형 둘째 옥타브 라벨 없음 |
| 3 | `drum_track_labels_snapshot` | 스냅샷 | 드럼, 건반형 oct2·oct3: 36 `bd`, 38 `sd`, 42 `ch` …, 표에 없는 음은 `n<번호>` |
| 4 | `out_of_key_follows_project_key` | 단위 | D장조에서 F# 안·F 밖, A단조에서 C 안·C# 밖, 조성 없음은 모두 안 |
| 5 | `piano_layout_labels_follow_p3_keys_then_octave_digit` | 단위 | 건반형 첫 16건반은 P3 표 순서, 뒤 8건반은 같은 키 + 옥타브 숫자(리뷰 1 지적 1) |
| 6 | `step_shows_keyboard_view_in_bottom_panel` | UI | Normal에는 `keyboard-view` 없음, Step 들어가면 있음, `Esc`로 나가면 없음 |
| 7 | `keyboard_view_follows_octave_keys` | UI | `]` 뒤 `keyboard-key-0` 라벨 `C5 A`, `keyboard-octave` `oct5` |
| 8 | `octave_buttons_run_step_octave_actions` | UI | `+` 클릭 → 옥타브 5, 이어 `a` 키가 C5를 쓴다(포커스가 문서에 남음). `−` 두 번 → 3 |
| 9 | `octave_button_settles_composing_like_key` | UI | 계이름형 `c` 조합 중 `+` 클릭 → C4가 확정되고 옥타브 5 |
| 10 | `octave_button_at_bound_says_range` | UI | 옥타브 9에서 `+` → 9 그대로, 키 `]`와 같은 메시지 |
| 11 | `keyboard_view_in_arrangement_step` | UI | 배치 뷰에서 Step에 들어가도 건반 뷰가 있다 |
| 12 | `closed_bottom_panel_hides_keyboard_view` | UI | 하단 영역을 접으면 Step에서도 없음, 다시 펴면 있음 |
| 13 | `layout_setting_change_updates_labels` | UI | 설정을 계이름형으로 바꾸면 `keyboard-key-0` `C4 c` |
| 14 | `drum_track_shows_gm_labels` | UI | 채널 10 트랙, oct2: `keyboard-key-0` `bd A` |
| 15 | `key_edit_updates_shading` | UI | 조성을 D장조로 바꾸면 `keyboard_view` F 건반 `out_of_key` |
| 16 | `typed_labels_write_shown_pitches` | UI | 두 레이아웃에서 라벨이 있는 건반마다 그 라벨을 Step에 치면 그 건반 음이 써진다(계이름형 둘째 옥타브는 칠 때마다 옥타브를 되돌린다) |

## 불변식

- 건반 뷰는 문서·Step 상태를 바꾸지 않는다(그리기 경로에서 편집 없음). 바꾸는 길은 버튼이 내는 Action뿐이고, 그 처리기는 키와 같다.
- 라벨과 실제 입력이 어긋나지 않는다: 라벨 계산은 Step이 쓰는 표(`pianoroll::keys::semitone_of_key`, `step::pitch_of`)로 거꾸로 맞춘다(시험 16).
- 실시간·MIDI 경로를 건드리지 않는다.

## 범위 밖

- Play 모드의 건반 뷰와 「연주」 버튼, Play 옥타브: S5(단계 파일 S3-T5 「Play 모드는 S5」, S5 Play Task 범위). S5 단계 파일에 이월 줄을 단다.
- 드럼 트랙 라벨 키 매핑(REQ 5.13 표의 `A` = `bd` 배치)과 킷 페이지: S6-T4. 그때 건반 뷰 라벨도 그 매핑으로 바꾼다(S6 단계 파일에 이월 줄).
- 조성 표시 끄기 설정(REQ 2 「끌 수 있음」): 피아노롤에도 없다. 지금 단계 파일 어디에도 배정되지 않았으므로 S3 단계 파일 S3-T5 아래에 남은 일로 적고 사람에게 보고한다.
- 조합 중인 음·지금 소리 나는 음의 건반 강조, 건반 클릭으로 음 넣기: REQ에 없다.
- 색만 다른 두 상태(조성 안·밖) 접근성 검사: S8(단계 파일 18 S8 「색만 다른 두 상태가 없음」).

## 리스크·결정 요청

- 규칙 2의 계이름형 둘째 옥타브 라벨(`c5`)과 건반형 뒤 8건반 빈 라벨, 규칙 3의 드럼 라벨 해석은 REQ 문장(「어느 키가 어느 음인지 보여 준다」)을 이 계획의 글로 구체화한 것이다. 사람이 다르게 원하면 결정 요청으로 바꾼다.
- 하단 영역이 컨트롤러 레인과 건반 뷰를 함께 쓰는데(7.1 그림 「/」), 컨트롤러 레인은 S6이라 지금은 「Step이면 건반 뷰」만 정한다. S6에서 레인과의 전환 규칙을 정한다.
