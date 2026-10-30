# Task 계획: S3-T4c 미리듣기 가벼운 경로와 배치 뷰 Step 표시

- 브랜치: `s3/t4c-preview-arrangement-step`
- 관련 REQ: 5.1, 5.6(Step 미리듣기·셋잇단 표시), 5.10(클립 밖 입력은 마디 단위 새 클립), 5.11, PRE-1, PRE-3, OUT-4
- 선행 Task: S3-T4a, S3-T4b(Step 상태 `step.rs`, 처리기 `step_input.rs`), S3-T2(`preview::sound_note`)
- 관련 결정: `docs/decisions/2026-10-29-s3-t4-step-input-details.md` 7절(나눔), `2026-10-17-mode-entry-view.md`(배치 뷰 Step 이월), `2026-10-30-s3-t4b-vim-step-mml.md`(리뷰 3 제안 2 문구)
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

1. 정지 중 이름 포트 트랙의 미리듣기가 음 키마다 `Engine::prepare`(프로젝트 전체 렌더 + 목적지 열거) + `swap`을 하지 않는다. 엔진이 마지막 미리듣기 스냅샷의 포트 구성(트랙 포트 순서, 메트로놈 포트, 클럭 포트)과 경로표를 기억하고, 구성이 같고 그 사이 스냅샷 교체·포트 변화가 없으면 그 경로표로 바로 보낸다. 정지 중 미리듣기에는 스냅샷의 이벤트가 쓰이지 않으므로 문서 revision은 열쇠에 넣지 않는다.
   - 무효화(단계 파일의 「엔진 교체 세대」에 해당): `prepare`(재생 시작·재생 중 편집), 엔진이 직접 넣는 스냅샷(포트 지우기·이름 바꾸기, 서버 복구, 추가 포트 재등록), 추가 가상 포트 만들기·대체 매핑, 포트 감시의 사라짐·돌아옴과 목적지 목록 변화.
   - 포트를 찾지 못한 결과는 기억하지 않는다(다음 미리듣기가 다시 찾는다. 그 사이 포트를 만들 수 있다).
2. 뱅크·프로그램을 미리듣기마다 다시 보내지 않는다. 엔진이 (목적지, 채널)마다 마지막으로 보낸 뱅크·프로그램 묶음을 기억하고, 보낼 묶음이 같으면 건너뛴다. 위 1의 무효화 때 함께 잊는다(재생이 song의 음색을 보냈을 수 있다). 테스트 노트(PRE-3)는 연결 확인이므로 늘 보내고 기억도 갱신한다.
3. 배치 뷰에서 Step 모드면 주 뷰에 Step 위치 표시를 그린다: 현재 트랙 이름과 커서 위치(`마디.박.틱`), 그리고 커서가 클립 안이면 그 클립, 클립 끝에 이어 쓰면 그렇다고, 클립 밖이면 「입력하면 마디 a–b에 새 클립」(지금 길이의 음 하나를 쓴다고 보고 `clip_for_notes`로 계산), 쓸 수 없으면 그 이유. 배치 뷰 Element(트랙×시간 격자)는 S4-T1이므로, 이 Task는 자리표시 영역 안의 글 표시까지만 하고 격자 위의 커서·고스트 클립 블록은 S4-T1 이월로 넘긴다(에이전트 해석, 아래 리스크).
4. S3-T4b 넘김: `n n`의 조합 표시가 `nc_`(셋잇단 한 번과 맞게, 리뷰 3 제안 3). 결정 `2026-10-30-s3-t4b-vim-step-mml.md` 2절의 경계 문구를 「음·쉼표는 커서가 든 클립, 타이는 직전 스텝의 클립, `Backspace`·`Shift+Backspace`는 커서 바로 앞 틱의 클립. 어느 경우든 그 한 클립 안만 지운다」로 나눈다(리뷰 3 제안 2). 결정 2026-10-29 1절에 같은 경계(커서 = 앞 클립 끝 = 다음 클립 시작이면 다음 클립)를 한 줄 더한다.

## 공개 인터페이스 초안

~~~rust
// crates/keyscore-app/src/engine.rs
/// 미리듣기가 스냅샷을 새로 준비한 횟수(시험·진단용).
pub fn preview_prepares(cx: &App) -> u64;
impl Engine {
    pub(crate) fn forget_preview(&mut self);              // 1·2의 무효화
    pub(crate) fn preview_route(&self, ports: &[OutputPort], destination: &str) -> Option<u16>;
    pub(crate) fn remember_preview(&mut self, ports: Vec<OutputPort>, routes: Vec<(String, u16)>);
    pub(crate) fn setup_sent(&self, destination: &str, channel: u8, messages: &[Midi1Message]) -> bool;
    pub(crate) fn note_setup_sent(&mut self, destination: &str, channel: u8, messages: Vec<Midi1Message>);
}

// crates/keyscore-app/src/preview.rs
pub(crate) enum Purpose { Preview, TestNote }
fn sound_note(&mut self, track, pitch, velocity, length, purpose: Purpose, cx) -> NoteSend;

// crates/keyscore-app/src/step_input.rs (또는 새 모듈)
impl ProjectWindow {
    /// 배치 뷰의 Step 위치 표시 글(시험·접근성용). Step이 아니거나 트랙이 없으면 `None`.
    pub fn arrangement_step_text(&self, cx: &App) -> Option<ArrangementStep>;
}
pub struct ArrangementStep { pub cursor: String, pub clip: String }
~~~

## 테스트 목록

