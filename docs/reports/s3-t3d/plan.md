# Task 계획: S3-T3d 곡 끝 정지

- 브랜치: `s3/t3d-end-of-song`
- 관련 REQ: TRN-1(재생/정지), 5.8 `Space`/`Shift+Space`, UI-4(재생 헤드), MET-1(메트로놈)
- 선행 Task: S2-T5a(트랜스포트), S3-T2(커서부터 재생)
- 관련 결정: `docs/decisions/2026-10-25-playback-end-of-song.md` A(곡 끝에서 정지·틱 0·메트로놈 멈춤, Sequence 모드 예외)
- 관련 스파이크: 없음

## 끝의 정의 (내 말로)

재생이 곡 끝에 닿으면 스스로 멈춘다. 트랜스포트 상태는 「정지」, 표시 위치는 틱 0, 재생 헤드는 사라지고 메트로놈 클릭도 멈춘다. Sequence 모드에서는 멈추지 않는다. 규칙은 아래가 전부이고 시험이 고정한다.

### 1. 곡 끝

- 곡 끝은 모든 트랙에서 가장 늦게 끝나는 클립의 끝 틱(`Clip::end`, 배타)이다. 숨김 꼬리(클립 끝 뒤로 나간 노트 부분)는 세지 않는다. 클립이 없으면 곡 끝은 틱 0이다(결정 문서 「상황」).
- 재생 중 편집으로 곡이 길어지거나 짧아지면 스냅샷 교체(TRN-3)와 함께 새 곡 끝이 들어간다. 재생 위치가 이미 새 곡 끝을 지났으면 곧바로 멈춘다.

### 2. 어디서 멈추는가: 스케줄러

- 결정 문서의 추천 구현은 「창이 재생 헤드 보간으로 곡 끝을 보고 `stop`」이었다. 그러면 스케줄러가 lookahead(10 ms) 안에 이미 넘긴 곡 끝 틱의 클릭(곡이 마디 경계에서 끝나면 다음 마디 첫 박)과 앱 타이머 지터 동안의 클릭·Clock이 나간다. 결정의 「메트로놈도 멈춘다」를 정확히 지키려고, 곡 끝은 스냅샷의 값으로 스케줄러에 넘기고 스케줄러가 그 틱에서 스스로 멈춘다. 스케줄러에 위치 조회는 더하지 않는다(결정 문서의 뜻과 같다).
- `keyscore_midi::Snapshot::with_end(Option<Tick>)`: 곡 끝. `None`이면 멈추지 않는다(지금 동작).
- 스케줄러는 곡 끝 틱 `E`에서:
  - 틱 `E`의 스냅샷 이벤트(Note Off 등, 같은 틱 정렬 규칙대로)는 보낸다. 노트는 클립 안에 있으므로 `E`에 Note On은 없다.
  - 틱 `E`의 박(클릭·박 보고)과 MIDI Clock은 보내지 않는다(다음 마디의 것이다. 루프 끝 규칙과 같다).
  - 울리는 노트를 모두 끄고(`E`의 호스트 시각), 클럭 장치에 Stop(0xFC)을 보내고 정지 상태가 된다. 정지 명령과 같은 정리다.
  - 재생 시작 위치가 이미 `E` 이상이면(빈 프로젝트, 곡 끝 뒤 커서에서 `Shift+Space`, 곡 끝에서 일시정지 뒤 재개) chase만 하고 곧바로 같은 정리로 멈춘다.
- 루프 구간이 켜져 있으면 곡 끝으로 멈추지 않는다. 루프 구간은 S5에서 생기며, 그때 루프와 곡 끝의 관계를 다시 본다(지금은 앱이 루프를 켜지 않는다).
- 스케줄러가 곡 끝에서 멈췄음을 앱에 알리는 길: `Scheduler::take_ended() -> bool`. 마지막 `play`·`locate` 뒤에 곡 끝으로 멈췄으면 한 번 `true`다. 그 뒤에 `play`를 넣었으면 옛 정지는 `false`로 친다(재생을 다시 시작한 창을 정지로 표시하지 않는다). 실시간 스레드 쪽은 원자값 하나를 쓴다(할당·락 없음).

### 3. 앱

- `Engine::prepare`가 스냅샷에 곡 끝을 넣는다. 창이 Sequence 모드면 `None`.
- 재생을 시작한 창은 곡 끝 감시 작업(`END_POLL` = 20 ms 주기 타이머)을 든다. 주기마다 자신이 엔진의 주인이고 `take_ended()`가 `true`면 `mark_stopped`(상태 「정지」, 위치 틱 0, 재생 헤드 없음)하고 다시 그린다. 정지·다른 창의 재생·서버 재시작으로 정지하면 감시 작업도 놓는다.
- 재생 중 모드가 Sequence로 들어가거나 나오면 스냅샷을 다시 넣어(`transport_refresh`) 곡 끝 유무를 바꾼다. Sequence에서 나온 순간 곡 끝을 지났으면 곧바로 멈춘다.
- 곡 끝 정지 뒤 `Space`는 처음부터 재생한다(정지 상태의 규칙 그대로).

