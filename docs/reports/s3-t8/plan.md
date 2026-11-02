# Task 계획: S3-T8 일시정지 자리와 타이밍 시험 직렬화

- 브랜치: `s3/t8-pause-position`
- 관련 REQ: TRN-1(재생·정지·일시정지), TRN-3(재생 중 편집), NF-3·NF-4(지터·드리프트 측정)
- 선행 Task: S1-T4(스케줄러), S2-T5a(트랜스포트), S3-T3d(곡 끝 정지)
- 관련 스파이크: 없음
- 성격: S3 출구 서명을 기다리는 동안 하는 후속(HANDOFF 「남은 작업」의 두 줄). S4 범위를 건드리지 않는다.

## 배경 (조사 결과)

`review_s2_t5a_r2::pause_position_does_not_drift_with_many_live_edits`가 전체 시험에서 가끔 실패했다(일시정지 자리가 들은 박보다 9–20틱 앞). 부하 탓으로 보았으나 단독으로 25번 돌리면 1번 실패한다(Tick 6220 대 들은 박 6240). 원인은 부하가 아니다.

- 스케줄러는 지금부터 lookahead(기본 10 ms, 120 BPM에서 약 19틱) 안의 이벤트를 OS에 미리 넘긴다. 넘긴 예약은 취소하지 않는다.
- 앱의 일시정지 자리(`transport_pause`)는 앱이 든 기준점(`Anchor`)으로 구한 「지금」의 틱이다.
- 그래서 일시정지를 박 직전 19틱 안에서 누르면 그 박은 이미 넘겨져 들리는데 자리는 박 앞이다. 확률은 약 19/480 ≈ 4%이고 관찰과 맞다.
- 시험만의 문제가 아니다. 이어서 재생하면 자리부터 다시 보내므로 이미 울린 노트가 한 번 더 울린다(겹침).

## 끝의 정의 (내 말로)

1. 일시정지 자리는 스케줄러가 「OS에 아직 넘기지 않은 첫 틱」으로 알려 준다. 이어서 재생하면 그 틱부터 보내므로 이미 넘긴 노트를 다시 보내지 않고, 넘기지 않은 노트를 빠뜨리지 않는다. 재생 중 교체(`swap`)가 보낸 구간 뒤부터 이어 보내는 규칙(`sent_through`)과 같은 규칙이다.
2. 스케줄러가 제때(상한 50 ms) 답하지 않으면(스레드가 굶거나 끝남) 지금처럼 앱의 추정 자리를 쓴다.
3. `crates/keyscore-midi/tests/scheduler.rs`의 시험은 같은 바이너리 안에서 서로 직렬로 돈다. 20초 지터·드리프트 측정이 다른 루프백 시험과 겹쳐 0.5 ms를 넘던 흔들림을 없앤다.

## 공개 인터페이스 초안

~~~rust
// crates/keyscore-midi/src/scheduler.rs
impl Scheduler {
    /// 정지하고, 재생 중이었으면 이어서 재생할 틱(아직 OS에 넘기지 않은 첫 틱)을 돌려준다.
    /// 재생 중이 아니었거나 `wait` 안에 스레드가 정지를 처리하지 않았으면 `None`.
    pub fn pause(&mut self, wait: Duration) -> Result<Option<Tick>, MidiError>;
}
// Signals에 `paused: AtomicU64`(처리한 명령 번호), `paused_tick: AtomicU64`(틱, 재생 중이 아니면 u64::MAX).
// 실시간 스레드는 원자 store만 한다(할당·락 없음).
~~~

앱: `ProjectWindow::transport_pause`가 이 창이 엔진 주인이면 `scheduler().pause(PAUSE_WAIT)`를 쓰고, `Some(t)`면 `Paused(t)`, `None`이면 지금의 추정 자리. `PAUSE_WAIT`는 50 ms.

## 테스트 목록