미리듣기 시험 `crates/keyscore-app/tests/preview_light.rs`(CoreMIDI 엔진, 앱의 추가 가상 포트를 이름 포트로 쓴다). 배치 뷰 시험 `crates/keyscore-app/tests/arrangement_step.rs`(헤드리스).

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `preview_repeat_on_named_port_prepares_once` | UI(MIDI) | 이름 포트 트랙에서 Step 음 키 셋: `preview_prepares`가 1 늘고 Note On 셋이 그 포트로 온다 |
| 2 | `preview_after_track_port_change_prepares_again` | UI(MIDI) | 트랙 포트를 다른 이름 포트로 바꾸면 다시 준비하고 새 포트로 온다 |
| 3 | `preview_after_play_stop_prepares_again_and_resends_program` | UI(MIDI) | 재생·정지 뒤 미리듣기는 다시 준비하고 프로그램을 다시 보낸다 |
| 4 | `preview_sends_program_once_while_unchanged` | UI(MIDI) | 가상 포트, 프로그램 7: 음 키 둘에 프로그램 한 번, Note On 둘 |
| 5 | `preview_resends_program_after_track_program_edit` | UI(MIDI) | 트랙 프로그램을 바꾸면 다음 미리듣기가 새 프로그램을 먼저 보낸다 |
| 6 | `preview_alternating_tracks_on_shared_channel_resend_program` | UI(MIDI) | 같은 채널·다른 프로그램 두 트랙을 번갈아 들으면 매번 그 트랙 프로그램이 먼저 간다 |
| 7 | `test_note_always_sends_program` | UI(MIDI) | 테스트 노트 두 번에 프로그램 두 번 |
| 8 | `preview_to_missing_port_finds_port_added_later` | UI(MIDI) | 없는 이름 포트 미리듣기 실패 → 그 이름의 가상 포트를 만들면 다음 미리듣기가 닿는다 |
| 9 | `preview_after_virtual_port_rename_follows_new_port` | UI(MIDI) | 이름 포트 미리듣기 뒤 그 가상 포트 이름을 바꾸면(트랙도 따라감) 다음 미리듣기가 새 이름 포트로 간다 |
| 10 | `step_in_arrangement_shows_track_and_cursor` | UI | 배치 뷰 Step: `arrangement-step-cursor`에 트랙 이름·`1.1.0` |
| 11 | `step_in_arrangement_inside_clip_names_clip` | UI | 클립 안이면 그 클립(이름 없으면 시작 위치) |
| 12 | `step_in_arrangement_at_clip_end_says_extend` | UI | 클립 끝 = 커서면 「이어 씀」 |
| 13 | `step_in_arrangement_outside_clip_shows_new_clip_bars` | UI | 빈 마디 3에서 「입력하면 마디 3–3에 새 클립」, 온음표+점(긴 스텝)이면 3–4 |
| 14 | `step_in_arrangement_follows_input` | UI | 음 하나를 친 뒤 표시가 새 클립 안·커서 위치로 바뀐다 |
| 15 | `step_in_arrangement_blocked_by_next_clip_says_why` | UI | 다음 클립이 스텝 안에서 시작하면 쓸 수 없는 이유 |
| 16 | `normal_in_arrangement_has_no_step_display` | UI | Normal에서는 표시 없음(자리표시 그대로), Step을 나가면 사라짐 |
| 17 | `step_in_piano_roll_has_no_arrangement_display` | UI | 피아노롤에서는 이 표시를 그리지 않는다 |
| 18 | `triplet_twice_shows_single_n` | UI | `n n c`의 표시 `nc_`, 노트 640 |

## 불변식

- 실시간 스레드 경로는 바꾸지 않는다(스케줄러 API 그대로). 기억은 메인 스레드의 `Engine` 안에만 있다.
- 미리듣기가 다른 창의 재생 스냅샷을 바꾸지 않는다(기존 규칙 그대로).
- 기억한 경로표의 포트 번호는 그 스냅샷이 지금 스케줄러에 있을 때만 쓴다. 엔진 밖에서 스냅샷을 넣는 길은 `prepare` 뒤뿐이므로(`transport.rs`) `prepare`에서 잊으면 된다. 엔진 안에서 직접 넣는 곳(`Snapshot::new` 다섯 곳)은 모두 잊는다.
- 표시는 문서·커서를 바꾸지 않는다(그리기 경로에서 편집 없음).

## 범위 밖

- 배치 뷰 Element(트랙×시간 격자, 클립 블록, 격자 위 커서·고스트 클립): S4-T1. 이 Task의 표시 글은 S4-T1이 격자 표시로 바꾸며 지운다(이월 줄을 단계 파일에 적는다).
- 재생 중 미리듣기 경로(이미 스냅샷 재사용), 다른 창 재생 중 경로: 그대로.
- 실제 신디의 키 반복 측정(단계 파일 「실제 신디로 먼저 재 본다」): 에이전트가 소리를 들을 수 없다. 보고서 「사람 확인」에 둔다. 대신 헤드리스 시험으로 준비 횟수와 메시지 열을 고정한다.
- 결정 대기 `2026-10-26-duplicate-offset-grid.md`: 결정이 나지 않았다.

## 리스크·결정 요청

- 신디 쪽에서 사람이 음색을 손으로 바꾸면, 다음 미리듣기는 프로그램을 다시 보내지 않아 그 음색으로 들린다. 재생 시작(chase)과 테스트 노트는 늘 보낸다. 단계 파일이 「반복 전송 줄이기」를 요구하므로 받아들이고 보고서에 적는다.
- 배치 뷰 표시를 글로 하는 것은 에이전트 해석이다. 결정 `2026-10-17-mode-entry-view.md` 이월은 「커서·클립 생성 표시를 그린다」이고 REQ에는 모양이 없다. 격자가 없는 지금 격자 위에 그리면 S4-T1이 다시 만든다. 결정 문서 이월 줄에 이 읽기와 S4-T1 이월을 적는다.