## 공개 인터페이스 초안

~~~rust
// keyscore-midi
impl Snapshot {
    /// 곡 끝 틱. 재생이 이 틱에 닿으면 스케줄러가 스스로 멈춘다. `None`이면 멈추지 않는다.
    #[must_use]
    pub fn with_end(self, end: Option<Tick>) -> Self;
    #[must_use]
    pub fn end(&self) -> Option<Tick>;
}
impl Scheduler {
    /// 마지막 `play`·`locate` 뒤 곡 끝에서 스스로 멈췄으면 한 번 `true`.
    pub fn take_ended(&mut self) -> bool;
}

// keyscore-app (crate 내부)
pub(crate) fn song_end(project: &Project) -> Tick;
pub const END_POLL: Duration; // 시험이 시계를 돌리는 데 쓴다
~~~

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `song_end_stops_playback_and_sends_offs` | 통합(midi 루프백) | 곡 끝 틱의 Note Off는 오고, 뒤로 이벤트가 없고, `take_ended`가 한 번 `true` |
| 2 | `song_end_does_not_click_on_end_tick` | 통합(midi 루프백) | 한 마디 곡 + 메트로놈: 클릭 정확히 네 번, 곡 끝 박 클릭 없음 |
| 3 | `play_from_at_or_after_end_stops_at_once` | 통합(midi 루프백) | 시작 위치 ≥ 곡 끝이면 클릭·노트 없이 멈춤 |
| 4 | `end_none_keeps_playing` | 통합(midi 루프백) | `with_end(None)`이면 지금처럼 계속 |
| 5 | `swap_extending_end_keeps_playing` | 통합(midi 루프백) | 재생 중 더 긴 곡 끝으로 교체하면 새 끝까지 |
| 6 | `swap_to_end_before_position_stops_at_once` | 통합(midi 루프백) | 지난 곡 끝으로 교체하면 곧바로 멈춤 |
| 7 | `take_ended_is_false_after_new_play` | 통합(midi 루프백) | 끝으로 멈춘 뒤 다시 `play`하면 옛 정지를 보고하지 않음 |
| 8 | `song_end_in_loop_is_ignored` | 통합(midi 루프백) | 루프 중에는 곡 끝으로 멈추지 않음 |
| 9 | `song_end_is_latest_clip_end` | 단위(app) | 여러 트랙·클립, 숨김 꼬리, 클립 없음 → 0 |
| 10 | `playback_stops_at_song_end_and_rewinds` | UI + 루프백(app) | 곡 끝 뒤 상태 정지·위치 0·재생 헤드 없음, 메트로놈 클릭 멈춤 |
| 11 | `sequence_mode_keeps_playing_past_song_end` | UI + 루프백(app) | Sequence 모드에서 곡 끝 뒤에도 재생 중, 클릭 계속 |
| 12 | `leaving_sequence_past_end_stops` | UI + 루프백(app) | 곡 끝 뒤 Sequence에서 나오면 멈춤 |
| 13 | `edit_extending_song_while_playing_plays_to_new_end` | UI + 루프백(app) | 재생 중 클립을 늘리면 새 끝의 노트가 들림 |
| 14 | `space_after_song_end_plays_from_start` | UI + 루프백(app) | 곡 끝 정지 뒤 재생은 처음부터 |

## 불변식

- 실시간 스레드: 곡 끝 처리는 기존 정지 정리(`release_notes`, Clock Stop)와 원자값 저장만 쓴다. 할당·락·로그 없음(`assert_no_alloc`).
- 같은 틱 순서: 곡 끝 틱의 이벤트 > 곡 끝 > 그 틱의 클릭·Clock(보내지 않음).
- 창 상태와 소리: 창이 「재생 중」인 동안 스케줄러가 멈췄으면 다음 감시 주기(20 ms) 안에 창도 「정지」가 된다.

## 범위 밖

- 루프 구간(S5)과 곡 끝의 관계. 지금은 루프 중 곡 끝을 무시한다.
- 곡 끝 표식(Logic의 프로젝트 끝처럼 사람이 옮기는 표식). REQ에 없다.
- 배치 뷰 커서부터 재생(S4-T1 이월 그대로).
- Sequence 모드의 녹음 자체(S5).

## 리스크·결정 요청

- 결정 문서의 추천 구현(창이 보간으로 보고 `stop`)과 달리 스케줄러가 멈춘다. 결정의 결과(정지·틱 0·메트로놈 멈춤·Sequence 예외)는 같고, 클릭이 곡 끝 박에서 한 번 더 나는 일을 막으려는 구현 선택이다. 결정 문서 「구현자 추천」도 「충분하다」고만 적었으므로 결정 요청 없이 진행한다.
- 곡 끝 직전의 편집으로 곡이 길어졌는데 교체가 스케줄러에 닿기 전에 옛 곡 끝을 지나면 멈춘다(교체는 정지를 되돌리지 않는다). 창 쪽 지연이 수 ms라 드물다. 알려진 한계로 보고서에 적는다.