| # | 테스트 이름 | 종류 | 검사하는 것 |
| --- | --- | --- | --- |
| 1 | `pause_returns_first_unsent_tick_after_lookahead` (`keyscore-midi/tests/scheduler.rs`) | 통합(루프백) | 재생 중 `pause`가 돌려준 틱보다 앞의 노트는 모두 받았고, 그 틱 이상의 노트는 하나도 받지 않았다. 반복(박 직전을 노려 20번) |
| 2 | `pause_resume_neither_repeats_nor_drops_notes` (같은 파일) | 통합(루프백) | 16분음표 열을 재생 → `pause` → 그 틱부터 `play`를 여러 번. 받은 Note On의 음 번호(노트마다 다름)가 빠짐도 겹침도 없다 |
| 3 | `pause_when_stopped_returns_none` (같은 파일) | 통합 | 정지 중 `pause`는 `Ok(None)`이고 정지 명령처럼 동작한다 |
| 4 | `pause_position_never_precedes_heard_beat` (`keyscore-app/tests/pause_position.rs`) | 통합(앱+루프백) | 박 트랙을 재생하며 박 직전 여러 자리에서 `transport::Pause`를 20번. 매번 자리 ≥ 마지막으로 들은 박 + 1 |
| 5 | `resume_after_pause_does_not_repeat_heard_note` (같은 파일) | 통합(앱+루프백) | 일시정지 → `transport::Play`로 이어서. 들은 박이 겹치지 않는다 |
| 6 | 기존 `review_s2_t5a_r2::pause_position_does_not_drift_with_many_live_edits`, `pause_after_several_tempo_edits_matches_elapsed_time` | 통합 | 고치지 않고 그대로 통과(앞의 것은 결정적으로) |

1·4는 박 직전에 누르는 경우를 노려야 구현 전에 확실히 실패한다. 박 시각은 `Scheduler::poll_beat`(앱 시험은 루프백 수신 시각)로 알고, 다음 박 5 ms 전에 누른다.

## 불변식

- 실시간 스레드: 정지 처리에 원자 store 둘만 더한다. 할당·락·로그 없음(`assert_no_alloc` 범위 그대로).
- 신호 순서: 스레드는 `paused_tick`을 쓴 뒤 `paused`(명령 번호)를 Release로 쓴다. 손잡이는 `paused`를 Acquire로 읽어 자기 명령 번호 이상일 때만 `paused_tick`을 읽는다. 손잡이는 `&mut self`라 기다리는 동안 다른 명령을 넣지 않는다.
- 이어서 재생할 틱 = `sent_through + 1`, 기준점 뒤로 보낸 것이 없으면 기준점 틱. 교체(`swap`)의 「보낸 구간 뒤 첫 이벤트부터」와 같다.
- 앱의 `TransportState::Paused` 값의 의미(이어서 재생할 자리)는 그대로다.

## 범위 밖

- 넘긴 예약을 OS에서 지우기(`MIDIFlushOutput`). 일시정지를 누른 뒤 lookahead(10 ms) 안의 노트는 지금처럼 울린다. 재생 시작 지연과 같은 크기라 듣기에 문제 없다고 본다.
- 재생 헤드 표시. 일시정지 자리가 화면의 추정 자리보다 최대 lookahead만큼 뒤로 보일 수 있다(약 19틱, 줌 기본에서 2 px 안팎).
- 루프 구간과의 관계(S5 이월 줄 「`Scheduler::locate`와 루프·곡 끝」). 이어서 재생할 틱이 루프 끝과 같아지는 경우는 S5에서 본다.
- `review_s1_t4_r2::drop_with_queued_swaps_releases_all_ports_on_control_thread`의 흔들림. 리뷰어가 같은 기계에서 시험을 돌릴 때만 보였다(다른 프로세스와의 겹침).

## 리스크·결정 요청

- REQ에 없는 세부(이어서 재생할 자리를 「넘기지 않은 첫 틱」으로)는 DAW 관례(일시정지 뒤 이어 재생은 겹침·빠짐 없음)와 이미 만든 교체 규칙으로 정한다(`MEMORY.md` 「세부 판단 위임」). 결정 요청은 쓰지 않는다.
- 클릭(메트로놈)만 남고 같은 틱의 이벤트는 넘긴 경우 `sent_through`는 그 틱이다. 이어서 재생하면 그 틱의 클릭 하나를 건너뛴다. 항목 상한에 걸린 바퀴에서만 생기며, 노트를 두 번 내는 것보다 낫다고 본다.
- 메인 스레드가 최대 50 ms 기다린다. 스케줄러 스레드는 1 ms마다 깨므로 보통 1–2 ms다.
